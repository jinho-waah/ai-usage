import type { CSSProperties } from "react";

import type { AccountLabel } from "../types";

/** 계정 라벨: 별칭(글자 배지), 이모지, 색 점 중 하나 */
export function LabelBadge({ label, color }: { label: AccountLabel; color: string }) {
  if (label.kind === "dot") {
    return <span className="label-dot" style={{ background: color }} />;
  }
  if (label.kind === "emoji") {
    return <span className="label-emoji">{label.text}</span>;
  }
  return (
    <span className="label-alias" style={{ "--account-color": color } as CSSProperties}>
      {label.text}
    </span>
  );
}
