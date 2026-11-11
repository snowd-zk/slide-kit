# AGENTS.md — 한국어 16:9 HTML 슬라이드 덱 제작 규칙

너는 이 폴더(kit)에서 `input/draft.md`(슬라이드 초안)를 받아 1440×810 HTML 덱을 만들고, 빌드, 렌더 및 검사까지 끝낸 뒤 보고한다.
아래 규칙은 모든 요청에 기본 적용한다. 사용자가 명시적으로 다르게 지시한 항목만 예외로 한다.

## 폴더
- `template/deck-template.html` — CSS 전체 + 아이콘 스프라이트 + 컴포넌트별 샘플 슬라이드 11장. 새 덱의 출발점.
- `examples/gyeonggi-interim/deck-src.html` — 실제 완성 덱 22장(작업 예시). 템플릿에 없는 컴포넌트(병합 도식, 도넛 게이지, 와이어프레임, 2×2 기준 허브, 정상/실패 행, 분기 도식, 효과 카드)는 여기서 찾아 쓴다. 내용(주제)은 복사하지 않는다.
- `input/draft.md` — 이번 덱의 초안(정본). 형식은 `input/draft-template.md` 참고.
- `scripts/build.py` (또는 `/Users/SD/Documents/Antigravity/bin/slide_build`) — 폰트 서브셋 및 이미지 내장 → 자가완결 HTML 1개.
- `scripts/slide_render` (또는 `/Users/SD/Documents/Antigravity/bin/slide_render`, `scripts/render.js`) — Rust 기반 슬라이드별 PNG(2560×1440) + PDF + 6대 레이아웃 게이트 검사.
- `fonts/` — 사용 폰트 6개(수정 금지). `decks/` — 덱 작업 폴더.

## 작업 순서
1. `input/draft.md`를 끝까지 읽는다. 장 수, 장별 [헤드라인]·[시각 자료]·본문 및 핵심 요약(Takeaway), 머리말의 색 토큰을 목록으로 정리한다.
2. 작업 폴더를 만든다: `decks/<slug>/` (slug는 영문 소문자, 숫자 및 하이픈).
   - `template/deck-template.html` → `decks/<slug>/deck-src.html`
   - `template/assets/` → `decks/<slug>/assets/`
3. 장마다 [시각 자료] 성격에 맞는 샘플 슬라이드를 골라 복제하고 텍스트를 초안 문구로 바꾼다. 쓰지 않는 샘플은 지운다.
   - 흐름 및 절차 → `flow-steps`, `asis-tobe-flow` / 표 및 진척 → `table-bars` / 대안 비교 → `option-cards`, `comparison-table`
   - 구조 및 계층 → `layer-stack` / 주체 간 순서 → `sequence` / 일정 → `gantt` / 결정 요청 → `decisions` / 마무리 → `qna`
   - mermaid 코드나 표가 초안에 있으면 그 노드, 라벨 및 행을 그대로 HTML 컴포넌트로 옮긴다(mermaid 렌더러 사용 금지).
   - 상단 바: `PART`, 섹션명, 소제목, 쪽번호 `NN / 전체`를 장마다 맞춘다. `data-name`은 영문 슬러그(PNG 파일명이 된다).
4. 빌드: `/Users/SD/Documents/Antigravity/bin/slide_build --src decks/<slug>/deck-src.html --out decks/<slug>/out/<slug>.html`
5. 렌더+게이트: `/Users/SD/Documents/Antigravity/bin/slide_render decks/<slug>/out/<slug>.html decks/<slug>/out/png --pdf decks/<slug>/out/<slug>.pdf`
   - 반드시 **빌드된 HTML**을 렌더한다(deck-src.html 직접 렌더 시 외부 요청으로 FAIL). (호환용: `node scripts/render.js` 가능)
6. `GATE: FAIL`이면 표시된 슬라이드 및 요소만 고치고 4~5를 다시 돈다. `GATE: PASS`가 될 때까지 반복한다.
7. 게이트 PASS 후 `out/png/`의 **모든 PNG를 하나씩 직접 열어** 본다(샘플링 금지). 확인 항목:
   두부(빈 네모) 글자, 잘린 글자, 단어 중간 줄바꿈, 요소 겹침, 라벨 없는 화살표, 하단 1/3 공백, 이모지, 초안과 다른 문구 및 수치.
   문제가 있으면 해당 요소만 고치고 4~7을 다시 한다.
8. 보고한다(아래 "보고 형식").

