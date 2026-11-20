// Rust 쪽 명령을 부르는 함수 모음.
// 앱(Tauri) 안에서는 `invoke` 로 Rust 함수를 부르고,
// 일반 브라우저(`npm run dev` 화면 미리보기)에서는 가짜 데이터를 돌려준다.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { mockAccounts, mockHistory, mockSettings, mockSnapshot } from "./mock";
import type {
  Account,
  AccountEdit,
  ApiCostInfo,
  HistoryData,
  HistoryRange,
  LegacyProfile,
  Settings,
  Snapshot,
} from "./types";

const inApp = isTauri();
let browserSettings: Settings = mockSettings();
let browserAccounts: Account[] = mockAccounts();
let browserApiCost: ApiCostInfo = { configured: false, keyHint: null, error: null, cost: null };

// ───────── 히스토리 창 ─────────

export function getHistory(range: HistoryRange): Promise<HistoryData> {
  return inApp ? invoke<HistoryData>("get_history", { range }) : Promise.resolve(mockHistory(range));
}

/** 히스토리 창을 연다 (이미 열려 있으면 앞으로). 브라우저 미리보기에서는 새 탭. */
export function openHistory(): Promise<void> {
  if (inApp) return invoke("open_history");
  window.open(`${location.pathname}#history`, "_blank");
  return Promise.resolve();
}

// ───────── Claude API 비용 (Admin API 키) ─────────

export function getApiCostInfo(): Promise<ApiCostInfo> {
  return inApp ? invoke<ApiCostInfo>("get_api_cost_info") : Promise.resolve(browserApiCost);
}

/** 키를 확인하고 키체인에 저장한다. 틀린 키면 오류 메시지를 던진다. */
export function setAdminKey(key: string): Promise<ApiCostInfo> {
  if (inApp) return invoke<ApiCostInfo>("set_admin_key", { key });
  if (!key.trim().startsWith("sk-ant-admin")) {
    return Promise.reject("Admin API 키(sk-ant-admin…)를 넣어 주세요. 일반 API 키로는 비용을 볼 수 없어요");
  }
  browserApiCost = {
    configured: true,
    keyHint: `sk-ant-admin…${key.trim().slice(-4)}`,
    error: null,
    cost: { usd: 3.1, refreshedAt: new Date(Date.now() - 3 * 3_600_000).toISOString() },
  };
  return Promise.resolve(browserApiCost);
}

export function removeAdminKey(): Promise<ApiCostInfo> {
  if (inApp) return invoke<ApiCostInfo>("remove_admin_key");
  browserApiCost = { configured: false, keyHint: null, error: null, cost: null };
  return Promise.resolve(browserApiCost);
}

// ───────── 계정 ─────────

export function getAccounts(): Promise<Account[]> {
  return inApp ? invoke<Account[]>("get_accounts") : Promise.resolve(browserAccounts);
}

/** Claude 계정을 새로 만들고 로그인 창을 연다. */
export function addClaudeAccount(): Promise<Account> {
  if (inApp) return invoke<Account>("add_claude_account");
  const account: Account = {
    id: `browser-${browserAccounts.length + 1}`,
    provider: "claude",
    name: `Claude ${browserAccounts.length + 1}`,
    label: { kind: "name", text: "" },
    color: "#B07CC6",
    orgId: null,
    orgName: null,
    plan: null,
  };
  browserAccounts = [...browserAccounts, account];
  return Promise.resolve(account);
}

/** Codex 를 추가한다. Codex CLI 로그인이 없으면 오류 메시지를 던진다. */
export function addCodexAccount(): Promise<Account> {
  if (inApp) return invoke<Account>("add_codex_account");
  if (browserAccounts.some((account) => account.provider === "codex")) {
    return Promise.reject("Codex 는 이미 추가돼 있어요");
  }
  const account: Account = {
    id: "browser-codex",
    provider: "codex",
    name: "Codex",
    label: { kind: "name", text: "" },
    color: "#10A37F",
    orgId: null,
    orgName: null,
    plan: "Pro",
  };
  browserAccounts = [...browserAccounts, account];
  return Promise.resolve(account);
}

export async function updateAccount(edit: AccountEdit): Promise<void> {
  if (inApp) return invoke("update_account", { edit });
  browserAccounts = browserAccounts.map((account) =>
    account.id === edit.id ? { ...account, name: edit.name, label: edit.label, color: edit.color } : account,
  );
}

export async function moveAccount(id: string, offset: number): Promise<void> {
  if (inApp) return invoke("move_account", { id, offset });
  const index = browserAccounts.findIndex((account) => account.id === id);
  const target = index + offset;
  if (index < 0 || target < 0 || target >= browserAccounts.length) return;
  const next = [...browserAccounts];
  [next[index], next[target]] = [next[target], next[index]];
  browserAccounts = next;
}

/** 로그인 창을 다시 연다. */
export function reloginAccount(id: string): Promise<void> {
  return inApp ? invoke("relogin_account", { id }) : Promise.resolve();
}

/** 계정을 지운다. 그 계정의 로그인 정보도 함께 지운다. */
export async function removeAccount(id: string): Promise<void> {
  if (inApp) return invoke("remove_account", { id });
  browserAccounts = browserAccounts.filter((account) => account.id !== id);
}

/** 원본 Claude Usage 앱에서 가져올 수 있는 프로필 목록 */
export function listLegacyProfiles(): Promise<LegacyProfile[]> {
  if (inApp) return invoke<LegacyProfile[]>("list_legacy_profiles");
  const now = Date.now();
  return Promise.resolve([
    {
      id: "mock-legacy",
      name: "Jinho",
      weeklyCount: 500,
      sessionCount: 1000,
      firstAt: new Date(now - 52 * 86_400_000).toISOString(),
      lastAt: new Date(now).toISOString(),
    },
  ]);
}

/** 원본 앱 프로필의 기록을 계정 하나로 가져온다. 가져온 기록 수를 돌려준다. */
export function importLegacyHistory(profileId: string, accountId: string): Promise<number> {
  return inApp ? invoke<number>("import_legacy_history", { profileId, accountId }) : Promise.resolve(123);
}

export function onAccountsChanged(callback: (accounts: Account[]) => void): Promise<UnlistenFn> {
  if (!inApp) return Promise.resolve(() => {});
  return listen<Account[]>("accounts-changed", (event) => callback(event.payload));
}

// ───────── 사용량·설정 ─────────

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
