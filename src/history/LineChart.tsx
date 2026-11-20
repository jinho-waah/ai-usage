import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";

// 히스토리 창의 선 그래프 (SVG 로 직접 그림).
// - 세로축은 항상 0~100(%), 가로축은 시각(ms) 또는 주기 경과 시간(시간)
// - 선 2px, 격자는 1px 실선, 값이 바뀌기 전까지 같은 값이므로 기본은 계단 모양
// - 마우스(또는 ←/→ 키)로 세로선을 움직이면 그 위치의 모든 선 값을 툴팁에 보여 준다

export interface ChartPoint {
  x: number;
  y: number;
}

export interface ChartLine {
  id: string;
  /** 툴팁·끝 이름표에 쓰는 이름 */
  label: string;
  color: string;
  points: ChartPoint[];
  /** primary: 진한 2px 선, muted: 회색 지난 주기, reference: 점선 기준선 */
  tone?: "primary" | "muted" | "reference";
  /** muted 선의 진하기 (0~1) */
  strength?: number;
  /** false 면 점과 점을 곧게 잇는다 (기준선). 기본은 계단 */
  step?: boolean;
  /** 툴팁에서 뺄지 */
  hideInTooltip?: boolean;
}

export interface ChartMarker {
  x: number;
  y: number;
  color: string;
}

export interface Tick {
  value: number;
  label: string;
}

interface LineChartProps {
  lines: ChartLine[];
  xDomain: [number, number];
  /** 가로축 눈금. 그래프 너비에 맞춰 만든다 */
  xTicks: (plotWidth: number) => Tick[];
  height: number;
  markers?: ChartMarker[];
  /** 툴팁 제목 */
  formatX: (x: number) => string;
  /** 이 선 아래를 옅게 칠한다 (선이 하나일 때) */
  areaFor?: string;
  /** 선 끝에 이름표 (선이 4개 이하이고 겹치지 않을 때만) */
  endLabels?: boolean;
  ariaLabel: string;
}

const Y_TICKS = [0, 25, 50, 75, 100];
const MARGIN = { top: 10, bottom: 24, left: 40 };
const END_LABEL_SPACE = 76;
/** 끝 이름표끼리 이만큼보다 가까우면 이름표를 빼고 범례·툴팁에 맡긴다 */
const END_LABEL_MIN_GAP = 13;

function toneOrder(line: ChartLine): number {
  return { primary: 0, muted: 1, reference: 2 }[line.tone ?? "primary"];
}

/** 계단 모양: x 에서의 값 = x 보다 앞선 마지막 점의 값. 선 밖이면 null */
export function valueAt(line: ChartLine, x: number): number | null {
  const points = line.points;
  if (points.length === 0 || x < points[0].x || x > points[points.length - 1].x) return null;
  let low = 0;
  let high = points.length - 1;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (points[middle].x <= x) low = middle;
    else high = middle - 1;
  }
  const point = points[low];
  if (line.step === false && low < points.length - 1) {
    const next = points[low + 1];
    const ratio = next.x === point.x ? 0 : (x - point.x) / (next.x - point.x);
    return point.y + (next.y - point.y) * ratio;
  }
  return point.y;
}

