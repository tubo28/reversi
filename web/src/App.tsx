import { popcount } from "./game/bits";
import { Board } from "./components/Board";
import { Controls } from "./components/Controls";
import { ScoreBoard } from "./components/ScoreBoard";
import { StatusLine } from "./components/StatusLine";
import { useReversiGame } from "./hooks/useReversiGame";
import type { ReversiApi } from "./wasm/reversiWasm";

export interface AppProps {
  // Test-only injection point; the real app always loads the wasm module itself.
  api?: ReversiApi;
}

export function App({ api }: AppProps = {}) {
  const { state, loadError, onHumanMove, newGame, newSprint } = useReversiGame(api);
  const interactive = !state.gameOver && !state.busy && state.turn === state.humanColor;

  return (
    <main className="app">
      <Controls disabled={false} onNewGame={newGame} onNewSprint={newSprint} />
      <ScoreBoard blackCount={popcount(state.black)} whiteCount={popcount(state.white)} />
      <Board
        black={state.black}
        white={state.white}
        legalMoves={state.legalMoves}
        lastMove={state.lastMove}
        interactive={interactive}
        onCellClick={onHumanMove}
      />
      <StatusLine text={loadError ?? state.status} />
    </main>
  );
}
