import { useEffect, useState } from "react";

import {
  addClaudeAccount,
  addCodexAccount,
  getApiCostInfo,
  getLoginItem,
  importLegacyHistory,
  listLegacyProfiles,
  moveAccount,
  reloginAccount,
  removeAccount,
  removeAdminKey,
  setAdminKey,
  setLoginItem,
  updateAccount,
} from "../api";
import { formatAsOf, formatDateRange, formatUsd, t } from "../i18n";
import { PALETTE } from "../palette";
import type { Account, ApiCostInfo, LabelKind, LegacyProfile, LoginItemState, Settings } from "../types";
import { ChevronIcon } from "./Icons";
import { LabelBadge } from "./LabelBadge";
import { Segmented } from "./Segmented";


interface SettingsViewProps {
  settings: Settings;
  accounts: Account[];
  onChange: (settings: Settings) => void;
  onBack: () => void;
}

/** 설정 화면: 계정, 메뉴바 표시, 퍼센트 기준 */
export function SettingsView({ settings, accounts, onChange, onBack }: SettingsViewProps) {
  return (
    <div className="settings">
      <header className="header">
        <button className="icon-button" onClick={onBack} title={t("back")}>
          <ChevronIcon direction="left" size={12} />
        </button>
        <span className="title">{t("settings")}</span>
        <span className="header-spacer" />
      </header>

      <AccountsSection accounts={accounts} />

      <section className="setting">
        <div className="setting-title">{t("menubarStyle")}</div>
        <Segmented
          value={settings.menubarStyle}
          options={[
            ["bars", t("bars")],
            ["numbers", t("numbers")],
          ]}
          onChange={(menubarStyle) => onChange({ ...settings, menubarStyle })}
        />
        <p className="setting-help">{t("menubarStyleHelp")}</p>
      </section>

      <section className="setting">
        <div className="setting-title">{t("percentMode")}</div>
        <Segmented
          value={settings.percentMode}
          options={[
            ["remaining", t("remaining")],
            ["used", t("used")],
          ]}
          onChange={(percentMode) => onChange({ ...settings, percentMode })}
        />
        <p className="setting-help">
          {settings.percentMode === "remaining" ? t("percentModeHelpRemaining") : t("percentModeHelpUsed")}
        </p>
      </section>

      <StartupSection />

      <LegacyImportSection accounts={accounts} />
    </div>
  );
}

/** 로그인할 때 자동으로 열기 */
function StartupSection() {
  const [state, setState] = useState<LoginItemState | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getLoginItem().then(setState);
  }, []);

  if (!state) return null;

  const toggle = async (enabled: boolean) => {
    setError(null);
    try {
      setState(await setLoginItem(enabled));
    } catch (reason) {
      setError(String(reason));
    }
  };

  return (
    <section className="setting">
      <div className="setting-title">{t("startup")}</div>
      <label className={`switch-row${state.supported ? "" : " disabled"}`}>
        <span>{t("launchAtLogin")}</span>
        <input
          type="checkbox"
          role="switch"
          className="switch"
          checked={state.enabled}
          disabled={!state.supported}
          onChange={(event) => toggle(event.target.checked)}
        />
      </label>
      {error && <p className="error-text">{error}</p>}
      <p className="setting-help">
        {!state.supported
          ? t("launchAtLoginDev")
          : state.needsApproval
            ? t("launchAtLoginApproval")
            : t("launchAtLoginHelp")}
      </p>
    </section>
  );
}

