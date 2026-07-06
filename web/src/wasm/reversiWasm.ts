// The functions exported by reversi.wasm (a plain C ABI, not wasm-bindgen).
export interface ReversiWasm {
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

export interface GeneratedEndgame {
  black: bigint;
  white: bigint;
  margin: bigint;
}

export interface ReversiApi {
  validMoves(me: bigint, opp: bigint): bigint;
  flipMask(me: bigint, opp: bigint, mov: bigint): bigint;
  aiMove(me: bigint, opp: bigint, seed: number): bigint;
  generateEndgame(seed: number, targetEmpties: number): GeneratedEndgame | null;
}

// WebAssembly returns i64 values, which JS surfaces as *signed* BigInts (a set
// bit 63 becomes negative). Mask every return value back to unsigned 64-bit so
// popcount and bit tests behave. Passing unsigned BigInts back in as arguments
// is fine: wasm wraps them to i64 automatically.
const U64 = (1n << 64n) - 1n;

export function wrapWasm(wasm: ReversiWasm): ReversiApi {
  return {
    validMoves: (me, opp) => wasm.valid_moves(me, opp) & U64,
    flipMask: (me, opp, mov) => wasm.flip_mask(me, opp, mov) & U64,
    aiMove: (me, opp, seed) => wasm.ai_move(me, opp, seed) & U64,
    generateEndgame(seed, targetEmpties) {
      const ok = (wasm.generate_endgame(seed, targetEmpties) & U64) !== 0n;
      if (!ok) return null;
      return {
        black: wasm.generated_black() & U64,
        white: wasm.generated_white() & U64,
        margin: wasm.generated_margin() & U64,
      };
    },
  };
}

export async function loadReversiWasm(): Promise<ReversiApi> {
  const wasm = await loadRawWasm();
  return wrapWasm(wasm);
}

async function loadRawWasm(): Promise<ReversiWasm> {
  try {
    const res = await WebAssembly.instantiateStreaming(
      fetch("reversi.wasm"),
      {},
    );
    return res.instance.exports as unknown as ReversiWasm;
  } catch {
    // Fallback for servers that don't send application/wasm.
    const buf = await (await fetch("reversi.wasm")).arrayBuffer();
    const res = await WebAssembly.instantiate(buf, {});
    return res.instance.exports as unknown as ReversiWasm;
  }
}
