import type { ReactNode } from "react";

interface Props {
  title: string;
  children: ReactNode;
}

export function Section({ title, children }: Props) {
  return (
    <div className="flex min-w-0 flex-col gap-1 border-t border-gray-800 pt-2">
      <div className="font-semibold text-gray-400">{title}</div>
      {children}
    </div>
  );
}
