import type { AccountLabel } from "../types";

/** 그림 글자(이모지)가 들어 있는지. "C" 같은 보통 글자 라벨은 카드에서 "C Codex"처럼 겹쳐 보이지 않게 점으로 둔다 */
const PICTOGRAPH = /\p{Extended_Pictographic}/u;

/** 팝오버에서 이름 앞에 붙는 표시: 이모지 라벨이면 이모지, 아니면 계정 색 점 */
export function LabelBadge({ label, color }: { label: AccountLabel; color: string }) {
  if (label.kind === "emoji" && PICTOGRAPH.test(label.text)) {
    return <span className="label-emoji">{label.text}</span>;
  }
  return <span className="label-dot" style={{ background: color }} />;
}
