export interface StatusLineProps {
  text: string;
}

export function StatusLine({ text }: StatusLineProps) {
  return <p className="status">{text}</p>;
}
