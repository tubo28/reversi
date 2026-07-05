import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Controls } from "./Controls";

describe("Controls", () => {
  it("calls onNewGame with the chosen color", async () => {
    const user = userEvent.setup();
    const onNewGame = vi.fn();
    render(<Controls disabled={false} onNewGame={onNewGame} onNewSprint={() => {}} />);
    await user.click(screen.getByRole("button", { name: "New Game (Black)" }));
    expect(onNewGame).toHaveBeenCalledWith("black");
    await user.click(screen.getByRole("button", { name: "New Game (White)" }));
    expect(onNewGame).toHaveBeenCalledWith("white");
  });

  it("calls onNewSprint with the selected empties option", async () => {
    const user = userEvent.setup();
    const onNewSprint = vi.fn();
    render(<Controls disabled={false} onNewGame={() => {}} onNewSprint={onNewSprint} />);
    await user.selectOptions(screen.getByLabelText("Sprint difficulty"), "16");
    await user.click(screen.getByRole("button", { name: "必勝局面を生成" }));
    expect(onNewSprint).toHaveBeenCalledWith(16);
  });

  it("defaults the sprint difficulty to 14 empties", async () => {
    const user = userEvent.setup();
    const onNewSprint = vi.fn();
    render(<Controls disabled={false} onNewGame={() => {}} onNewSprint={onNewSprint} />);
    await user.click(screen.getByRole("button", { name: "必勝局面を生成" }));
    expect(onNewSprint).toHaveBeenCalledWith(14);
  });

  it("disables all buttons when disabled", () => {
    render(<Controls disabled onNewGame={() => {}} onNewSprint={() => {}} />);
    expect(screen.getByRole("button", { name: "New Game (Black)" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "New Game (White)" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "必勝局面を生成" })).toBeDisabled();
  });
});
