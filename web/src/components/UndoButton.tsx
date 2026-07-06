export interface UndoButtonProps {
  disabled: boolean;
  onUndo: () => void;
}

export function UndoButton({ disabled, onUndo }: UndoButtonProps) {
  return (
    <div className="undo-row">
      <button
        type="button"
        className="undo"
        disabled={disabled}
        onClick={onUndo}
      >
        Undo
      </button>
    </div>
  );
}
