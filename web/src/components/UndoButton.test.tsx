import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { UndoButton } from "./UndoButton";

describe("UndoButton", () => {
  it("calls onUndo when clicked", async () => {
    const user = userEvent.setup();
    const onUndo = vi.fn();
    render(<UndoButton disabled={false} onUndo={onUndo} />);
    await user.click(screen.getByRole("button", { name: "Undo" }));
    expect(onUndo).toHaveBeenCalledTimes(1);
  });

  it("is disabled when disabled is true", () => {
    render(<UndoButton disabled onUndo={() => {}} />);
    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();
  });
});
