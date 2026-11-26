interface Props {
  label: string;
  value: string;
}

export function Metric({ label, value }: Props) {
  return (
    <div className="min-w-0 rounded border border-gray-700 p-1.5">
      <div className="text-[10px] text-gray-400">{label}</div>
      <div className="truncate font-semibold text-white">{value}</div>
    </div>
  );
}
