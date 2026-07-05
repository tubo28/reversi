export interface ScoreBoardProps {
  blackCount: number;
  whiteCount: number;
}

export function ScoreBoard({ blackCount, whiteCount }: ScoreBoardProps) {
  return (
    <div className="scores">
      <span className="score">
        <span className="disk black" />
        <span id="black-count">{blackCount}</span>
      </span>
      <span className="score">
        <span className="disk white" />
        <span id="white-count">{whiteCount}</span>
      </span>
    </div>
  );
}
