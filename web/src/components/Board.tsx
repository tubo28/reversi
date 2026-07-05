import { isSet } from "../game/bits";

export interface BoardProps {
  black: bigint;
  white: bigint;
  legalMoves: bigint;
  lastMove: number;
  interactive: boolean;
  onCellClick: (index: number) => void;
}

export function Board({ black, white, legalMoves, lastMove, interactive, onCellClick }: BoardProps) {
  const cells = [];
  for (let index = 0; index < 64; index++) {
    const hasBlack = isSet(black, index);
    const hasWhite = isSet(white, index);
    const playable = interactive && !hasBlack && !hasWhite && isSet(legalMoves, index);

    cells.push(
      <button
        key={index}
        type="button"
        className={playable ? "cell playable" : "cell"}
        onClick={playable ? () => onCellClick(index) : undefined}
      >
        {hasBlack || hasWhite ? (
          <span className={hasBlack ? "disk black" : "disk white"}>
            {index === lastMove ? <span className="marker" /> : null}
          </span>
        ) : playable ? (
          <span className="hint" />
        ) : null}
      </button>,
    );
  }

  return (
    <div id="board" className="board" aria-label="Board">
      {cells}
    </div>
  );
}