/** 계정 편집 화면의 Claude API 칸: 이 계정의 Admin API 키 입력·삭제 */
function ApiKeyField({ accountId }: { accountId: string }) {
  const [info, setInfo] = useState<ApiCostInfo | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getApiCostInfo(accountId).then(setInfo);
  }, [accountId]);

  if (!info) return null;

  const save = async () => {
    if (!draft.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      setInfo(await setAdminKey(accountId, draft));
      setDraft("");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setError(null);
    try {
      setInfo(await removeAdminKey(accountId));
    } catch (reason) {
      setError(String(reason));
    }
  };

  const message = error ?? info.error;
  return (
    <div className="field">
      <span className="field-label">{t("apiCostTitle")}</span>
      <div className="field-stack">
      {info.configured ? (
        <div className="api-key-row">
          <div className="account-text">
            <span className="account-name">{info.keyHint}</span>
            <span className="account-sub">
              {info.cost
                ? `${t("apiThisMonth")} ${formatUsd(info.cost.usd)}` +
                  (info.cost.refreshedAt ? ` · ${formatAsOf(info.cost.refreshedAt, Date.now())}` : "")
                : t("apiKeyConnected")}
            </span>
          </div>
          <button className="text-button danger" onClick={remove}>
            {t("removeKey")}
          </button>
        </div>
      ) : (
        <div className="api-key-row">
          <input
            className="text-input"
            type="password"
            placeholder={t("apiKeyPlaceholder")}
            value={draft}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") save();
            }}
          />
          <button className="push-button" disabled={!draft.trim() || busy} onClick={save}>
            {busy ? t("checking") : t("save")}
          </button>
        </div>
      )}
      {message && <span className="error-text">{message}</span>}
      <span className="field-help">{t("apiCostHelp")}</span>
      </div>
    </div>
  );
}

/** 원본 Claude Usage 앱의 기록을 가져오는 칸. 원본 앱 기록이 없으면 보이지 않는다. */
function LegacyImportSection({ accounts }: { accounts: Account[] }) {
  const [profiles, setProfiles] = useState<LegacyProfile[]>([]);
  const [targets, setTargets] = useState<Record<string, string>>({});
  const [results, setResults] = useState<Record<string, string>>({});
  const claudeAccounts = accounts.filter((account) => account.provider === "claude");

  useEffect(() => {
    listLegacyProfiles().then(setProfiles);
  }, []);

  if (profiles.length === 0 || claudeAccounts.length === 0) return null;

  const runImport = async (profile: LegacyProfile) => {
    const accountId = targets[profile.id];
    if (!accountId) return;
    try {
      const count = await importLegacyHistory(profile.id, accountId);
      setResults((previous) => ({ ...previous, [profile.id]: t("imported", { count }) }));
    } catch (error) {
      setResults((previous) => ({ ...previous, [profile.id]: String(error) }));
    }
  };

  return (
    <section className="setting">
      <div className="setting-title">{t("importTitle")}</div>
      <p className="setting-help first">{t("importHelp")}</p>
      {profiles.map((profile) => (
        <div className="import-row" key={profile.id}>
          <div className="account-text">
            <span className="account-name">{profile.name}</span>
            <span className="account-sub">
              {formatDateRange(profile.firstAt, profile.lastAt)} ·{" "}
              {t("legacyRecords", { weekly: profile.weeklyCount, session: profile.sessionCount })}
            </span>
          </div>
          <div className="import-controls">
            <select
              className="select"
              value={targets[profile.id] ?? ""}
              onChange={(event) => setTargets((previous) => ({ ...previous, [profile.id]: event.target.value }))}
            >
              <option value="" disabled>
                {t("chooseAccount")}
              </option>
              {claudeAccounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.name}
                </option>
              ))}
            </select>
            <button className="push-button" disabled={!targets[profile.id]} onClick={() => runImport(profile)}>
              {t("importButton")}
            </button>
          </div>
          {results[profile.id] && <span className="import-result">{results[profile.id]}</span>}
        </div>
      ))}
    </section>
  );
}

/** 계정 목록과 "Claude 계정 추가" 버튼. `compact` 면 제목·구분선 없이 (빈 화면용) */
export function AccountsSection({ accounts, compact = false }: { accounts: Account[]; compact?: boolean }) {
  const [loginStarted, setLoginStarted] = useState(false);
  const [codexError, setCodexError] = useState<string | null>(null);
  const hasCodex = accounts.some((account) => account.provider === "codex");

  const addClaude = async () => {
    setLoginStarted(true);
    await addClaudeAccount();
  };

  const addCodex = async () => {
    setCodexError(null);
    try {
      await addCodexAccount();
    } catch (error) {
      setCodexError(String(error));
    }
  };

  return (
    <section className={compact ? "accounts-compact" : "setting"}>
      {!compact && <div className="setting-title">{t("accounts")}</div>}
      <div className="account-list">
        {accounts.map((account, index) => (
          <AccountRow key={account.id} account={account} isFirst={index === 0} isLast={index === accounts.length - 1} />
        ))}
      </div>
      <div className="add-buttons">
        <button className="push-button" onClick={addClaude}>
          + {t("addClaudeAccount")}
        </button>
        <button className="push-button" onClick={addCodex} disabled={hasCodex}>
          + {t("addCodex")}
        </button>
      </div>
      {codexError && <p className="setting-help error-text">{codexError}</p>}
      {loginStarted && (
        <p className="setting-help">
          {t("loginWindowOpened")} {t("googleLoginHint")}
        </p>
      )}
    </section>
  );
}

