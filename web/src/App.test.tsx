import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import type { ReversiApi } from "./wasm/reversiWasm";

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

describe("App", () => {
  it("renders the initial board with 4 disks and correct counts", () => {
    const { container } = render(<App api={fakeApi()} />);
    expect(container.querySelectorAll(".cell")).toHaveLength(64);
    expect(container.querySelector("#black-count")?.textContent).toBe("2");
    expect(container.querySelector("#white-count")?.textContent).toBe("2");
    expect(screen.getByText("Your turn (Black)")).toBeInTheDocument();
  });

  it("plays a full human->AI round trip after starting a new game as White", () => {
    render(<App api={fakeApi()} />);

    fireEvent.click(screen.getByRole("button", { name: "White" }));
    // Black moves first (it's black's turn but the human chose White), so the AI goes first.
    expect(screen.getByText("AI is thinking…")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(350);
    });
    expect(screen.getByText("Your turn (White)")).toBeInTheDocument();
  });
});
