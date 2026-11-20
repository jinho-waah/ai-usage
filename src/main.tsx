import { isTauri } from "@tauri-apps/api/core";
import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { locale } from "./i18n";
import "./styles.css";

document.documentElement.lang = locale;
// 일반 브라우저에서 열었을 때(화면 미리보기)는 배경을 깔아서 팝오버가 보이게 한다.
if (!isTauri()) document.documentElement.classList.add("preview");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
