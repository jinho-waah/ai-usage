import { isTauri } from "@tauri-apps/api/core";
import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { HistoryApp } from "./history/HistoryApp";
import { locale } from "./i18n";
import "./styles.css";

document.documentElement.lang = locale;

// 같은 화면 코드로 두 창을 그린다: 주소 끝이 #history 면 히스토리 창, 아니면 팝오버.
const isHistory = window.location.hash === "#history";
if (isHistory) {
  document.documentElement.classList.add("history-window");
} else if (!isTauri()) {
  // 일반 브라우저에서 열었을 때(화면 미리보기)는 배경을 깔아서 팝오버가 보이게 한다.
  document.documentElement.classList.add("preview");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isHistory ? <HistoryApp /> : <App />}</React.StrictMode>,
);
