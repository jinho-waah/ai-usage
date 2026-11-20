import { useEffect, useLayoutEffect, useRef, useState } from "react";

import {
  getSettings,
  getSnapshot,
  hidePopover,
  onPopoverShown,
  onSettingsChanged,
  onSnapshotUpdated,
  quitApp,
  refreshNow,
  resizePopover,
  setSettings,
} from "./api";
import { AccountCard } from "./components/AccountCard";
import { Footer } from "./components/Footer";
import { RefreshIcon, SlidersIcon } from "./components/Icons";
import { SettingsView } from "./components/SettingsView";
import { t } from "./i18n";
import type { Settings, Snapshot } from "./types";

/** 팝오버 화면 전체 */
export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [settings, setSettingsState] = useState<Settings | null>(null);
  const [view, setView] = useState<"main" | "settings">("main");
  const [now, setNow] = useState(() => Date.now());
  const [refreshing, setRefreshing] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  // 처음 데이터를 받고, Rust 가 보내는 갱신 알림을 구독한다.
  useEffect(() => {
    getSnapshot().then(setSnapshot);
    getSettings().then(setSettingsState);

    const subscriptions = [
      onSnapshotUpdated((next) => {
        setSnapshot(next);
        setNow(Date.now());
      }),
      onSettingsChanged(setSettingsState),
      onPopoverShown(() => {
        setNow(Date.now());
        getSnapshot().then(setSnapshot);
      }),
    ];
    return () => {
      subscriptions.forEach((subscription) => subscription.then((unsubscribe) => unsubscribe()));
    };
  }, []);

  // "n분 전 갱신", "남은 시간" 같은 문구를 30초마다 다시 계산한다.
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(timer);
  }, []);

  // Esc 키로 팝오버 닫기
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") hidePopover();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // 내용 높이가 바뀔 때마다 팝오버 창 높이를 맞춘다.
  useLayoutEffect(() => {
    const content = contentRef.current;
    if (!content) return;
    const observer = new ResizeObserver(() => {
      resizePopover(Math.ceil(content.getBoundingClientRect().height));
    });
    observer.observe(content);
    return () => observer.disconnect();
  }, []);

  const refresh = async () => {
    setRefreshing(true);
    try {
      setSnapshot(await refreshNow());
      setNow(Date.now());
    } finally {
      setTimeout(() => setRefreshing(false), 400);
    }
  };

  const changeSettings = (next: Settings) => {
    setSettingsState(next);
    setSettings(next).catch((error) => console.error("설정 저장 실패", error));
  };

  return (
    <div className="popover">
      <div className="popover-content" ref={contentRef}>
        {snapshot && settings && view === "settings" && (
          <SettingsView settings={settings} onChange={changeSettings} onBack={() => setView("main")} />
        )}

        {snapshot && settings && view === "main" && (
          <>
            <header className="header">
              <span className="title">AI Usage</span>
              <div className="header-actions">
                <button className="icon-button" onClick={refresh} title={t("refresh")}>
                  <RefreshIcon className={refreshing ? "spin" : undefined} />
                </button>
                <button className="icon-button" onClick={() => setView("settings")} title={t("settings")}>
                  <SlidersIcon />
                </button>
              </div>
            </header>

            <main className="accounts">
              {snapshot.accounts.length === 0 ? (
                <p className="empty">{t("noAccounts")}</p>
              ) : (
                snapshot.accounts.map((account) => (
                  <AccountCard key={account.id} account={account} percentMode={settings.percentMode} now={now} />
                ))
              )}
            </main>

            <Footer summary={snapshot.summary} updatedAt={snapshot.updatedAt} now={now} onQuit={quitApp} />
          </>
        )}
      </div>
    </div>
  );
}
