import { useEffect, useReducer, useRef, useState } from "react";
import { bitAt } from "../game/bits";
import { sideToMove, reversiReducer, type GameAction } from "../game/reducer";
import { initialGameState, type GameState, type Turn } from "../game/types";
import { loadReversiWasm, type ReversiApi } from "../wasm/reversiWasm";

export interface UseReversiGame {
  state: GameState;
  loadError: string | null;
  canUndo: boolean;
  onHumanMove(index: number): void;
  newGame(color: Turn): void;
  newSprint(targetEmpties: number): void;
  undo(): void;
}

// Orchestrates the pure reducer against the wasm engine: computes wasm calls,
// dispatches their results, and schedules the same delays as the original
// step()/aiMove()/newSprint() flow (a pass message pauses for 700ms, the "AI
// is thinking" message for 350ms before the AI actually moves, and sprint
// generation for 50ms before the blocking wasm call).
export function useReversiGame(providedApi?: ReversiApi): UseReversiGame {
  const [state, dispatch] = useReducer(
    reversiReducer,
    initialGameState("black"),
  );
  // Mirrors `state`, but updated synchronously so timeout callbacks and
  // recursive step()-style calls always see the latest values without
  // waiting for a re-render (React state updates are deferred).
  const stateRef = useRef(state);
  const apiRef = useRef<ReversiApi | null>(providedApi ?? null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const timeoutsRef = useRef(new Set<ReturnType<typeof setTimeout>>());

  function schedule(fn: () => void, delayMs: number): void {
    const id = setTimeout(() => {
      timeoutsRef.current.delete(id);
      fn();
    }, delayMs);
    timeoutsRef.current.add(id);
  }

  // Starting a new game/sprint supersedes any in-flight AI-move or pass
  // continuation from whatever was happening before.
  function clearPending(): void {
    for (const id of timeoutsRef.current) clearTimeout(id);
    timeoutsRef.current.clear();
  }

  function applyAction(action: GameAction): void {
    stateRef.current = reversiReducer(stateRef.current, action);
    dispatch(action);
  }

  // Drive the game until it is the human's turn or the game ends.
  function runStep(): void {
    const api = apiRef.current;
    const s = stateRef.current;
    if (!api || s.gameOver) return;

    const [me, opp] = sideToMove(s);
    const legal = api.validMoves(me, opp);
    if (legal === 0n) {
      const otherLegal = api.validMoves(opp, me);
      if (otherLegal === 0n) {
        applyAction({ type: "FINISH" });
        return;
      }
      applyAction({ type: "PASS" });
      schedule(runStep, 700);
      return;
    }

    if (s.turn === s.humanColor) {
      applyAction({ type: "SHOW_YOUR_TURN", legalMoves: legal });
      return;
    }

    applyAction({ type: "START_AI_THINKING" });
    schedule(runAiMove, 350);
  }

  function runAiMove(): void {
    const api = apiRef.current;
    if (!api) return;
    const s = stateRef.current;
    const [me, opp] = sideToMove(s);
    const seed = Math.floor(Math.random() * 0x100000000);
    const bit = api.aiMove(me, opp, seed);
    const flip = bit !== 0n ? api.flipMask(me, opp, bit) : 0n;
    applyAction({ type: "APPLY_AI_MOVE", bit, flip });
    runStep();
  }

  function newGame(color: Turn): void {
    clearPending();
    applyAction({ type: "NEW_GAME", color });
    runStep();
  }

  function onHumanMove(index: number): void {
    const api = apiRef.current;
    const s = stateRef.current;
    if (!api || s.gameOver || s.busy || s.turn !== s.humanColor) return;
    const bit = bitAt(index);
    const [me, opp] = sideToMove(s);
    const flip = api.flipMask(me, opp, bit);
    if (flip === 0n) return; // illegal
    applyAction({ type: "APPLY_HUMAN_MOVE", index, flip });
    runStep();
  }

  // Rewind to the human's previous decision point, undoing their last move and
  // the AI reply that followed. Pure reducer state restore — no wasm needed, so
  // there's nothing to re-drive afterwards.
  function undo(): void {
    const s = stateRef.current;
    if (s.busy || s.history.length === 0) return;
    clearPending();
    applyAction({ type: "UNDO" });
  }

  function newSprint(targetEmpties: number): void {
    clearPending();
    applyAction({ type: "SPRINT_STARTED" });
    schedule(() => {
      const api = apiRef.current;
      if (!api) return;
      const seed = Math.floor(Math.random() * 0x100000000);
      const result = api.generateEndgame(seed, targetEmpties);
      if (!result) {
        applyAction({ type: "SPRINT_FAILED" });
        return;
      }
      // Sprint always hands the board to the human (black) to move, so compute
      // their legal moves here — the board only wires up clickable cells for
      // squares in legalMoves, and without it the game cannot be played.
      const legalMoves = api.validMoves(result.black, result.white);
      applyAction({
        type: "SPRINT_SUCCEEDED",
        black: result.black,
        white: result.white,
        margin: result.margin,
        legalMoves,
      });
    }, 50);
  }

  // Load (or accept the injected) wasm API once, then kick off the first game.
  useEffect(() => {
    if (providedApi) {
      newGame("black");
      return;
    }
    let cancelled = false;
    loadReversiWasm()
      .then((api) => {
        if (cancelled) return;
        apiRef.current = api;
        newGame("black");
      })
      .catch((e) => {
        if (cancelled) return;
        console.error(e);
        setLoadError(
          "Failed to load WASM. Please open this page via an HTTP server.",
        );
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const timeouts = timeoutsRef.current;
    return () => {
      for (const id of timeouts) clearTimeout(id);
      timeouts.clear();
    };
  }, []);

  return {
    state,
    loadError,
    canUndo: state.history.length > 0,
    onHumanMove,
    newGame,
    newSprint,
    undo,
  };
}
