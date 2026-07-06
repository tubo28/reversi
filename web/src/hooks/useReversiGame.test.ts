import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bitAt } from "../game/bits";
import type { ReversiApi } from "../wasm/reversiWasm";
import { useReversiGame } from "./useReversiGame";

function fakeApi(overrides: Partial<ReversiApi> = {}): ReversiApi {
  return {
    validMoves: vi.fn(() => 1n),
    flipMask: vi.fn(() => 1n),
    aiMove: vi.fn(() => 0n),
    generateEndgame: vi.fn(() => null),
    ...overrides,
  };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useReversiGame", () => {
  it("auto-starts a new game as black with black to move", () => {
    const { result } = renderHook(() => useReversiGame(fakeApi()));
    expect(result.current.state.turn).toBe("black");
    expect(result.current.state.humanColor).toBe("black");
    expect(result.current.state.status).toBe("Your turn (Black)");
  });

  it("drives a human move through to the AI's response", () => {
    const api = fakeApi();
    const { result } = renderHook(() => useReversiGame(api));

    act(() => {
      result.current.onHumanMove(20);
    });
    expect(result.current.state.turn).toBe("white");
    expect(result.current.state.busy).toBe(true);
    expect(result.current.state.status).toBe("AI is thinking…");

    act(() => {
      vi.advanceTimersByTime(350);
    });
    expect(result.current.state.turn).toBe("black");
    expect(result.current.state.busy).toBe(false);
    expect(result.current.state.status).toBe("Your turn (Black)");
  });

  it("ignores an illegal human move (flipMask returns 0)", () => {
    const api = fakeApi({ flipMask: vi.fn(() => 0n) });
    const { result } = renderHook(() => useReversiGame(api));
    const before = result.current.state;

    act(() => {
      result.current.onHumanMove(20);
    });
    expect(result.current.state).toEqual(before);
  });

  it("shows a pass message, then re-evaluates after the pause", () => {
    const validMoves = vi
      .fn()
      .mockImplementationOnce(() => 0n) // black (to move) has no legal moves
      .mockImplementationOnce(() => 1n) // white (opponent) does -> pass, not finish
      .mockImplementation(() => 1n); // subsequent checks: keep the game going
    const api = fakeApi({ validMoves });
    const { result } = renderHook(() => useReversiGame(api));

    expect(result.current.state.status).toBe("Black passed");
    expect(result.current.state.turn).toBe("white");

    act(() => {
      vi.advanceTimersByTime(700);
    });
    // It's now White's (the AI's) turn, and re-evaluation should have kicked off thinking.
    expect(result.current.state.turn).toBe("white");
    expect(result.current.state.busy).toBe(true);
    expect(result.current.state.status).toBe("AI is thinking…");
  });

  it("finishes the game when neither side has a legal move", () => {
    const api = fakeApi({ validMoves: vi.fn(() => 0n) });
    const { result } = renderHook(() => useReversiGame(api));

    expect(result.current.state.gameOver).toBe(true);
    expect(result.current.state.status).toMatch(/^Game over/);
  });

  it("newSprint shows the generating status, then success with legal moves ready to play", () => {
    const api = fakeApi({
      generateEndgame: vi.fn(() => ({ black: bitAt(1), white: bitAt(2), margin: 4n })),
      validMoves: vi.fn(() => bitAt(20)),
    });
    const { result } = renderHook(() => useReversiGame(api));

    act(() => {
      result.current.newSprint(14);
    });
    expect(result.current.state.busy).toBe(true);
    expect(result.current.state.status).toBe("Generating…");

    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(result.current.state.busy).toBe(false);
    expect(result.current.state.black).toBe(bitAt(1));
    expect(result.current.state.white).toBe(bitAt(2));
    // The human (black) must have their legal moves populated so the board is
    // playable; otherwise no cell is clickable and the game cannot progress.
    expect(result.current.state.legalMoves).toBe(bitAt(20));
    expect(result.current.state.status).toBe("YOUR TURN (MAKE OPTIMAL MOVES)");
  });

  it("newSprint shows the failure message when generation fails", () => {
    const api = fakeApi({ generateEndgame: vi.fn(() => null) });
    const { result } = renderHook(() => useReversiGame(api));

    act(() => {
      result.current.newSprint(14);
    });
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(result.current.state.busy).toBe(false);
    expect(result.current.state.status).toBe("Generation failed. Please try again.");
  });
});
