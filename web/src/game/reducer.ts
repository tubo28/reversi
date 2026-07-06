import { bitAt, popcount } from "./bits";
import {
  initialGameState,
  label,
  other,
  type GameState,
  type Snapshot,
  type Turn,
  type Winner,
} from "./types";

export type GameAction =
  | { type: "NEW_GAME"; color: Turn }
  | { type: "APPLY_HUMAN_MOVE"; index: number; flip: bigint }
  | { type: "APPLY_AI_MOVE"; bit: bigint; flip: bigint }
  | { type: "PASS" }
  | { type: "FINISH" }
  | { type: "SHOW_YOUR_TURN"; legalMoves: bigint }
  | { type: "START_AI_THINKING" }
  | { type: "UNDO" }
  | { type: "SPRINT_STARTED" }
  | { type: "SPRINT_FAILED" }
  | {
      type: "SPRINT_SUCCEEDED";
      black: bigint;
      white: bigint;
      margin: bigint;
      legalMoves: bigint;
    };

// [me, opp] disks for the side to move.
export function sideToMove(state: GameState): [bigint, bigint] {
  return state.turn === "black"
    ? [state.black, state.white]
    : [state.white, state.black];
}

// The board fields worth restoring on undo.
function snapshot(state: GameState): Snapshot {
  return {
    black: state.black,
    white: state.white,
    turn: state.turn,
    lastMove: state.lastMove,
    legalMoves: state.legalMoves,
    status: state.status,
  };
}

export function reversiReducer(
  state: GameState,
  action: GameAction,
): GameState {
  switch (action.type) {
    case "NEW_GAME":
      return initialGameState(action.color);

    case "APPLY_HUMAN_MOVE": {
      const bit = bitAt(action.index);
      const [me, opp] = sideToMove(state);
      const newMe = me | bit | action.flip;
      const newOpp = opp ^ action.flip;
      const [black, white] =
        state.turn === "black" ? [newMe, newOpp] : [newOpp, newMe];
      return {
        ...state,
        black,
        white,
        lastMove: action.index,
        turn: other(state.turn),
        // Remember where the human moved from, so an undo can rewind this move
        // (and the AI reply that follows) back to this decision point.
        history: [...state.history, snapshot(state)],
      };
    }

    case "APPLY_AI_MOVE": {
      if (action.bit === 0n) {
        return { ...state, turn: other(state.turn), busy: false };
      }
      const [me, opp] = sideToMove(state);
      const newMe = me | action.bit | action.flip;
      const newOpp = opp ^ action.flip;
      const [black, white] =
        state.turn === "black" ? [newMe, newOpp] : [newOpp, newMe];
      const index = action.bit.toString(2).length - 1;
      return {
        ...state,
        black,
        white,
        lastMove: index,
        turn: other(state.turn),
        busy: false,
      };
    }

    case "PASS":
      return {
        ...state,
        status: `${label(state.turn)} passed`,
        turn: other(state.turn),
      };

    case "FINISH": {
      const b = popcount(state.black);
      const w = popcount(state.white);
      const winner: Winner = b > w ? "black" : b < w ? "white" : "draw";
      const result =
        winner === "draw"
          ? "Draw"
          : winner === state.humanColor
            ? "You win! 🎉"
            : "AI wins";
      return {
        ...state,
        gameOver: true,
        winner,
        status: `Game over — ${result} (Black ${b} : White ${w})`,
      };
    }

    case "SHOW_YOUR_TURN":
      return {
        ...state,
        status: `Your turn (${label(state.turn)})`,
        legalMoves: action.legalMoves,
      };

    case "UNDO": {
      if (state.history.length === 0) return state;
      const prev = state.history[state.history.length - 1];
      return {
        ...state,
        black: prev.black,
        white: prev.white,
        turn: prev.turn,
        lastMove: prev.lastMove,
        legalMoves: prev.legalMoves,
        status: prev.status,
        gameOver: false,
        winner: null,
        busy: false,
        history: state.history.slice(0, -1),
      };
    }

    case "START_AI_THINKING":
      return { ...state, busy: true, status: "AI is thinking…" };

    case "SPRINT_STARTED":
      return {
        ...state,
        busy: true,
        gameOver: false,
        winner: null,
        lastMove: -1,
        status: "Generating…",
      };

    case "SPRINT_FAILED":
      return {
        ...state,
        busy: false,
        status: "Generation failed. Please try again.",
      };

    case "SPRINT_SUCCEEDED":
      return {
        ...state,
        black: action.black,
        white: action.white,
        turn: "black",
        humanColor: "black",
        busy: false,
        gameOver: false,
        winner: null,
        lastMove: -1,
        legalMoves: action.legalMoves,
        sprint: true,
        history: [],
        status: `Your turn (${label("black")})\nTHIS IS A WINNING POSITION. MAKE OPTIMAL MOVES TO WIN.`,
      };

    default:
      return state;
  }
}
