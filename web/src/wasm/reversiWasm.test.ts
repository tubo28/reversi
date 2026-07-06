import { describe, expect, it, vi } from "vitest";
import type { ReversiWasm } from "./reversiWasm";
import { wrapWasm } from "./reversiWasm";

function fakeWasm(overrides: Partial<ReversiWasm> = {}): ReversiWasm {
  return {
    valid_moves: vi.fn(() => 0n),
    flip_mask: vi.fn(() => 0n),
    ai_move: vi.fn(() => 0n),
    generate_endgame: vi.fn(() => 0n),
    generated_black: vi.fn(() => 0n),
    generated_white: vi.fn(() => 0n),
    generated_margin: vi.fn(() => 0n),
    ...overrides,
  };
}

describe("wrapWasm", () => {
  it("masks a negative BigInt return back to an unsigned 64-bit value", () => {
    const wasm = fakeWasm({ valid_moves: vi.fn(() => -1n) });
    const api = wrapWasm(wasm);
    expect(api.validMoves(0n, 0n)).toBe((1n << 64n) - 1n);
  });

  it("masks flipMask and aiMove results the same way", () => {
    const wasm = fakeWasm({
      flip_mask: vi.fn(() => -8n),
      ai_move: vi.fn(() => -2n),
    });
    const api = wrapWasm(wasm);
    expect(api.flipMask(0n, 0n, 0n)).toBe((1n << 64n) - 1n - 7n);
    expect(api.aiMove(0n, 0n, 0)).toBe((1n << 64n) - 1n - 1n);
  });

  it("returns null from generateEndgame without reading the getters when generation fails", () => {
    const generatedBlack = vi.fn(() => 1n);
    const wasm = fakeWasm({
      generate_endgame: vi.fn(() => 0n),
      generated_black: generatedBlack,
    });
    const api = wrapWasm(wasm);
    expect(api.generateEndgame(1, 14)).toBeNull();
    expect(generatedBlack).not.toHaveBeenCalled();
  });

  it("reads black/white/margin from the getters when generation succeeds", () => {
    const wasm = fakeWasm({
      generate_endgame: vi.fn(() => 1n),
      generated_black: vi.fn(() => 5n),
      generated_white: vi.fn(() => 9n),
      generated_margin: vi.fn(() => 3n),
    });
    const api = wrapWasm(wasm);
    expect(api.generateEndgame(1, 14)).toEqual({
      black: 5n,
      white: 9n,
      margin: 3n,
    });
  });
});