function AccountRow({ account, isFirst, isLast }: { account: Account; isFirst: boolean; isLast: boolean }) {
  const [editing, setEditing] = useState(false);

  return (
    <div className="account-item">
      <div className="account-row">
        <LabelBadge label={account.label} color={account.color} />
        <div className="account-text">
          <span className="account-name">{account.name}</span>
          <span className="account-sub">
            {account.provider === "codex"
              ? [account.plan, t("codexCli")].filter(Boolean).join(" · ")
              : account.orgId
                ? [account.plan, account.orgName].filter(Boolean).join(" · ")
                : t("notSignedIn")}
          </span>
        </div>
        <div className="account-actions">
          <button
            className="icon-button small"
            disabled={isFirst}
            title={t("moveUp")}
            onClick={() => moveAccount(account.id, -1)}
          >
            ↑
          </button>
          <button
            className="icon-button small"
            disabled={isLast}
            title={t("moveDown")}
            onClick={() => moveAccount(account.id, 1)}
          >
            ↓
          </button>
          <button className="text-button" onClick={() => setEditing(!editing)}>
            {t("edit")}
          </button>
        </div>
      </div>
      {editing && <AccountEditor account={account} onDone={() => setEditing(false)} />}
    </div>
  );
}

/** 이름, 메뉴바 라벨(별칭·이모지·색 점), 색을 고치는 작은 편집기 */
function AccountEditor({ account, onDone }: { account: Account; onDone: () => void }) {
  const [name, setName] = useState(account.name);
  const [kind, setKind] = useState<LabelKind>(account.label.kind);
  const [text, setText] = useState(account.label.text);
  const [color, setColor] = useState(account.color);
  const [confirmingRemove, setConfirmingRemove] = useState(false);

  const save = async () => {
    await updateAccount({ id: account.id, name, label: { kind, text }, color });
    onDone();
  };

  const remove = async () => {
    if (!confirmingRemove) {
      setConfirmingRemove(true);
      return;
    }
    await removeAccount(account.id);
  };

  return (
    <div className="account-editor">
      <label className="field">
        <span className="field-label">{t("name")}</span>
        <input className="text-input" value={name} maxLength={30} onChange={(event) => setName(event.target.value)} />
      </label>

      <div className="field">
        <span className="field-label">{t("menubarLabel")}</span>
        <div className="field-stack">
          <Segmented
            value={kind}
            options={[
              ["name", t("labelName")],
              ["emoji", t("emoji")],
              ["dot", t("dot")],
            ]}
            onChange={setKind}
          />
          {kind === "emoji" && (
            <input
              className="text-input short"
              value={text}
              maxLength={4}
              placeholder="💼"
              onChange={(event) => setText(event.target.value)}
            />
          )}
          {kind === "name" && <span className="field-help">{t("labelNameHelp")}</span>}
        </div>
      </div>

      <div className="field">
        <span className="field-label">{t("color")}</span>
        <div className="swatches">
          {PALETTE.map((swatch) => (
            <button
              key={swatch}
              className={`swatch${swatch === color ? " active" : ""}`}
              style={{ background: swatch }}
              title={swatch}
              onClick={() => setColor(swatch)}
            />
          ))}
        </div>
      </div>

      {account.provider === "claude" && <ApiKeyField accountId={account.id} />}

      <div className="editor-actions">
        <button className={`text-button danger${confirmingRemove ? " confirming" : ""}`} onClick={remove}>
          {confirmingRemove ? t("confirmRemove") : t("remove")}
        </button>
        {account.provider === "claude" && (
          <button className="text-button" onClick={() => reloginAccount(account.id)}>
            {t("relogin")}
          </button>
        )}
        <span className="spacer" />
        <button className="text-button" onClick={onDone}>
          {t("cancel")}
        </button>
        <button className="push-button" onClick={save}>
          {t("save")}
        </button>
      </div>
    </div>
  );
}
