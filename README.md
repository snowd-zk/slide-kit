# SlideKit (slide-kit)

Production 16:9 (`1440×810`) HTML presentation deck engineering system for Google Antigravity and AI agents.
Includes self-contained HTML generation, parallel WOFF2 font subsetting, 2560×1440 QHD screenshot rendering, integrated vector PDF export, and a 6-gate layout inspector.

## Features

- **Standard Widescreen Ratio**: 16:9 (`1440 × 810 px`) canvas.
- **Single Self-Contained HTML**: Zero external network dependencies (WOFF2 font subsetting + Base64 image inlining).
- **Native Rust Toolchain**:
  - `slide_build`: Sub-second HTML compiler (woff2 subsetting & base64 inlining).
  - `slide_render`: CDP-powered 6-gate layout inspection & 2560×1440 PNG / vector PDF rendering.
- **6-Gate Layout Inspector**: Automated detection of `OUT_OF_SLIDE`, `TEXT_OVERFLOW`, `OUT_OF_BODY`, `BIG_GAP`, `EMPTY_BOTTOM`, and `OVERLAP`.

## Installation

### Antigravity Skill
```bash
git clone https://github.com/snowd-zk/slide-kit.git
mkdir -p ~/.gemini/config/skills/slide-kit
cp -R slide-kit/skills/slide-kit/* ~/.gemini/config/skills/slide-kit/
```

### Build Native Rust Tools
```bash
cd slide-kit
cargo build --release --manifest-path crates/slide_render/Cargo.toml
cargo build --release --manifest-path crates/slide_build/Cargo.toml
```

## Usage

In your Antigravity chat, trigger with:
```
/slide-kit
```
