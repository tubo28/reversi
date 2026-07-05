import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ScoreBoard } from "./ScoreBoard";

describe("ScoreBoard", () => {
  it("renders the given black and white counts", () => {
    render(<ScoreBoard blackCount={5} whiteCount={3} />);
    expect(screen.getByText("5")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });
});
