# AI Usage

여러 Claude 계정과 OpenAI Codex의 사용 한도를 macOS 메뉴바에서 한눈에 보는 앱입니다.
Rust([Tauri](https://tauri.app))와 React로 만듭니다.

> 현재 **M2(계정 연동)** 단계입니다. Claude 계정(앱 안 로그인)과 Codex(Codex CLI 로그인)의 실제 사용량을 보여 줍니다.
> 계획은 [`docs/spec.md`](docs/spec.md)에 있습니다.

## 계정 연결

- **Claude:** 팝오버 → "Claude 계정 추가" → 열리는 창에서 claude.ai 에 로그인합니다.
  앱 안의 브라우저에서는 **구글 로그인이 막혀 있으니 "이메일로 계속하기"** 를 쓰세요. (구글로 가입한 계정도 같은 이메일로 됩니다)
  계정마다 로그인 쿠키를 따로 저장하므로 여러 계정을 동시에 볼 수 있습니다.
- **Codex:** 팝오버 → "Codex 추가". 터미널에서 `codex login` 으로 로그인돼 있으면 바로 붙습니다. 앱은 `~/.codex/auth.json` 을 읽기만 합니다.
- 화면 확인용 가짜 데이터로 실행하려면 `AI_USAGE_DEMO=1 npm run tauri dev`.

## 실행하기 (개발 모드)

필요한 것: macOS 14 이상, [Rust](https://rustup.rs), Node.js 20.19 이상, Xcode.

```bash
npm install
npm run tauri dev
```

- 처음 실행할 때는 Rust 컴파일 때문에 몇 분 걸립니다.
- 앱이 뜨면 Dock이 아니라 **메뉴바**에 아이콘이 나타납니다. 누르면 팝오버가 열립니다.
- `cargo: command not found` 가 나오면 Rust 경로를 셸에 추가하세요: `echo '. "$HOME/.cargo/env"' >> ~/.zshrc` 후 터미널을 다시 엽니다.

### 화면만 브라우저에서 보기

```bash
npm run dev
```

http://localhost:1420 을 열면 팝오버 화면을 가짜 데이터로 볼 수 있습니다. (메뉴바는 없음)

### 메뉴바 아이콘을 PNG로 뽑아 보기

```bash
cd src-tauri
cargo run --example render_menubar -- ../menubar-previews
```

라이트·다크 모드와 막대·숫자 스타일별 아이콘이 PNG로 저장됩니다.

### 테스트

```bash
cd src-tauri && cargo test
```

## 프로젝트 구조

```
src/                    화면 (React + TypeScript)
  App.tsx               팝오버 전체
  components/           계정 카드, 막대, 설정 화면 등
  api.ts                Rust 명령 호출
  i18n.ts               한국어·영어 문구, 날짜·숫자 표기
src-tauri/              앱 본체 (Rust)
  src/lib.rs            시작점: 메뉴바 아이콘, 팝오버, 1분 갱신, 명령 등록
  src/menubar/          메뉴바 아이콘에 그릴 내용(mod.rs)과 macOS 그리기(macos.rs)
  src/popover.rs        팝오버 창 열기·닫기·위치
  src/model.rs          데이터 구조 (계정, 사용량, 요약)
  src/settings.rs       설정 저장
  src/accounts.rs       계정 목록 저장 (비밀값 없음)
  src/claude.rs         claude.ai 응답 해석
  src/claude_web.rs     계정별 claude.ai 웹뷰 (로그인 창 겸 조회)
  src/webview_js.rs     웹뷰 안에서 자바스크립트 실행 (macOS)
  src/codex.rs          Codex 사용량 (CLI 로그인 읽기, 세션 로그 대체)
  src/refresh.rs        1분마다 조회하고 메뉴바·팝오버에 알림
  src/mock.rs           데모용 가짜 데이터 (AI_USAGE_DEMO=1)
  tauri.conf.json       앱 이름, 창 설정
docs/                   스펙과 분석 문서
```

## 라이선스

추후 결정.
