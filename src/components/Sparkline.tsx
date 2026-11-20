import type { TrendPoint } from "../types";

/** 최근 사용률 변화를 보여 주는 작은 선 그래프. 0~100% 를 세로 범위로 쓴다. */
export function Sparkline({ points, color }: { points: TrendPoint[]; color: string }) {
  if (points.length < 2) return null;

  const last = points.length - 1;
  const line = points
    .map((point, index) => {
      const x = (index / last) * 100;
      const y = 29 - (Math.min(100, Math.max(0, point.percent)) / 100) * 27;
      return `${index === 0 ? "M" : "L"}${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");

  return (
    <svg className="sparkline" viewBox="0 0 100 30" preserveAspectRatio="none" aria-hidden="true">
      <path d={`${line} L100,30 L0,30 Z`} fill={color} opacity={0.14} />
      <path
        d={line}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeLinejoin="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