export function LineChart({
  lines,
  xDomain,
  xTicks,
  height,
  markers = [],
  formatX,
  areaFor,
  endLabels = false,
  ariaLabel,
}: LineChartProps) {
  const wrapper = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(640);
  const [hoverX, setHoverX] = useState<number | null>(null);

  useEffect(() => {
    const element = wrapper.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setWidth(Math.max(200, entry.contentRect.width)));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const [x0, x1] = xDomain;
  const plotHeight = height - MARGIN.top - MARGIN.bottom;
  const y = (value: number) => MARGIN.top + plotHeight - (Math.min(100, Math.max(0, value)) / 100) * plotHeight;

  // 끝 이름표: 선 끝의 높이가 서로 충분히 떨어져 있을 때만
  const primary = lines.filter((line) => (line.tone ?? "primary") === "primary" && line.points.length > 0);
  const labelYs = primary.map((line) => y(line.points[line.points.length - 1].y)).sort((a, b) => a - b);
  const labelsFit = labelYs.every((value, index) => index === 0 || value - labelYs[index - 1] >= END_LABEL_MIN_GAP);
  const showEndLabels = endLabels && primary.length <= 4 && labelsFit;

  const right = showEndLabels ? END_LABEL_SPACE : 14;
  const plotWidth = Math.max(10, width - MARGIN.left - right);
  const x = (value: number) => MARGIN.left + ((value - x0) / (x1 - x0 || 1)) * plotWidth;
  const invert = (pixel: number) => x0 + ((pixel - MARGIN.left) / plotWidth) * (x1 - x0);

  // 선 모양 (점이 수백 개 정도라 그릴 때마다 새로 계산해도 충분히 빠르다)
  const paths = lines.map((line) => {
    if (line.points.length === 0) return { line, d: "", area: "" };
    let d = `M${x(line.points[0].x).toFixed(1)},${y(line.points[0].y).toFixed(1)}`;
    for (let index = 1; index < line.points.length; index++) {
      const point = line.points[index];
      d +=
        line.step === false
          ? ` L${x(point.x).toFixed(1)},${y(point.y).toFixed(1)}`
          : ` H${x(point.x).toFixed(1)} V${y(point.y).toFixed(1)}`;
    }
    const area = `${d} V${y(0).toFixed(1)} H${x(line.points[0].x).toFixed(1)} Z`;
    return { line, d, area };
  });

  const ticks = xTicks(plotWidth);
  const [clipId] = useState(() => `clip-${Math.random().toString(36).slice(2)}`);

  const move = (event: PointerEvent<SVGRectElement>) => {
    const box = event.currentTarget.ownerSVGElement?.getBoundingClientRect();
    if (!box) return;
    const value = invert(event.clientX - box.left);
    setHoverX(Math.min(x1, Math.max(x0, value)));
  };

  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = (x1 - x0) / 60;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      const start = hoverX ?? x1;
      const next = start + (event.key === "ArrowRight" ? step : -step);
      setHoverX(Math.min(x1, Math.max(x0, next)));
    } else if (event.key === "Escape") {
      setHoverX(null);
    }
  };

  // 툴팁: 세로선 위치의 모든 선 값 (값이 먼저, 이름은 뒤)
  const rows =
    hoverX === null
      ? []
      : lines
          .filter((line) => !line.hideInTooltip)
          .map((line) => ({ line, value: valueAt(line, hoverX) }))
          .filter((row): row is { line: ChartLine; value: number } => row.value !== null)
          // 진한 선(이번 주기·계정) 먼저, 지난 주기, 기준선 순서
          .sort((a, b) => toneOrder(a.line) - toneOrder(b.line));
  const hoverPixel = hoverX === null ? 0 : x(hoverX);
  const tooltipOnLeft = hoverPixel > width - 200;

  return (
    <div
      className="chart"
      ref={wrapper}
      tabIndex={0}
      role="img"
      aria-label={ariaLabel}
      onKeyDown={keyDown}
      onFocus={() => setHoverX((current) => current ?? x1)}
      onBlur={() => setHoverX(null)}
    >
      <svg width={width} height={height} aria-hidden="true">
        <defs>
          <clipPath id={clipId}>
            <rect x={MARGIN.left} y={0} width={plotWidth} height={height} />
          </clipPath>
        </defs>

        {/* 가로 격자와 세로축 숫자 */}
        {Y_TICKS.map((tick) => (
          <g key={tick}>
            <line
              className={tick === 0 ? "chart-axis" : "chart-grid"}
              x1={MARGIN.left}
              x2={MARGIN.left + plotWidth}
              y1={y(tick)}
              y2={y(tick)}
            />
            <text className="chart-tick" x={MARGIN.left - 7} y={y(tick)} dy="0.32em" textAnchor="end">
              {tick}%
            </text>
          </g>
        ))}

        {/* 가로축 눈금 이름 */}
        {ticks.map((tick, index) => {
          const position = x(tick.value);
          const anchor = position < MARGIN.left + 18 ? "start" : position > MARGIN.left + plotWidth - 18 ? "end" : "middle";
          return (
            <text
              key={`${tick.value}-${index}`}
              className="chart-tick"
              x={position}
              y={MARGIN.top + plotHeight + 16}
              textAnchor={anchor}
            >
              {tick.label}
            </text>
          );
        })}

        <g clipPath={`url(#${clipId})`}>
          {paths.map(({ line, area }) =>
            line.id === areaFor && area ? (
              <path key={`${line.id}-area`} d={area} fill={line.color} opacity={0.1} />
            ) : null,
          )}
          {paths.map(({ line, d }) => {
            const tone = line.tone ?? "primary";
            return (
              <path
                key={line.id}
                d={d}
                fill="none"
                stroke={tone === "primary" ? line.color : "var(--chart-muted)"}
                strokeWidth={tone === "primary" ? 2 : tone === "muted" ? 1.5 : 1}
                strokeOpacity={tone === "muted" ? (line.strength ?? 0.6) : 1}
                strokeDasharray={tone === "reference" ? "4 4" : undefined}
                strokeLinejoin="round"
                strokeLinecap="round"
              />
            );
          })}
          {markers.map((marker, index) => (
            <circle
              key={`marker-${index}`}
              className="chart-dot"
              cx={x(marker.x)}
              cy={y(marker.y)}
              r={4}
              fill={marker.color}
            />
          ))}
        </g>

        {/* 끝 이름표: 글자는 글자색, 선 끝의 색이 누구인지 알려 준다 */}
        {showEndLabels &&
          primary.map((line) => {
            const last = line.points[line.points.length - 1];
            return (
              <text
                key={`${line.id}-label`}
                className="chart-end-label"
                x={x(last.x) + 8}
                y={y(last.y)}
                dy="0.32em"
              >
                {line.label}
              </text>
            );
          })}

        {/* 세로선과 그 위치의 점 */}
        {hoverX !== null && (
          <g>
            <line
              className="chart-crosshair"
              x1={hoverPixel}
              x2={hoverPixel}
              y1={MARGIN.top}
              y2={MARGIN.top + plotHeight}
            />
            {rows
              .filter(({ line }) => (line.tone ?? "primary") === "primary")
              .map(({ line, value }) => (
                <circle key={`${line.id}-hover`} className="chart-dot" cx={hoverPixel} cy={y(value)} r={4} fill={line.color} />
              ))}
          </g>
        )}

        {/* 마우스를 받는 투명한 판 (선보다 넓게) */}
        <rect
          x={MARGIN.left}
          y={0}
          width={plotWidth}
          height={height}
          fill="transparent"
          onPointerMove={move}
          onPointerDown={move}
          onPointerLeave={() => setHoverX(null)}
        />
      </svg>

      {hoverX !== null && rows.length > 0 && (
        <div
          className="chart-tooltip"
          style={tooltipOnLeft ? { right: width - hoverPixel + 12 } : { left: hoverPixel + 12 }}
        >
          <div className="chart-tooltip-title">{formatX(hoverX)}</div>
          {rows.map(({ line, value }) => (
            <div className="chart-tooltip-row" key={line.id}>
              <span
                className={`line-key${line.tone === "reference" ? " dashed" : ""}`}
                style={{ borderColor: (line.tone ?? "primary") === "primary" ? line.color : "var(--chart-muted)" }}
              />
              <strong>{Math.round(value)}%</strong>
              <span className="chart-tooltip-name">{line.label}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
