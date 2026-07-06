import { bitAt, popcount } from "./bits";
import { initialGameState, label, other, type GameState, type Side } from "./types";

export type GameAction =
  | { type: "NEW_GAME"; color: Side }
  | { type: "APPLY_HUMAN_MOVE"; index: number; flip: bigint }
  | { type: "APPLY_AI_MOVE"; bit: bigint; flip: bigint }
  | { type: "PASS" }
  | { type: "FINISH" }
  | { type: "SHOW_YOUR_TURN"; legalMoves: bigint }
  | { type: "START_AI_THINKING" }
  | { type: "SPRINT_STARTED" }
  | { type: "SPRINT_FAILED" }
  | { type: "SPRINT_SUCCEEDED"; black: bigint; white: bigint; margin: bigint; legalMoves: bigint };

// [me, opp] disks for the side to move.
export function sideToMove(state: GameState): [bigint, bigint] {
  return state.turn === "black" ? [state.black, state.white] : [state.white, state.black];
}

export function reversiReducer(state: GameState, action: GameAction): GameState {
  switch (action.type) {
    case "NEW_GAME":
      return initialGameState(action.color);

    case "APPLY_HUMAN_MOVE": {
      const bit = bitAt(action.index);
      const [me, opp] = sideToMove(state);
      const newMe = me | bit | action.flip;
      const newOpp = opp ^ action.flip;
      const [black, white] = state.turn === "black" ? [newMe, newOpp] : [newOpp, newMe];
      return { ...state, black, white, lastMove: action.index, turn: other(state.turn) };
    }

    case "APPLY_AI_MOVE": {
      if (action.bit === 0n) {
        return { ...state, turn: other(state.turn), busy: false };
      }
      const [me, opp] = sideToMove(state);
      const newMe = me | action.bit | action.flip;
      const newOpp = opp ^ action.flip;
      const [black, white] = state.turn === "black" ? [newMe, newOpp] : [newOpp, newMe];
      const index = action.bit.toString(2).length - 1;
      return { ...state, black, white, lastMove: index, turn: other(state.turn), busy: false };
    }

    case "PASS":
      return { ...state, status: `${label(state.turn)} passed`, turn: other(state.turn) };

    case "FINISH": {
      const b = popcount(state.black);
      const w = popcount(state.white);
      const humanCount = state.humanColor === "black" ? b : w;
      const aiCount = state.humanColor === "black" ? w : b;
      const result = humanCount > aiCount ? "You win! 🎉" : humanCount < aiCount ? "AI wins" : "Draw";
      return { ...state, gameOver: true, status: `Game over — ${result} (Black ${b} : White ${w})` };
    }

    case "SHOW_YOUR_TURN":
      return { ...state, status: `Your turn (${label(state.turn)})`, legalMoves: action.legalMoves };

    case "START_AI_THINKING":
      return { ...state, busy: true, status: "AI is thinking…" };

    case "SPRINT_STARTED":
      return { ...state, busy: true, gameOver: false, lastMove: -1, status: "Generating…" };

    case "SPRINT_FAILED":
      return { ...state, busy: false, status: "Generation failed. Please try again." };

    case "SPRINT_SUCCEEDED":
      return {
        ...state,
        black: action.black,
        white: action.white,
        turn: "black",
        humanColor: "black",
        busy: false,
        gameOver: false,
        lastMove: -1,
        legalMoves: action.legalMoves,
        status: `YOUR TURN (MAKE OPTIMAL MOVES)`,
      };

    default:
      return state;
  }
}
