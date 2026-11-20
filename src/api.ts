// Rust 쪽 명령을 부르는 함수 모음.
// 앱(Tauri) 안에서는 `invoke` 로 Rust 함수를 부르고,
// 일반 브라우저(`npm run dev` 화면 미리보기)에서는 가짜 데이터를 돌려준다.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { mockSettings, mockSnapshot } from "./mock";
import type { Settings, Snapshot } from "./types";

const inApp = isTauri();
let browserSettings: Settings = mockSettings();

export function getSnapshot(): Promise<Snapshot> {
  return inApp ? invoke<Snapshot>("get_snapshot") : Promise.resolve(mockSnapshot());
}

export function refreshNow(): Promise<Snapshot> {
  return inApp ? invoke<Snapshot>("refresh_now") : Promise.resolve(mockSnapshot());
}

export function getSettings(): Promise<Settings> {
  return inApp ? invoke<Settings>("get_settings") : Promise.resolve(browserSettings);
}

export async function setSettings(settings: Settings): Promise<void> {
  if (inApp) {
    await invoke("set_settings", { settings });
  } else {
    browserSettings = settings;
  }
}

/** 내용 높이에 맞게 팝오버 창 높이를 바꾼다. */
export function resizePopover(height: number): Promise<void> {
  return inApp ? invoke("resize_popover", { height }) : Promise.resolve();
}

export function hidePopover(): Promise<void> {
  return inApp ? invoke("hide_popover") : Promise.resolve();
}

export function quitApp(): Promise<void> {
  return inApp ? invoke("quit_app") : Promise.resolve();
}

/** Rust 가 1분마다 보내는 새 스냅샷을 받는다. 반환값을 부르면 구독을 끝낸다. */
export function onSnapshotUpdated(callback: (snapshot: Snapshot) => void): Promise<UnlistenFn> {
  if (!inApp) return Promise.resolve(() => {});
  return listen<Snapshot>("snapshot-updated", (event) => callback(event.payload));
}

export function onSettingsChanged(callback: (settings: Settings) => void): Promise<UnlistenFn> {
  if (!inApp) return Promise.resolve(() => {});
  return listen<Settings>("settings-changed", (event) => callback(event.payload));
}

/** 팝오버가 열릴 때(창이 포커스를 얻을 때) 알림을 받는다. */
export function onPopoverShown(callback: () => void): Promise<UnlistenFn> {
  if (!inApp) return Promise.resolve(() => {});
  return getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (focused) callback();
  });
}
