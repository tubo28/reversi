import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { bitAt } from "../game/bits";
import { START_BLACK, START_WHITE } from "../game/types";
import { Board } from "./Board";

describe("Board", () => {
  it("renders 64 cells", () => {
    render(
      <Board
        black={START_BLACK}
        white={START_WHITE}
        legalMoves={0n}
        lastMove={-1}
        interactive={false}
        onCellClick={() => {}}
      />,
    );
    expect(screen.getAllByRole("button")).toHaveLength(64);
  });

  it("shows a marker only on the last-move disk", () => {
    const { container } = render(
      <Board
        black={START_BLACK}
        white={START_WHITE}
        legalMoves={0n}
        lastMove={28}
        interactive={false}
        onCellClick={() => {}}
      />,
    );
    const cells = container.querySelectorAll(".cell");
    expect(cells[28].querySelector(".marker")).not.toBeNull();
    expect(cells[35].querySelector(".marker")).toBeNull();
  });

  it("marks only empty legal cells as playable when interactive", () => {
    const legalMoves = bitAt(20) | bitAt(29);
    const { container } = render(
      <Board
        black={START_BLACK}
        white={START_WHITE}
        legalMoves={legalMoves}
        lastMove={-1}
        interactive
        onCellClick={() => {}}
      />,
    );
    const cells = container.querySelectorAll(".cell");
    expect(cells[20].classList.contains("playable")).toBe(true);
    expect(cells[29].classList.contains("playable")).toBe(true);
    expect(cells[21].classList.contains("playable")).toBe(false);
  });

  it("does not mark legal cells as playable when not interactive", () => {
    const legalMoves = bitAt(20);
    const { container } = render(
      <Board
        black={START_BLACK}
        white={START_WHITE}
        legalMoves={legalMoves}
        lastMove={-1}
        interactive={false}
        onCellClick={() => {}}
      />,
    );
    const cells = container.querySelectorAll(".cell");
    expect(cells[20].classList.contains("playable")).toBe(false);
  });

  it("calls onCellClick with the clicked cell's index", async () => {
    const user = userEvent.setup();
    const onCellClick = vi.fn();
    const legalMoves = bitAt(20);
    const { container } = render(
      <Board
        black={START_BLACK}
        white={START_WHITE}
        legalMoves={legalMoves}
        lastMove={-1}
        interactive
        onCellClick={onCellClick}
      />,
    );
    await user.click(container.querySelectorAll(".cell")[20]);
    expect(onCellClick).toHaveBeenCalledWith(20);
  });
});
