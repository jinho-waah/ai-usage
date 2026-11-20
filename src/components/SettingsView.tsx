import { t } from "../i18n";
import type { Settings } from "../types";
import { ChevronIcon } from "./Icons";

interface SettingsViewProps {
  settings: Settings;
  onChange: (settings: Settings) => void;
  onBack: () => void;
}

/** 설정 화면 (M1: 메뉴바 표시, 퍼센트 기준) */
export function SettingsView({ settings, onChange, onBack }: SettingsViewProps) {
  return (
    <div className="settings">
      <header className="header">
        <button className="icon-button" onClick={onBack} title={t("back")}>
          <ChevronIcon direction="left" size={12} />
        </button>
        <span className="title">{t("settings")}</span>
        <span className="header-spacer" />
      </header>

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
            ["used", t("used")],
            ["remaining", t("remaining")],
          ]}
          onChange={(percentMode) => onChange({ ...settings, percentMode })}
        />
        <p className="setting-help">{t("percentModeHelp")}</p>
      </section>

      <p className="settings-note">{t("settingsNote")}</p>
    </div>
  );
}

/** macOS 스타일 세그먼트 버튼 (둘 중 하나 고르기) */
function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: [T, string][];
  onChange: (value: T) => void;
}) {
  return (
    <div className="segmented" role="radiogroup">
      {options.map(([optionValue, label]) => (
        <button
          key={optionValue}
          role="radio"
          aria-checked={value === optionValue}
          className={`segment${value === optionValue ? " active" : ""}`}
          onClick={() => onChange(optionValue)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
