import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusLine } from "./StatusLine";

describe("StatusLine", () => {
  it("renders the given text verbatim", () => {
    render(<StatusLine text="Your turn (Black)" />);
    expect(screen.getByText("Your turn (Black)")).toBeInTheDocument();
  });
});
