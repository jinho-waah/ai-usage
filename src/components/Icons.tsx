// 작은 아이콘들. 글자색(currentColor)을 따라가므로 라이트·다크 모드에 자동으로 맞는다.

interface IconProps {
  size?: number;
  className?: string;
}

const stroke = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.6,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export function RefreshIcon({ size = 14, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" className={className} aria-hidden="true">
      <path {...stroke} d="M13.2 8.6A5.3 5.3 0 1 1 11.6 4" />
      <path {...stroke} d="M12.2 1.8v2.8H9.4" />
    </svg>
  );
}

export function SlidersIcon({ size = 14, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" className={className} aria-hidden="true">
      <path {...stroke} d="M2.5 4.5h11M2.5 11.5h11" />
      <circle cx="5.5" cy="4.5" r="1.9" fill="currentColor" />
      <circle cx="10.5" cy="11.5" r="1.9" fill="currentColor" />
    </svg>
  );
}

export function ChevronIcon({
  size = 10,
  direction,
}: IconProps & { direction: "left" | "right" | "down" }) {
  const rotation = { right: 0, down: 90, left: 180 }[direction];
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 10 10"
      style={{ transform: `rotate(${rotation}deg)`, transition: "transform 120ms ease" }}
      aria-hidden="true"
    >
      <path {...stroke} d="M3.5 1.8 6.7 5 3.5 8.2" />
    </svg>
  );
}
