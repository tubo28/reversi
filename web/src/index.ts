// Frontend controller for the WebAssembly reversi engine.
//
// The engine (src/wasm.rs) exposes three pure functions, all from the
// "black to move" perspective:
//   valid_moves(me, opp)      -> legal move mask for the side to move
//   flip_mask(me, opp, mov)   -> opponent disks flipped by playing `mov`
//   ai_move(me, opp, seed)    -> best move mask (0 = pass)
// The whole board is two u64s, kept here as BigInts. When it is white's turn
// we swap the arguments, since move positions are color-independent.

import "./style.css";

// The functions exported by reversi.wasm (a plain C ABI, not wasm-bindgen).
interface ReversiWasm {
  valid_moves(black: bigint, white: bigint): bigint;
  flip_mask(black: bigint, white: bigint, mov: bigint): bigint;
  ai_move(black: bigint, white: bigint, seed: number): bigint;
  // Sprint mode: generate a position where the side to move has a proven forced
  // win (confirmed by exact endgame search), then read it back via the getters.
  generate_endgame(seed: number, targetEmpties: number): bigint;
  generated_black(): bigint;
  generated_white(): bigint;
  generated_margin(): bigint;
}

type Side = "black" | "white";

// Starting position, matching Rust's Board::new():
// black on (3,4) & (4,3); white on (3,3) & (4,4). Bit index = row * 8 + col.
const START_BLACK = (1n << 28n) | (1n << 35n);
const START_WHITE = (1n << 27n) | (1n << 36n);

// WebAssembly returns i64 values, which JS surfaces as *signed* BigInts (a set
// bit 63 becomes negative). Mask every return value back to unsigned 64-bit so
// popcount and bit tests behave. Passing unsigned BigInts back in as arguments
// is fine: wasm wraps them to i64 automatically.
const U64 = (1n << 64n) - 1n;
let wasm: ReversiWasm;
const validMoves = (me: bigint, opp: bigint): bigint => wasm.valid_moves(me, opp) & U64;
const flipMask = (me: bigint, opp: bigint, mov: bigint): bigint => wasm.flip_mask(me, opp, mov) & U64;
const aiMoveMask = (me: bigint, opp: bigint, seed: number): bigint => wasm.ai_move(me, opp, seed) & U64;

let black = START_BLACK;
let white = START_WHITE;
let turn: Side = "black"; // side to move
let humanColor: Side = "black";
let gameOver = false;
let busy = false; // true while the AI is thinking (blocks input)
let lastMove = -1; // cell index of the disk just placed (-1 = none)

const boardEl = document.getElementById("board") as HTMLElement;
const statusEl = document.getElementById("status") as HTMLElement;
const blackCountEl = document.getElementById("black-count") as HTMLElement;
const whiteCountEl = document.getElementById("white-count") as HTMLElement;

// --- bit helpers ---------------------------------------------------------

function bitAt(index: number): bigint {
  return 1n << BigInt(index);
}

function isSet(mask: bigint, index: number): boolean {
  return ((mask >> BigInt(index)) & 1n) === 1n;
}

// Cell index of a single-bit mask.
function indexOfBit(bit: bigint): number {
  return bit.toString(2).length - 1;
}

function popcount(x: bigint): number {
  let c = 0;
  while (x > 0n) {
    x &= x - 1n;
    c++;
  }
  return c;
}

// --- perspective helpers -------------------------------------------------

// [me, opp] disks for the side to move.
function sideToMove(): [bigint, bigint] {
  return turn === "black" ? [black, white] : [white, black];
}

function legalFor(side: Side): bigint {
  return side === "black"
    ? validMoves(black, white)
    : validMoves(white, black);
}

function other(side: Side): Side {
  return side === "black" ? "white" : "black";
}

// Play `bit` for the current side to move and store the new board.
function applyMove(bit: bigint): void {
  const [me, opp] = sideToMove();
  const flip = flipMask(me, opp, bit);
  const newMe = me | bit | flip;
  const newOpp = opp ^ flip;
  lastMove = indexOfBit(bit);
  if (turn === "black") {
    black = newMe;
    white = newOpp;
  } else {
    white = newMe;
    black = newOpp;
  }
}

// --- rendering -----------------------------------------------------------

function render(): void {
  const humanTurn = !gameOver && !busy && turn === humanColor;
  const legal = humanTurn ? legalFor(turn) : 0n;

  boardEl.replaceChildren();
  for (let index = 0; index < 64; index++) {
    const cell = document.createElement("button");
    cell.type = "button";
    cell.className = "cell";

    if (isSet(black, index) || isSet(white, index)) {
      const d = document.createElement("span");
      d.className = isSet(black, index) ? "disk black" : "disk white";
      if (index === lastMove) {
        const marker = document.createElement("span");
        marker.className = "marker";
        d.appendChild(marker);
      }
      cell.appendChild(d);
    } else if (isSet(legal, index)) {
      const hint = document.createElement("span");
      hint.className = "hint";
      cell.appendChild(hint);
      cell.classList.add("playable");
      cell.addEventListener("click", () => onHumanMove(index));
    }
    boardEl.appendChild(cell);
  }

  blackCountEl.textContent = String(popcount(black));
  whiteCountEl.textContent = String(popcount(white));
}

