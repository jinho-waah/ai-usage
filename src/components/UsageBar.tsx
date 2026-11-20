import type { CSSProperties } from "react";

interface UsageBarProps {
  /** 사용량 0~100. 막대는 항상 사용량만큼 채운다. */
  used: number;
  /** 시간 눈금 위치 0~100. 없으면 눈금을 그리지 않는다. */
  elapsed?: number;
  /** true 면 빨강 (사용량이 시간 진행보다 5%p 이상 앞섬) */
  warn?: boolean;
}

/** 메뉴바와 같은 모양의 막대: 흐린 바탕 + 사용량 채움 + 시간 눈금 */
export function UsageBar({ used, elapsed, warn = false }: UsageBarProps) {
  const hasMarker = elapsed !== undefined;
  const style = {
    "--used": `${clamp(used)}%`,
    "--elapsed": `${clamp(elapsed ?? 0)}%`,
  } as CSSProperties;

  return (
    <div className={`usage-bar${warn ? " warn" : ""}${hasMarker ? " has-marker" : ""}`} style={style}>
      <div className="usage-bar-body">
        <div className="usage-bar-fill" />
      </div>
      {hasMarker && <div className="usage-bar-marker" />}
    </div>
  );
}

function clamp(value: number): number {
  return Math.min(100, Math.max(0, value));
}
