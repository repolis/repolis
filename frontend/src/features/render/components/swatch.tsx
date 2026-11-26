interface Props {
  color: string;
  label: string;
}

export function Swatch({ color, label }: Props) {
  return (
    <span className="flex items-center gap-1 rounded bg-gray-800 px-1.5 py-0.5">
      <span
        className="inline-block h-2 w-2 rounded-sm"
        style={{ background: color }}
      />
      {label}
    </span>
  );
}
