import {
  Cell,
  Legend,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
} from "recharts";
import type { DashboardAmmoCaliberRow } from "../../types";

const SLICE_COLORS = [
  "#2563eb",
  "#7c3aed",
  "#059669",
  "#d97706",
  "#dc2626",
  "#64748b",
  "#db2777",
  "#0d9488",
];

type ChartRow = { name: string; value: number };

export function AmmoCaliberDonut({
  rows,
}: {
  rows: DashboardAmmoCaliberRow[];
}) {
  if (rows.length === 0) {
    return <p className="muted dashboard-widget-empty">No ammunition in inventory.</p>;
  }

  const data: ChartRow[] = rows.map((r) => ({
    name: r.caliber,
    value: r.rounds,
  }));

  return (
    <div className="dashboard-donut-wrap">
      <ResponsiveContainer width="100%" height={220}>
        <PieChart>
          <Pie
            data={data}
            dataKey="value"
            nameKey="name"
            cx="50%"
            cy="50%"
            innerRadius={52}
            outerRadius={82}
            paddingAngle={2}
          >
            {data.map((_, i) => (
              <Cell
                key={`cell-${i}`}
                fill={SLICE_COLORS[i % SLICE_COLORS.length]}
              />
            ))}
          </Pie>
          <Tooltip
            formatter={(value: number | string) => [
              typeof value === "number"
                ? value.toLocaleString()
                : value,
              "Rounds on hand",
            ]}
          />
          <Legend />
        </PieChart>
      </ResponsiveContainer>
    </div>
  );
}