function setStatus(text: string): void {
  statusEl.textContent = text;
}

const label = (side: Side): string => (side === "black" ? "Black" : "White");

// --- game flow -----------------------------------------------------------

function onHumanMove(index: number): void {
  if (gameOver || busy || turn !== humanColor) return;
  const bit = bitAt(index);
  const [me, opp] = sideToMove();
  if (flipMask(me, opp, bit) === 0n) return; // illegal
  applyMove(bit);
  turn = other(turn);
  render();
  step();
}

// Drive the game until it is the human's turn or the game ends.
function step(): void {
  if (gameOver) return;

  const legal = legalFor(turn);
  if (legal === 0n) {
    if (legalFor(other(turn)) === 0n) {
      finish();
      return;
    }
    // Current side must pass.
    setStatus(`${label(turn)} passed`);
    turn = other(turn);
    render();
    // Continue after a short pause so the pass message is visible.
    setTimeout(step, 700);
    return;
  }

  if (turn === humanColor) {
    setStatus(`Your turn (${label(turn)})`);
    render();
    return;
  }

  // AI's turn.
  busy = true;
  setStatus("AI is thinking…");
  render();
  setTimeout(aiMove, 350);
}

function aiMove(): void {
  const [me, opp] = sideToMove();
  const seed = Math.floor(Math.random() * 0x100000000);
  const bit = aiMoveMask(me, opp, seed);
  busy = false;
  if (bit !== 0n) {
    applyMove(bit);
  }
  turn = other(turn);
  render();
  step();
}

function finish(): void {
  gameOver = true;
  render();
  const b = popcount(black);
  const w = popcount(white);
  const humanCount = humanColor === "black" ? b : w;
  const aiCount = humanColor === "black" ? w : b;
  let result: string;
  if (humanCount > aiCount) result = "You win! 🎉";
  else if (humanCount < aiCount) result = "AI wins";
  else result = "Draw";
  setStatus(`Game over — ${result} (Black ${b} : White ${w})`);
}

function newGame(color: Side): void {
  black = START_BLACK;
  white = START_WHITE;
  turn = "black";
  humanColor = color;
  gameOver = false;
  busy = false;
  lastMove = -1;
  render();
  step();
}

// --- sprint mode ---------------------------------------------------------

// Generate a guaranteed-win endgame and start playing it out. The generation is
// a heavy synchronous WASM call, so we paint the "generating" status first (via
// setTimeout) before blocking, matching how aiMove() is scheduled.
function newSprint(): void {
  busy = true;
  gameOver = false;
  lastMove = -1;
  setStatus("生成中。。。");
  render(); // redraw with the board disabled while we generate

  setTimeout(() => {
    const seed = Math.floor(Math.random() * 0x100000000);
    const targetEmpties = Number(
      (document.getElementById("sprint-empties") as HTMLSelectElement).value,
    );
    const ok = (wasm.generate_endgame(seed, targetEmpties) & U64) !== 0n;
    if (!ok) {
      busy = false;
      setStatus("生成に失敗しました。もう一度お試しください。");
      render();
      return;
    }
    // The stashed board is from the mover's perspective; present the human as
    // Black to move (Reversi is colour-symmetric, so this is just a label).
    black = wasm.generated_black() & U64;
    white = wasm.generated_white() & U64;
    const margin = wasm.generated_margin() & U64;
    turn = "black";
    humanColor = "black";
    busy = false;
    gameOver = false;
    lastMove = -1;
    render();
    step();
    setStatus(`あなたの手番です（最善で必勝・+${margin}石）`);
  }, 50);
}

// --- boot ----------------------------------------------------------------

async function loadWasm(): Promise<ReversiWasm> {
  try {
    const res = await WebAssembly.instantiateStreaming(fetch("reversi.wasm"), {});
    return res.instance.exports as unknown as ReversiWasm;
  } catch (_) {
    // Fallback for servers that don't send application/wasm.
    const buf = await (await fetch("reversi.wasm")).arrayBuffer();
    const res = await WebAssembly.instantiate(buf, {});
    return res.instance.exports as unknown as ReversiWasm;
  }
}

async function main(): Promise<void> {
  try {
    wasm = await loadWasm();
  } catch (e) {
    setStatus("Failed to load WASM. Please open this page via an HTTP server.");
    console.error(e);
    return;
  }
  (document.getElementById("new-black") as HTMLElement).addEventListener("click", () => newGame("black"));
  (document.getElementById("new-white") as HTMLElement).addEventListener("click", () => newGame("white"));
  (document.getElementById("new-sprint") as HTMLElement).addEventListener("click", () => newSprint());
  newGame("black");
}

main();
