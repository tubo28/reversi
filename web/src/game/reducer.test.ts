import { describe, expect, it } from "vitest";
import { bitAt, popcount } from "./bits";
import { reversiReducer, sideToMove, type GameAction } from "./reducer";
import { START_BLACK, START_WHITE, initialGameState, type GameState } from "./types";

const dispatch = (state: GameState, action: GameAction): GameState => reversiReducer(state, action);

describe("NEW_GAME", () => {
  it("resets to 4 starting disks with black to move", () => {
    const state = dispatch(initialGameState("black"), { type: "NEW_GAME", color: "black" });
    expect(state.black).toBe(START_BLACK);
    expect(state.white).toBe(START_WHITE);
    expect(state.turn).toBe("black");
    expect(state.humanColor).toBe("black");
    expect(state.gameOver).toBe(false);
    expect(state.busy).toBe(false);
    expect(state.lastMove).toBe(-1);
  });

  it("still starts with black to move when the human plays white", () => {
    const state = dispatch(initialGameState("black"), { type: "NEW_GAME", color: "white" });
    expect(state.turn).toBe("black");
    expect(state.humanColor).toBe("white");
  });
});

describe("sideToMove", () => {
  it("returns [black, white] when it's black's turn", () => {
    const state = initialGameState("black");
    expect(sideToMove(state)).toEqual([state.black, state.white]);
  });

  it("returns [white, black] when it's white's turn", () => {
    const state = { ...initialGameState("black"), turn: "white" as const };
    expect(sideToMove(state)).toEqual([state.white, state.black]);
  });
});

describe("APPLY_HUMAN_MOVE", () => {
  it("flips the correct discs and switches turn", () => {
    const state: GameState = {
      ...initialGameState("black"),
      turn: "black",
    };
    const next = dispatch(state, { type: "APPLY_HUMAN_MOVE", index: 20, flip: bitAt(27) });
    expect(next.black).toBe(bitAt(20) | bitAt(27) | bitAt(28) | bitAt(35));
    expect(next.white).toBe(bitAt(36));
    expect(next.turn).toBe("white");
    expect(next.lastMove).toBe(20);
  });
});

describe("APPLY_AI_MOVE", () => {
  it("with bit=0n leaves the board unchanged but still flips turn and clears busy", () => {
    const state: GameState = { ...initialGameState("black"), turn: "white", busy: true, lastMove: 5 };
    const next = dispatch(state, { type: "APPLY_AI_MOVE", bit: 0n, flip: 0n });
    expect(next.black).toBe(state.black);
    expect(next.white).toBe(state.white);
    expect(next.lastMove).toBe(5);
    expect(next.turn).toBe("black");
    expect(next.busy).toBe(false);
  });

  it("with a nonzero bit applies the move, switches turn and clears busy", () => {
    const state: GameState = { ...initialGameState("black"), turn: "white", busy: true };
    const next = dispatch(state, { type: "APPLY_AI_MOVE", bit: bitAt(20), flip: bitAt(28) });
    expect(next.white).toBe(bitAt(20) | bitAt(28) | bitAt(27) | bitAt(36));
    expect(next.black).toBe(bitAt(35));
    expect(next.lastMove).toBe(20);
    expect(next.turn).toBe("black");
    expect(next.busy).toBe(false);
  });
});

describe("PASS", () => {
  it("reports the current side passing, then switches turn", () => {
    const state: GameState = { ...initialGameState("black"), turn: "black" };
    const next = dispatch(state, { type: "PASS" });
    expect(next.status).toBe("Black passed");
    expect(next.turn).toBe("white");
  });

  it("works symmetrically for white", () => {
    const state: GameState = { ...initialGameState("black"), turn: "white" };
    const next = dispatch(state, { type: "PASS" });
    expect(next.status).toBe("White passed");
    expect(next.turn).toBe("black");
  });
});

describe("FINISH", () => {
  const black3 = bitAt(0) | bitAt(1) | bitAt(2);
  const white1 = bitAt(3);

  it("declares the human the winner from their perspective", () => {
    const state: GameState = { ...initialGameState("black"), black: black3, white: white1 };
    const next = dispatch(state, { type: "FINISH" });
    expect(next.gameOver).toBe(true);
    expect(next.status).toBe("Game over — You win! 🎉 (Black 3 : White 1)");
  });

  it("declares the AI the winner when the human has fewer disks", () => {
    const state: GameState = { ...initialGameState("white"), black: black3, white: white1 };
    const next = dispatch(state, { type: "FINISH" });
    expect(next.status).toBe("Game over — AI wins (Black 3 : White 1)");
  });

  it("declares a draw when counts are equal", () => {
    const state: GameState = { ...initialGameState("black"), black: START_BLACK, white: START_WHITE };
    expect(popcount(state.black)).toBe(popcount(state.white));
    const next = dispatch(state, { type: "FINISH" });
    expect(next.status).toBe("Game over — Draw (Black 2 : White 2)");
  });
});

describe("SHOW_YOUR_TURN", () => {
  it("shows the current side's turn and records the legal-move mask", () => {
    const state: GameState = { ...initialGameState("black"), turn: "black" };
    const next = dispatch(state, { type: "SHOW_YOUR_TURN", legalMoves: bitAt(20) | bitAt(29) });
    expect(next.status).toBe("Your turn (Black)");
    expect(next.legalMoves).toBe(bitAt(20) | bitAt(29));
  });
});

describe("START_AI_THINKING", () => {
  it("sets busy and the thinking status", () => {
    const next = dispatch(initialGameState("black"), { type: "START_AI_THINKING" });
    expect(next.busy).toBe(true);
    expect(next.status).toBe("AI is thinking…");
  });
});

describe("sprint actions", () => {
  it("SPRINT_STARTED marks busy and the generating status", () => {
    const state: GameState = { ...initialGameState("black"), gameOver: true, lastMove: 9 };
    const next = dispatch(state, { type: "SPRINT_STARTED" });
    expect(next.busy).toBe(true);
    expect(next.gameOver).toBe(false);
    expect(next.lastMove).toBe(-1);
    expect(next.status).toBe("Generating…");
  });

  it("SPRINT_FAILED clears busy and shows the failure message", () => {
    const state: GameState = { ...initialGameState("black"), busy: true };
    const next = dispatch(state, { type: "SPRINT_FAILED" });
    expect(next.busy).toBe(false);
    expect(next.status).toBe("Generation failed. Please try again.");
  });

  it("SPRINT_SUCCEEDED sets the generated board and the win-margin status", () => {
    const state: GameState = { ...initialGameState("white"), busy: true, gameOver: true, lastMove: 4 };
    const next = dispatch(state, {
      type: "SPRINT_SUCCEEDED",
      black: bitAt(10) | bitAt(11),
      white: bitAt(12),
      margin: 6n,
      legalMoves: bitAt(20) | bitAt(21),
    });
    expect(next.black).toBe(bitAt(10) | bitAt(11));
    expect(next.white).toBe(bitAt(12));
    expect(next.turn).toBe("black");
    expect(next.humanColor).toBe("black");
    expect(next.busy).toBe(false);
    expect(next.gameOver).toBe(false);
    expect(next.lastMove).toBe(-1);
    expect(next.legalMoves).toBe(bitAt(20) | bitAt(21));
    expect(next.status).toBe("YOUR TURN (MAKE OPTIMAL MOVES)");
  });
});
