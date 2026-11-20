# Claude Usage (v3.4.0) 분석 노트 — REA

새 usage 앱을 만들기 위한 참고 자료. 대상 앱을 리버스 엔지니어링한 결과를 정리했다.

- 대상: `/Applications/Claude Usage.app` (`HamedElfayome.Claude-Usage` 3.4.0, build 19). MIT 오픈소스 [hamed-elfayome/Claude-Usage-Tracker](https://github.com/hamed-elfayome/Claude-Usage-Tracker)
- 도구: REA 6.10.1 CLI. `native-macos`와 `rea-artifact-graph` 프로바이더만 사용했고, 디컴파일 엔진(Ghidra/Hopper/IDA/Kuna)은 없었다.
- 표기
  - `[REA ev_…]`: REA Evidence ID
  - `[관찰]`: 일반 셸 도구(`strings`, 디스크 파일)로 본 사실
  - `[추론]`: 근거에서 이끌어 낸 추정

## 1. 구성

| 구성 요소 | 내용 | 근거 |
|---|---|---|
| 메인 앱 | Swift/SwiftUI와 AppKit. 유니버설(x86_64/arm64), macOS 13 이상. `LSUIElement`(Dock 아이콘 없음). Sparkle 2.8.1 내장. DynamicNotchKit 정적 링크 | [REA ev_51dff3e4] [REA ev_9bc3bbd8] |
| 메인 앱 권한 | 샌드박스 없음. hardened runtime. `network.client`, `disable-library-validation` | [REA ev_450991b8] |
| 위젯 확장 | WidgetKit, macOS 14 이상, 샌드박스. `~/Library/Application Support/Claude Usage/widget/` 읽기 전용 temporary exception | [REA ev_e69c293f] [REA ev_1d386cf0] |
| 링크 프레임워크(앱) | SwiftUI, AppKit, **Charts**, WidgetKit, Combine, WebKit, Security, ServiceManagement, UserNotifications, IOKit, Carbon, CryptoKit, Network | [REA ev_8ca4651c] |
| 링크 프레임워크(위젯) | SwiftUI, WidgetKit, AppIntents (Charts 없음) | [REA ev_8ca4651c] |
| 에셋 | AccentColor, AppIcon, 로고 3개뿐. 색상과 메뉴바 아이콘은 코드로 그림 | [REA ev_01165d55] |

## 2. 메뉴바

- 메뉴바는 AppKit `NSStatusItem`, `NSPopover` 조합으로 직접 구현했다. 임포트에 `NSStatusBar`, `NSStatusItem`, `NSStatusBarButton`, `NSPopover`, `NSEvent`, `NSMenu`가 있다. SwiftUI `MenuBarExtra` 임포트는 관찰되지 않았다. 다만 임포트 목록이 완전하지 않을 수 있다. [REA ev_9bc3bbd8] [REA ev_8a627539]
- `MenuBarManager`가 가진 저장 프로퍼티:
  - `statusItem`, `popover`, `eventMonitor`(바깥 클릭 감지), `detachedWindow`, `refreshTimer`
  - `consecutiveRefreshFailures`, `lastSuccessfulRefreshTime`
  - `cachedImage`, `cachedImageKey`, `cachedIsDarkMode`(아이콘 캐시)
  - 옵저버: wake, screen, iconStyle, iconConfig, credentials, displayMode, refreshInterval

  [REA ev_3dcf3e8e]
- `StatusBarUIManager`는 지표마다 상태 아이템을 따로 만든다(`statusItems`, `multiProfileStatusItems`). 렌더링은 `MenuBarIconRenderer`가 맡는다. [REA ev_3dcf3e8e]
- 아이콘은 `NSImage`, `NSBezierPath`, `NSAttributedString`으로 직접 그리고, Aqua/DarkAqua 외형을 감지한다. [REA ev_9bc3bbd8]
- ObjC 셀렉터:
  - 팝오버: `togglePopover:`, `popoverShouldDetach:`, `detachableWindowForPopover:`(팝오버를 떼어 창으로 만드는 기능)
  - 컨텍스트 메뉴: `contextMenuRefresh`, `preferencesClicked`, `quitClicked`

  [REA ev_3dcf3e8e]
- 설정 항목 (UI 문구 테이블) [REA ev_e44f2753]
  - 지표: 세션(5시간), 주간, API 크레딧. 드래그로 순서를 바꾼다. 모두 끄면 앱 로고만 표시한다.
  - 단색(adaptive) 또는 상태색(초록/주황/빨강)
  - 라벨 `S:` / `W:` / `API:` 표시, 남은 % 표시(배터리처럼)
  - 진행 바 위의 시간 마커, 페이스 기반 색(초록에서 보라), 주말 제외 주간 페이스
  - 다음 세션까지 카운트다운(`→2H`)

## 3. 팝오버

- 뷰: `PopoverContentView`, `SmartHeader`, `SmartUsageDashboard`, `UsageRow`, `ContextualInsights`, `BottomBarSection`, `StatusBannerView`, `ProfileSwitcherCompact`. 배경은 `NSVisualEffectView` 래퍼를 쓴다. [REA ev_3dcf3e8e]
- 상태 관리는 Combine `ObservableObject`(`MenuBarManager`, `ProfileManager` 등)로 한다. macOS 13을 지원하려고 `@Observable`을 쓰지 않은 것으로 보인다. [REA ev_97b07c61] [추론]
- 표시 항목 [REA ev_e44f2753]
  - 세션, 주간, 모델별(Opus/Sonnet/Fable/Design), Extra Usage, API 비용·크레딧, 월 지출
  - 리셋 표시: `Resets in %@ (%@)`. 남은 시간, 리셋 시각, 둘 다 중에서 고른다. 12/24시간 표기
  - Claude 서비스 상태(status.claude.com)
  - 배너: `Updated %dm ago`, `Refresh failed %d times`, `Credentials expired`
  - 인사이트 3종: Efficient Usage, High Session Usage, Weekly Limit Approaching

## 4. 그래프 (사용량 히스토리)

- Swift Charts를 쓴다. [REA ev_8ca4651c]
- 앱 전체에서 쓰는 Charts API는 다음과 같다. 어느 차트 뷰가 어떤 마크를 쓰는지는 정적 분석만으로 알 수 없다. [REA ev_8a627539]
  - `Chart(data)`
  - `LineMark(x:y:)`, `LineMark(x:y:series:)`
  - `AreaMark(x:y:stacking:)`, `BarMark(… stacking:)` + `.cornerRadius`, `PointMark`
  - `.interpolationMethod`, `.lineStyle(StrokeStyle)`, `.symbolSize`, `.foregroundStyle`
  - `AxisMarks(values: .stride(by: Calendar.Component, count:))`, `.automatic(desiredCount:)`, `position: .leading`
  - `AxisGridLine(stroke:)`, `AxisValueLabel(format:)`
  - `PlottableValue.value(_:Date, unit: Calendar.Component)`(날짜 단위 묶음)
- 뷰와 모델: `UsageHistoryView`, `CombinedUsageChart`, `BillingCycleChart`, `DailyCostChart`, `EmptyHistoryView` / `ChartDataPoint`, `ChartTimeScale`, `ChartStyle`, `UsageSeries` [REA ev_3dcf3e8e]
- 화면 구성: 탭(Sessions / Weekly / Billing), 기간(5시간 / 24시간 / 7일 / 30일), "Now" 표시, CSV/JSON 내보내기, Clear, 레코드 목록 [REA ev_e44f2753]
- 저장 형식 [관찰]: `~/Library/Application Support/Claude Usage/history/usageHistory_<profileId>.json` → `{ "snapshots": [...] }`

  | resetType | 개수(=상한으로 추정) | 기록 간격(중앙값) | 주요 필드 |
  |---|---|---|---|
  | `sessionReset` | 1000 | 약 11분 | `sessionPercentage`, `sessionTokensUsed` |
  | `weeklyReset` | 500 | 약 2시간 | `weeklyPercentage`, `opus/sonnet/fable/designWeeklyPercentage`, `*TokensUsed` |
  | `billingCycle` | 100 | — | `apiSpendCents`, `apiPrepaidCreditsCents`, `apiCurrency` |

  - 공통 필드: `id`, `timestamp`(2001-01-01 기준 초, `JSONEncoder` 기본값), `triggeringResetTime`, `provider`
  - `*TokensUsed`는 914개 샘플 전부 `Percentage × 10,000`이다. 실제 토큰 수가 아니라 100만을 가정한 가상 값이다. [관찰]
  - `UsageHistoryService`가 가진 프로퍼티: `maxSession/Weekly/BillingSnapshots`, `sessionRecordingInterval`, `weeklyRecordingInterval`, `historyCache` [REA ev_3dcf3e8e]

## 5. 위젯

- 위젯 6종: `SessionUsageWidget`, `UsageRingsWidget`, `UsageDialWidget`, `UsageOverviewWidget`, `NextResetWidget`, `ActiveProfileWidget`. 모두 `ClaudeUsageWidgetBundle`에 들어 있다. [REA ev_3c8cc26f]
- 타임라인 프로바이더는 두 가지다. 고정형 `UsageTimelineProvider`와, 프로필을 고르는 `ProfileTimelineProvider`(AppIntent)다. 후자는 `SelectProfileIntent`, `WidgetProfileEntity`, `WidgetProfileQuery`를 쓴다. 엔트리 타입은 `UsageEntry`다. [REA ev_3c8cc26f]
- 위젯 뷰(`MeterRing`, `MeterRow`, `MeterTrack`, `PercentText`, `ProfileRings` …)는 Charts 없이 직접 그린다. [REA ev_3c8cc26f] [REA ev_8ca4651c]
- 데이터 경로
  1. 메인 앱의 `WidgetSnapshotService`가 `widget/snapshot.json`을 쓴다.
  2. `WidgetCenter.shared.reloadAllTimelines()`로 위젯을 다시 그리게 한다. 호출 빈도는 `reloadInterval`, `heartbeatInterval`로 조절한다.

  [REA ev_3dcf3e8e] [REA ev_8a627539]
- `snapshot.json` 구조 (schemaVersion 2) [관찰]:

  ```
  { generatedAt, schemaVersion,
    profiles: [{ id, name, isActive, isDisplayed, hasSessionLimit,
                 sessionPercentage, sessionResetTime, weeklyPercentage, weeklyResetTime, lastUpdated }] }
  ```
- App Group을 쓰지 않고 temporary exception으로 데이터를 공유한다. 이 방식은 App Store 배포에서 통하지 않는다. [추론]

## 6. 데이터 소스와 인증 [관찰: 바이너리 문자열]

- **claude.ai 쿠키 방식:** `https://claude.ai/api/organizations/{orgId}/usage`, `/overage_spend_limit`, `/overage_credit_grant`, `/prepaid/credits`
  - 쿠키는 `sessionKey`와 `cf_clearance`를 쓴다.
  - 로그인은 `WKWebView`로 받고, 쿠키는 `NSHTTPCookieStorage`에서 꺼낸다.
- **Claude Code OAuth 방식:** `https://api.anthropic.com/api/oauth/usage`(`anthropic-beta: oauth-2025-04-20`)
  - 토큰 갱신: `https://platform.claude.com/v1/oauth/token`
  - 키체인 서비스 `Claude Code-credentials`에서 읽는다(환경설정 값으로 확인).
- **응답 헤더:** `anthropic-ratelimit-unified-5h-utilization`, `-5h-reset`, `-7d-utilization`, `-7d-reset`를 파싱한다(`/v1/messages`).
- **Console(API 결제):** `https://console.anthropic.com/api` … `/workspaces/default/usage_cost`
- **상태:** `https://status.claude.com/api/v2/status.json`
- **Codex:** `https://chatgpt.com/backend-api/wham/usage`
- **기타 외부 호출:** 하트비트(`claude-usage-tracker.hamedelfayome.workers.dev?type=heartbeat`), 스폰서(`claudetracker.com/api/sponsor`), GitHub API
- **응답 JSON 키 일부:** `five_hour`, `seven_day`, `seven_day_sonnet`, `seven_day_omelette`, `utilization`, `resets_at`, `extra_usage`, `is_enabled`, `used_credits`. Swift는 15바이트 이하 문자열을 코드 안에 직접 넣기 때문에 일부 키는 문자열 덤프에 나오지 않는다.
- **세션 자동 시작:** 세션이 0%로 리셋되면 Claude 3.5 Haiku에 'Hi'를 보내 새 5시간 창을 시작한다(베타). [REA ev_e44f2753]

## 7. statusline (나중에 mod로 만들 부분)

앱에 내장된 bash 템플릿 [관찰]은 아래 순서로 사용량을 구한다.

1. 앱이 남긴 캐시 (5분 이내)
2. **Claude Code가 statusline stdin으로 넘기는 `five_hour` / `seven_day`의 `used_percentage`와 `resets_at`** (템플릿 주석: "Claude Code reports the session's own rate limits on stdin")
3. `swift ~/.claude/fetch-claude-usage.swift`. 앱이 세션 키와 cf_clearance 쿠키를 **평문으로 넣어 둔** 스크립트다.
4. 6시간 이내의 오래된 캐시 (`(stale Nh)` 표시)

→ mod는 2번(stdin)만으로 만들 수 있을 가능성이 높다. 그러면 세션 키가 필요 없다. [추론, Claude Code 문서로 확인 필요]

## 8. 보안 관찰

- statusline 기능이 `~/.claude/fetch-claude-usage.swift`(0700)에 claude.ai `sessionKey`와 `cf_clearance`를 평문으로 기록한다. 업스트림 `StatuslineService.swift`가 의도한 동작이다. README의 "평문 파일에 저장하지 않음"이라는 설명과 다르다. [관찰]
- REA 노출 점검에서는 배포된 번들 안에 자격 증명 흔적이 없었다. 문제는 번들이 아니라 런타임에 생성되는 파일이다. [REA ev_3b0ca0a2]

## 9. 새 앱 설계에 주는 시사점 (설계 선택)

- 메뉴바: `NSStatusItem` 위에 직접 그린 `NSImage`와 `NSPopover`(SwiftUI 호스팅)를 조합한다. `MenuBarExtra`보다 아이콘과 팝오버를 세밀하게 다룰 수 있다.
- 비밀값은 키체인에만 둔다. 스크립트나 파일에 넣지 않는다.
- 위젯 데이터는 App Group 컨테이너로 공유한다(temporary exception 대신).
- API는 퍼센트만 주므로 가짜 토큰 수는 표시하지 않는다.
- 히스토리는 상한 있는 링 버퍼, 기간 필터, Swift Charts로 만든다. 타임스탬프는 ISO8601로 저장하면 다루기 쉽다.
- 아직 확인하지 못한 것: 차트 뷰와 마크의 대응, 갱신 주기 기본값, 에러/백오프 세부 동작. 필요하면 MIT 소스를 보거나 Ghidra로 디컴파일한다.

## Evidence 목록

| ID | 내용 |
|---|---|
| ev_c74f7ee1 | `inspect-desktop-application` (197개 항목, 전체) |
| ev_9bc3bbd8 | `inspect-macho` (로드 커맨드, 의존성, 임포트) |
| ev_450991b8 / ev_1d386cf0 | `inspect-signature` 앱 / 위젯 |
| ev_51dff3e4 / ev_e69c293f | `inspect-plist` 앱 / 위젯 Info.plist |
| ev_8ca4651c | `trace-dylib-resolution` |
| ev_3b0ca0a2 | `audit-release-exposure` |
| ev_01165d55 | `inspect-asset-catalog` |
| ev_3dcf3e8e / ev_3c8cc26f | `inspect-native-dispatch-metadata` 앱 / 위젯 |
| ev_97b07c61 / ev_8a627539 | `demangle-swift` 프로토콜 / Charts·WidgetKit 임포트 |
| ev_e44f2753 | `inspect-plist` en.lproj/Localizable.strings (880개 항목) |