## 절대 규칙
- **초안 문구 그대로.** 헤드라인, 본문, 수치 및 고유명사를 줄이거나 바꾸지 않는다. 바꿀 수밖에 없는 경우(표기 변환, 이모지 제거, 도식용 짧은 라벨 추가)는 모두 `decks/<slug>/text-changes.md`에 "장 / 변경 전 / 변경 후 / 이유" 표로 기록한다.
- **지어내지 않는다.** 초안에 없는 수치, 주장, 고객명 및 성과를 만들지 않는다. 화살표 라벨 등 새로 넣는 짧은 말은 초안 문장에서 가져오고, 그래도 없으면 text-changes.md B절에 "추가 라벨"로 남긴다.
- **캔버스 1440×810 고정.** `.slide` 크기 및 패딩을 바꾸지 않는다. 구조: 상단 바(`.top`) → 헤드라인(`h2.hl`) → 본문(`.bd`) → 핵심 요약 바(`.tk`). 표지 및 Q&A만 예외.
- **핵심 요약 바 필수.** 표지 및 Q&A를 뺀 모든 장에 `.tk`(라벨 "핵심 요약" + 초안 Takeaway 문장)를 둔다.
- **모든 화살표에 라벨.** `.ar`/`.va`/`.msg`에는 "무엇이 이동하는지"를 쓴 `<span>`을 반드시 넣는다. 라벨 없는 화살표 금지. 방향 및 종류가 다른 흐름은 색(`.c-m/.c-z/.c-a/.c-d`)과 범례로 구분한다. 화살촉 없는 연결선(`.tree` 등)은 구조선일 때만 쓴다.
- **이모지 금지.** 아이콘은 `<svg class="ic s28"><use href="#i-이름"/></svg>`(스프라이트) 인라인 SVG만. 필요한 아이콘이 없으면 같은 형식(24×24, `fill="none" stroke="currentColor"`)의 `<symbol>`을 스프라이트에 추가한다. 초안의 ✔ ⏳ 💡 등은 SVG 아이콘 및 라벨로 바꾸고 기록한다.
- **하단 1/3을 비우지 않는다.** 본문이 아래까지 차야 한다. 비면 글자 크기(`t18→t20`), 카드 높이(`flex:1`, `.grow`), 세로 배분(`justify-content:space-between`)으로 채운다. 더미 내용을 넣어 채우지 않는다.
- **폰트.** 제목, 강조 및 라벨 = Gmarket Sans(`var(--G)`), 본문 = Pretendard(`var(--P)`). `@font-face` 6개 외 폰트 및 굵기를 추가하지 않는다. 시스템 폰트(맑은 고딕 등) 지정 금지.
- **색은 토큰만.** 색은 `:root` 토큰 블록 한 곳에서만 정의한다. 컴포넌트 및 인라인 스타일에는 `var(--pri)` 같은 토큰만 쓰고 색 코드(#xxxxxx)를 새로 쓰지 않는다.
- **외부 리소스 금지.** 외부 URL, CDN, 웹폰트 링크, `<script src>`, `@import`를 쓰지 않는다. 이미지는 `assets/` 상대경로로만 참조한다(빌드 때 내장된다).
- **요소 단위 수정.** 피드백 및 게이트 실패는 해당 요소(텍스트 한 줄, 아이콘 하나, 카드 하나)만 고친다. 덱 전체를 처음부터 다시 생성하지 않는다(이미 고친 부분이 되돌아간다). 여러 지적은 모아서 한 번에 반영한다.
- **게이트 우회 금지.** 게이트를 통과시키려고 `data-bleed`/`data-overlay`를 남용하거나 `overflow:hidden`으로 넘침을 숨기지 않는다. `data-bleed`는 표지 이미지처럼 의도적으로 슬라이드 밖으로 나가는 장식에만, `data-overlay`는 의도적 겹침에만 쓴다.
- 게이트 및 육안 검사를 하지 않았으면 "검사했다"고 쓰지 않는다.

## 레이아웃 요령
- 줄바꿈: `word-break:keep-all`이 기본이다. 숫자 묶음 및 괄호가 갈라지면 `<span style="white-space:nowrap">`로 묶는다. 제목은 필요하면 `<br>`로 끊는다.
- 텍스트 넘침(TEXT_OVERFLOW): 박스 폭을 늘리거나 글자를 1단계 줄인다. 문구는 줄이지 않는다.
- BIG_GAP / EMPTY_BOTTOM: 블록에 `flex:1`·`.grow`를 주거나 글자를 키운다.
- 슬라이드 고유 스타일이 필요하면 해당 `<section>` 바로 위에 `<style>`을 두고, 클래스 이름은 기존 이름과 겹치지 않게 짓는다(겹치면 다른 장이 깨진다).
- 시퀀스 및 간트의 좌표 규칙은 템플릿 해당 샘플의 주석을 따른다.

## 팔레트 바꾸기
- `deck-src.html` 상단 `:root` 블록의 "(1) 핵심 색" 12개만 바꾼다. "(2) 파생 색"은 `color-mix`로 자동 계산되므로 보통 손대지 않는다.
- 초안 머리말에 색 토큰이 있으면 이름을 다음처럼 대응시킨다: 본문 글자→`--ink`, 주 색→`--pri`, 짙은 헤더→`--navy`, 보조 강조→`--proof`, 보조 액센트→`--act`, 보조 설명→`--slate`, 연한 배경→`--soft`, 테두리→`--line`.
- 로고: `decks/<slug>/assets/logo.svg`를 교체(파일명 유지). 표지 이미지: `assets/cover.jpg`를 교체(가로형, 약 1800px 폭 권장).

## 보고 형식 (마지막 응답)
- 산출물 경로: 자가완결 HTML, PDF, PNG 폴더, text-changes.md
- 게이트 결과: `GATE: PASS` 줄과 장 수
- 육안 검사: 확인한 장 수(전체여야 함)와 발견→조치 목록
- 초안 대비 변경: text-changes.md 요약(없으면 "없음")
- 남은 문제 및 확인 필요 사항
