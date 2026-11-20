import type { AccountLabel } from "../types";

/** 팝오버에서 이름 앞에 붙는 표시: 이모지 라벨이면 이모지, 아니면 계정 색 점 */
export function LabelBadge({ label, color }: { label: AccountLabel; color: string }) {
  if (label.kind === "emoji") {
    return <span className="label-emoji">{label.text}</span>;
  }
  return <span className="label-dot" style={{ background: color }} />;
}
