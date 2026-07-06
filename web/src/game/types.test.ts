import { describe, expect, it } from "vitest";
import { isSet } from "./bits";
import {
  START_BLACK,
  START_WHITE,
  initialGameState,
  label,
  other,
} from "./types";

describe("other", () => {
  it("flips black to white and back", () => {
    expect(other("black")).toBe("white");
    expect(other("white")).toBe("black");
  });
});

describe("label", () => {
  it("capitalizes the side name", () => {
    expect(label("black")).toBe("Black");
    expect(label("white")).toBe("White");
  });
});

describe("starting position", () => {
  it("places black on (3,4) and (4,3)", () => {
    expect(isSet(START_BLACK, 28)).toBe(true);
    expect(isSet(START_BLACK, 35)).toBe(true);
  });

  it("places white on (3,3) and (4,4)", () => {
    expect(isSet(START_WHITE, 27)).toBe(true);
    expect(isSet(START_WHITE, 36)).toBe(true);
  });
});

describe("initialGameState", () => {
  it("starts black to move regardless of the human's chosen color", () => {
    const state = initialGameState("white");
    expect(state.turn).toBe("black");
    expect(state.humanColor).toBe("white");
    expect(state.black).toBe(START_BLACK);
    expect(state.white).toBe(START_WHITE);
    expect(state.gameOver).toBe(false);
    expect(state.busy).toBe(false);
    expect(state.lastMove).toBe(-1);
  });
});
