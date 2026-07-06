import { useState } from "react";
import type { Side } from "../game/types";

export interface ControlsProps {
  disabled: boolean;
  onNewGame: (color: Side) => void;
  onNewSprint: (targetEmpties: number) => void;
}

export function Controls({ disabled, onNewGame, onNewSprint }: ControlsProps) {
  const [sprintEmpties, setSprintEmpties] = useState(14);

  return (
    <>
      <div className="controls">
        <button type="button" disabled={disabled} onClick={() => onNewGame("black")}>
          New Game (Black)
        </button>
        <button type="button" disabled={disabled} onClick={() => onNewGame("white")}>
          New Game (White)
        </button>
      </div>

      <div className="controls">
        <select
          aria-label="Sprint difficulty"
          value={sprintEmpties}
          onChange={(e) => setSprintEmpties(Number(e.target.value))}
        >
          <option value={12}>Empties 12 (easy)</option>
          <option value={14}>Empties 14</option>
          <option value={16}>Empties 16 (hard)</option>
        </select>
        <button type="button" disabled={disabled} onClick={() => onNewSprint(sprintEmpties)}>
          Generate Winning Position
        </button>
      </div>
    </>
  );
}
