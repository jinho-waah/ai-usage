# AI Usage

여러 Claude 계정과 OpenAI Codex의 사용 한도를 macOS 메뉴바에서 한눈에 보는 앱입니다.
Rust([Tauri](https://tauri.app))와 React로 만듭니다.

> 현재 **M4(히스토리 창)** 까지 됐습니다. 계획과 남은 단계는 [`docs/spec.md`](docs/spec.md)에 있습니다.

## 할 수 있는 것

- **메뉴바:** 계정마다 주간 한도를 막대(또는 숫자)로. 세로 눈금은 시간, 페이스보다 빨리 쓰면 빨강.
- **팝오버:** 계정 카드(이름 옆에 로그인한 이메일, 주간·5시간 한도, 소진 예측, 모델별 한도, Extra usage, 이번 달 API 비용, 최근 7일 그래프),
  하단에 Claude·Codex 누적 토큰(API 가격으로 환산한 추정 금액 포함)과 서비스 상태.
- **히스토리 창:** 계정별 주간·5시간 사용률 추이, 주간 주기 비교(이번 주기 vs 지난 주기), 표 보기.
- **알림:** 로그인이 풀렸을 때, Claude·OpenAI 상태 페이지에 장애가 올라왔을 때,
  한도를 설정한 만큼 썼을 때와 그 한도가 리셋됐을 때(기본 10% 남음, 설정에서 바꾸거나 끔).
- **로그인할 때 자동으로 열기** (설치한 앱에서, 처음 켤 때 켜짐. 설정에서 끌 수 있음)

## 원본 앱(Claude Usage)과 비교

[Claude Usage Tracker](https://github.com/hamed-elfayome/Claude-Usage-Tracker) 3.4.0(아래 "원본")을 대신하려고 만들었습니다.
숫자는 2026-11-21에 macOS 27.2에서 직접 잰 값입니다.

### 성능

| 항목 | 원본 3.4.0 | AI Usage 0.1.0 | 차이 |
|---|---|---|---|
| CPU 사용 (대기 10분 누적) | 2.36초 (평균 0.39%) | 1.54초 (평균 0.26%) | **약 35% 적음** |
| 메모리 (phys_footprint 합계) | 31 MB | 144 MB | 약 4.6배 많음 |
| 프로세스 수 | 1개 | 7개 (본체 39 MB + WebKit 보조 6개) | |
| 앱 크기 | 38 MB | 8 MB | **약 79% 작음** |
| 실행 파일 (Apple Silicon 부분만) | 16.1 MB | 7.4 MB | **약 54% 작음** |
| 디스크에 쌓인 데이터 | 91 MB (캐시 86 MB, 약 2.5개월 사용) | 35 MB (기록 DB 22 MB, 로그인 저장소 12 MB) | |

- CPU·메모리·프로세스 수는 두 앱을 같은 Mac에서 동시에 켜 두고 잰 값입니다. 두 앱 모두 새로 켠 뒤 1분 기다리고, 팝오버를 열지 않은 채로 10분 동안 쟀습니다.
- 이 측정에서 AI Usage는 계정 4개(Claude 3, Codex 1)를 1분마다 조회하고, 5분마다 누적 토큰과 서비스 상태도 확인합니다.
  일을 더 하면서도 CPU는 덜 씁니다.
- 메모리는 AI Usage가 더 씁니다. claude.ai 계정마다 로그인 쿠키를 따로 두려고 계정별 웹뷰를 띄우기 때문에,
  WebKit 보조 프로세스(화면·네트워크·GPU)가 약 105 MB를 차지합니다. 앱 본체만 보면 39 MB로 비슷합니다.
- 원본이 큰 데는 이유도 있습니다. 인텔 Mac용 코드, 자동 업데이트(Sparkle), 위젯, 15개 언어를 함께 담고 있습니다. AI Usage는 Apple Silicon 전용입니다.
- AI Usage의 디스크 데이터는 설치 하루째 값입니다. 다만 지난 Claude Code·Codex 로그는 이미 전부 색인한 상태이고, 기록은 지우지 않고 계속 쌓습니다.
- 잰 방법: CPU는 `ps -o time`의 누적 CPU 시간 차이, 메모리는 `footprint`의 `phys_footprint`.
  WebKit 보조 프로세스는 앱을 켤 때 새로 생긴 것을 그 앱 몫으로 셌습니다. (원본은 켤 때 보조 프로세스를 만들지 않았습니다)
- 한 번 잰 값이라 상황(계정 수, 네트워크, 팝오버 사용)에 따라 달라질 수 있습니다.

### 좋아진 점

- **비밀값을 파일에 두지 않습니다.** 원본은 터미널 statusline 기능 때문에 claude.ai 세션 키와 `cf_clearance` 쿠키를
  `~/.claude/fetch-claude-usage.swift` 에 평문으로 씁니다(원본 README에는 "키체인에만 저장"이라고 적혀 있음).
  AI Usage는 claude.ai 로그인을 계정별 WebKit 저장소 안에만 두고 밖으로 꺼내지 않습니다. Admin API 키는 키체인에만 둡니다.
- **다른 도구의 로그인을 건드리지 않습니다.** 원본은 프로필을 바꾸면 Claude Code 로그인도 바꾸고, 그 로그인 정보의 사본을 자기 키체인에 보관합니다.
  AI Usage는 Claude Code 로그인에 손대지 않고, Codex 로그인(`~/.codex/auth.json`)은 읽기만 합니다.
- **사용량을 가져오는 곳 말고는 접속하지 않습니다.** 원본은 하루 한 번 하트비트, 6시간마다 스폰서 정보, 업데이트 확인을 보냅니다.
  AI Usage가 접속하는 곳은 claude.ai, chatgpt.com, 두 회사의 상태 페이지, 그리고 키를 넣었을 때만 Anthropic Admin API입니다.
- **실제 토큰 수와 API 환산 금액.** Claude Code·Codex 로그에서 토큰을 세고, API 가격으로 치면 얼마였을지 보여 줍니다.
  (원본 기록의 토큰 수는 사용률에 10,000을 곱해 만든 값입니다)
- **언제 바닥나는지 시각으로.** 지금 속도면 리셋 전에 다 쓰게 될 때 "이 속도면 월요일 오후 11시쯤 소진 예정"처럼 알려 줍니다. (원본은 페이스를 색으로 표시)
- **주간 주기 비교.** 히스토리 창에서 이번 주기와 지난 주기들을 같은 축에 겹쳐 봅니다.
- **OpenAI 상태도 함께.** 원본은 Claude 상태만 봅니다. AI Usage는 Codex가 쓰는 OpenAI 구성 요소도 보고, 장애가 올라오면 알림을 보냅니다.
- **원본 기록 가져오기.** 원본에 쌓인 사용률 기록을 옮겨 와서 그래프가 끊기지 않습니다.

### 원본에만 있는 것

위젯, 자동 업데이트, 터미널 statusline과 Dynamic Island, 세션 자동 시작, 한도에 닿으면 프로필 자동 전환, 단축키, 15개 언어, 인텔 Mac 지원.

## 계정 연결

- **Claude:** 팝오버 → "Claude 계정 추가" → 열리는 창에서 claude.ai 에 로그인합니다.
  앱 안의 브라우저에서는 **구글 로그인이 막혀 있으니 "이메일로 계속하기"** 를 쓰세요. (구글로 가입한 계정도 같은 이메일로 됩니다)
  계정마다 로그인 쿠키를 따로 저장하므로 여러 계정을 동시에 볼 수 있습니다.
- **Codex:** 팝오버 → "Codex 추가". 터미널에서 `codex login` 으로 로그인돼 있으면 바로 붙습니다. 앱은 `~/.codex/auth.json` 을 읽기만 합니다.
- **Claude API 비용 (선택):** 설정 → 계정 "편집" → "API 비용" 칸에 그 계정의 Claude Console 조직에서 만든
  **Admin API 키(`sk-ant-admin…`)** 를 넣으면 카드에 이번 달 API 비용이 나옵니다. 키는 계정마다 macOS 키체인에만 저장됩니다.
  데이터는 보통 몇 시간 늦게 반영됩니다.
- 화면 확인용 가짜 데이터로 실행하려면 `AI_USAGE_DEMO=1 npm run tauri dev`.

## 설치하기 (내 Mac에서 빌드)

필요한 것: macOS 14 이상, [Rust](https://rustup.rs), Node.js 20.19 이상, Xcode.

```bash
npm install
npm run tauri build -- --bundles app
ditto "src-tauri/target/release/bundle/macos/AI Usage.app" "/Applications/AI Usage.app"
open "/Applications/AI Usage.app"
```

- Apple 개발자 계정 없이 ad-hoc 서명으로 빌드합니다. 내 Mac에서 빌드한 앱은 바로 열립니다.
- 새로 빌드해서 덮어쓰면 키체인이 Admin API 키 접근을 다시 물을 수 있습니다. "항상 허용"을 누르세요.
- **설치한 앱과 개발 모드(`npm run tauri dev`)를 동시에 켜지 마세요.** 메뉴바 아이콘이 둘 생기고 같은 계정을 두 번 조회합니다.

## 개발 모드

```bash
npm install
npm run tauri dev
```

- 처음 실행할 때는 Rust 컴파일 때문에 몇 분 걸립니다.
- 앱이 뜨면 Dock이 아니라 **메뉴바**에 아이콘이 나타납니다. 누르면 팝오버가 열립니다.
- 개발 모드는 로그인 쿠키를 `~/Library/WebKit/ai-usage`, 설치한 앱은 `~/Library/WebKit/io.github.jinho-waah.ai-usage` 에 따로 둡니다.
  계정 목록·설정·기록 DB(`~/Library/Application Support/io.github.jinho-waah.ai-usage/`)는 같이 씁니다.
- `cargo: command not found` 가 나오면 Rust 경로를 셸에 추가하세요: `echo '. "$HOME/.cargo/env"' >> ~/.zshrc` 후 터미널을 다시 엽니다.

### 화면만 브라우저에서 보기

```bash
npm run dev
```

http://localhost:1420 은 팝오버, http://localhost:1420/#history 는 히스토리 창을 가짜 데이터로 보여 줍니다. (메뉴바는 없음)

### 메뉴바 아이콘을 PNG로 뽑아 보기

```bash
cd src-tauri
cargo run --example render_menubar -- ../menubar-previews
```

### 테스트

```bash
cd src-tauri && cargo test
```

## 데이터는 어디에

| 무엇 | 위치 |
|---|---|
| 계정 목록, 설정 | `~/Library/Application Support/io.github.jinho-waah.ai-usage/accounts.json`, `settings.json` (비밀값 없음) |
| 사용률 기록, 누적 토큰 | 같은 폴더의 `usage.db` (SQLite, 지우지 않고 계속 쌓음) |
| claude.ai 로그인 쿠키 | 계정마다 따로: `~/Library/WebKit/<앱 ID>/WebsiteDataStore/<계정 ID>/` |
| Admin API 키 | macOS 키체인 (`io.github.jinho-waah.ai-usage`, 계정마다 `admin-api-key:<계정 ID>`) |
| 누적 토큰의 원본 | Claude Code 로그 `~/.claude/projects`, Codex 로그 `~/.codex/sessions` (읽기만 함) |

## 프로젝트 구조

```
src/                    화면 (React + TypeScript)
  App.tsx               팝오버 전체
  history/              히스토리 창 (그래프, 눈금, 스타일)
  components/           계정 카드, 막대, 설정 화면 등
  api.ts                Rust 명령 호출
  palette.ts            계정 색 (색각 이상에서도 구분되게 검사한 8색)
  i18n.ts               한국어·영어 문구, 날짜·숫자 표기
src-tauri/              앱 본체 (Rust)
  src/lib.rs            시작점: 메뉴바 아이콘, 창, 주기 작업, 명령 등록
  src/menubar/          메뉴바 아이콘에 그릴 내용(mod.rs)과 macOS 그리기(macos.rs)
  src/popover.rs        팝오버 창 열기·닫기·위치
  src/model.rs          데이터 구조 (계정, 사용량, 요약)
  src/accounts.rs       계정 목록 저장 (비밀값 없음)
  src/claude.rs         claude.ai 응답 해석
  src/claude_web.rs     계정별 claude.ai 웹뷰 (로그인 창 겸 조회)
  src/webview_js.rs     웹뷰 안에서 자바스크립트 실행 (macOS)
  src/codex.rs          Codex 사용량 (CLI 로그인 읽기, 세션 로그 대체)
  src/history.rs        사용률 기록 DB
  src/history_view.rs   히스토리 창 데이터 (주기 나누기)
  src/tokens.rs         Claude Code·Codex 로그에서 누적 토큰 집계
  src/api_cost.rs       Admin API 로 이번 달 API 비용 (키는 키체인)
  src/pricing.rs        모델별 API 가격표 (구독 사용량의 API 환산 금액 추정)
  src/services.rs       Claude·OpenAI 상태 페이지
  src/alerts.rs         알림 (로그인 만료, 서비스 장애)
  src/login_item.rs     로그인할 때 자동으로 열기
  src/legacy.rs         원본 Claude Usage 앱 기록 가져오기
  src/refresh.rs        주기적으로 조회하고 메뉴바·팝오버에 알림
  src/mock.rs           데모용 가짜 데이터 (AI_USAGE_DEMO=1)
  tauri.conf.json       앱 이름, 창 설정
docs/                   스펙과 분석 문서
```

## 라이선스

추후 결정.
