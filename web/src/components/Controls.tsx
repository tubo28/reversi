import type { Turn } from "../game/types";

const SPRINT_EMPTIES = 14;

export interface ControlsProps {
  disabled: boolean;
  onNewGame: (color: Turn) => void;
  onNewSprint: (targetEmpties: number) => void;
}

export function Controls({ disabled, onNewGame, onNewSprint }: ControlsProps) {
  return (
    <div className="controls">
      <h2 className="controls-title">New Game</h2>
      <div className="controls-buttons">
        <button
          type="button"
          disabled={disabled}
          onClick={() => onNewGame("black")}
        >
          Black
        </button>
        <button
          type="button"
          disabled={disabled}
          onClick={() => onNewGame("white")}
        >
          White
        </button>
        <button
          type="button"
          disabled={disabled}
          onClick={() => onNewSprint(SPRINT_EMPTIES)}
        >
          Sprint
        </button>
      </div>
    </div>
  );
}
