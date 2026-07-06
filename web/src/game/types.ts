export type Turn = "black" | "white";

// The outcome of a finished game (mirrors Rust's `Winner`).
export type Winner = "black" | "white" | "draw";

// Starting position, matching Rust's Board::new():
// black on (3,4) & (4,3); white on (3,3) & (4,4). Bit index = row * 8 + col.
export const START_BLACK = (1n << 28n) | (1n << 35n);
export const START_WHITE = (1n << 27n) | (1n << 36n);

export function other(turn: Turn): Turn {
  return turn === "black" ? "white" : "black";
}

export function label(turn: Turn): string {
  return turn === "black" ? "Black" : "White";
}

export interface GameState {
  black: bigint;
  white: bigint;
  turn: Turn;
  humanColor: Turn;
  gameOver: boolean;
  // Winner of the finished game, or null while it is still in progress.
  winner: Winner | null;
  busy: boolean;
  lastMove: number; // cell index of the disk just placed (-1 = none)
  status: string;
  // Legal moves for the human, as of the last time it became their turn.
  // Only meaningful while it's actually the human's interactive turn.
  legalMoves: bigint;
}

export function initialGameState(humanColor: Turn): GameState {
  return {
    black: START_BLACK,
    white: START_WHITE,
    turn: "black",
    humanColor,
    gameOver: false,
    winner: null,
    busy: false,
    lastMove: -1,
    status: "",
    legalMoves: 0n,
  };
}
