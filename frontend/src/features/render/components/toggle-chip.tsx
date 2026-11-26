interface Props {
  on: boolean;
  label: string;
  onClick: () => void;
}

export function ToggleChip({ on, label, onClick }: Props) {
  return (
    <button
      onClick={onClick}
      className={`rounded border px-1.5 py-0.5 ${
        on
          ? "border-gray-400 bg-gray-700 text-white"
          : "border-gray-700 text-gray-300 hover:border-gray-500"
      }`}
    >
      {label}
    </button>
  );
}
