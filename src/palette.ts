// 계정 색 팔레트. Rust(src-tauri/src/accounts.rs)의 PALETTE 와 같은 순서.
// 그래프에서 색각 이상이 있어도 서로 구분되도록 검사한 8색이다 (dataviz 검사기 통과, 순서가 구분을 돕는다).

export const PALETTE = ["#EB6834", "#2A78D6", "#1BAF7A", "#EDA100", "#E87BA4", "#008300", "#6250D6", "#E34948"];

/** 다크 모드 그래프에서 쓸 짝 (어두운 배경에서 같은 검사를 통과한 색) */
const DARK_PAIR: Record<string, string> = {
  "#EB6834": "#D95926",
  "#2A78D6": "#3987E5",
  "#1BAF7A": "#199E70",
  "#EDA100": "#C98500",
  "#E87BA4": "#D55181",
  "#008300": "#0CA30C",
  "#6250D6": "#9085E9",
  "#E34948": "#E66767",
};

/** 그래프에 그릴 계정 색. 팔레트 색이면 다크 모드에서 짝이 되는 색으로 바꾼다. 직접 고른 다른 색은 그대로. */
export function chartColor(color: string, dark: boolean): string {
  return dark ? (DARK_PAIR[color.toUpperCase()] ?? color) : color;
}
