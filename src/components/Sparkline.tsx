import type { TrendPoint } from "../types";

export const WEEK_MS = 7 * 86_400_000;
/** 기록이 짧아도 이만큼은 가로축으로 잡는다 (1시간) */
const MIN_SPAN_MS = 3_600_000;

/**
 * 가로축 범위를 모를 때(리셋 시각을 모를 때)의 길이.
 * 기록이 7일보다 짧으면 기록이 있는 기간만큼(최소 1시간) 줄인다.
 */
export function trendSpanMs(points: TrendPoint[]): number {
  if (points.length < 2) return MIN_SPAN_MS;
  const first = new Date(points[0].at).getTime();
  const last = new Date(points[points.length - 1].at).getTime();
  return Math.min(WEEK_MS, Math.max(MIN_SPAN_MS, last - first));
}

/**
 * 계정 카드의 작은 그래프: 최근 7일 동안 쓴 양(누적).
 * 쌓이다가 리셋 때 0% 로 떨어지고 다시 쌓이는 모양이 보인다.
 * 기록은 "값이 바뀔 때만" 남으므로 다음 기록 전까지 같은 값으로 보고 계단 모양으로 그린다.
 */
export function Sparkline({
  points,
  color,
  domain,
}: {
  points: TrendPoint[];
  color: string;
  /** 가로축 [시작, 끝] (ms). 모든 계정이 같은 최근 7일. 없으면 기록이 있는 기간 */
  domain?: [number, number];
}) {
  if (points.length === 0 || (!domain && points.length < 2)) return null;

  const times = points.map((point) => new Date(point.at).getTime());
  const last = times.length - 1;
  const [start, end] = domain ?? [times[last] - trendSpanMs(points), times[last]];
  const span = Math.max(1, end - start);
  const x = (time: number) => (Math.min(span, Math.max(0, time - start)) / span) * 100;
  const y = (percent: number) => {
    const value = Math.min(100, Math.max(0, percent));
    return 29 - (value / 100) * 27;
  };

  let line = `M${x(times[0]).toFixed(2)},${y(points[0].percent).toFixed(2)}`;
  for (let index = 1; index < points.length; index++) {
    const px = x(times[index]).toFixed(2);
    line += ` L${px},${y(points[index - 1].percent).toFixed(2)} L${px},${y(points[index].percent).toFixed(2)}`;
  }
  const area = `${line} L${x(times[last]).toFixed(2)},30 L${x(times[0]).toFixed(2)},30 Z`;
  return (
    <svg className="sparkline" viewBox="0 0 100 30" preserveAspectRatio="none" aria-hidden="true">
      <path d={area} fill={color} opacity={0.14} />
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
