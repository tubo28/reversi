import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Controls } from "./Controls";

describe("Controls", () => {
  it("calls onNewGame with the chosen color", async () => {
    const user = userEvent.setup();
    const onNewGame = vi.fn();
    render(
      <Controls
        disabled={false}
        onNewGame={onNewGame}
        onNewSprint={() => {}}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Black" }));
    expect(onNewGame).toHaveBeenCalledWith("black");
    await user.click(screen.getByRole("button", { name: "White" }));
    expect(onNewGame).toHaveBeenCalledWith("white");
  });

  it("calls onNewSprint when clicking Sprint", async () => {
    const user = userEvent.setup();
    const onNewSprint = vi.fn();
    render(
      <Controls
        disabled={false}
        onNewGame={() => {}}
        onNewSprint={onNewSprint}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Sprint" }));
    expect(onNewSprint).toHaveBeenCalledWith(14);
  });

  it("disables all buttons when disabled", () => {
    render(<Controls disabled onNewGame={() => {}} onNewSprint={() => {}} />);
    expect(screen.getByRole("button", { name: "Black" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "White" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Sprint" })).toBeDisabled();
  });
});
