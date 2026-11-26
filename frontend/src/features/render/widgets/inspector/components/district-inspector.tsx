import type { DistrictInfo } from "../../../types";
import { TYPOLOGY_COLORS } from "../../../utils/colors";
import { Chips } from "./chips";
import { Metric } from "./metric";
import { Section } from "./section";

interface Props {
  info: DistrictInfo;
  onPick: (n: string) => void;
}

export function DistrictInspector({ info, onPick }: Props) {
  return (
    <div className="flex min-w-0 flex-col gap-3">
      <div className="min-w-0 border-b border-gray-700 pr-5 pb-2">
        <div className="flex items-center gap-2 text-[11px] text-gray-400">
          <span
            className="inline-block h-2 w-2 rounded-sm"
            style={{
              background:
                TYPOLOGY_COLORS[info.typology] ?? TYPOLOGY_COLORS.unknown,
            }}
          />
          <span>District, {info.typology}</span>
        </div>
        <div className="mt-1 text-sm font-bold break-all text-white">
          {info.name}
        </div>
        {info.summary && (
          <div className="mt-1 text-gray-300">{info.summary}</div>
        )}
      </div>

      <div className="grid grid-cols-3 gap-2 text-center">
        <Metric label="Entities" value={String(info.building_count)} />
        <Metric label="Functions" value={String(info.total_methods)} />
        <Metric
          label="Lines"
          value={info.total_lines_of_code.toLocaleString()}
        />
      </div>

      {info.tags.length > 0 && (
        <Section title="Tags">
          <Chips items={info.tags} limit={6} />
        </Section>
      )}
      {info.top_buildings.length > 0 && (
        <Section title="Largest entities">
          <Chips items={info.top_buildings} limit={8} onPick={onPick} />
        </Section>
      )}
    </div>
  );
}
