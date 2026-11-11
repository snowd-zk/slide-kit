# Licenses — bundled fonts and sample assets

## Fonts (`fonts/`)

| Font | Files | License | Official source |
|---|---|---|---|
| Pretendard | Pretendard-Regular.otf, -Medium.otf, -SemiBold.otf, -Bold.otf | SIL Open Font License 1.1 (OFL-1.1) | https://github.com/orioncactus/pretendard |
| Gmarket Sans | GmarketSansTTFBold.ttf, GmarketSansTTFMedium.ttf | Free font provided by Gmarket (free for personal and commercial use per the publisher's notice) | https://corp.gmarket.com/fonts |

- The original license text files were not present in the source folder this kit was assembled from.
  **Verify the current license terms on each official site before redistribution or commercial use.**
- Pretendard (OFL-1.1): may be used, embedded, and redistributed; the font may not be sold by itself;
  modified versions must not use the reserved font name. Full text: https://openfontlicense.org
- The build script embeds a subset of these fonts (woff2, base64) into the output HTML. Under OFL-1.1, embedding in documents is permitted.

## Sample assets

- `template/assets/logo.svg`, `examples/gyeonggi-interim/assets/zkrypto_lockup_color.svg` — company logo of the example deck's author. Replace with your own logo; do not reuse for other organizations' materials.
- `template/assets/cover.jpg`, `examples/gyeonggi-interim/assets/cover.jpg` — sample cover illustration from the example deck. Replace for your own decks.

## Code

- `scripts/`, `template/` HTML/CSS and the inline SVG icon set: internal kit, free to modify for your decks.
- `puppeteer` (installed by `npm install`): Apache-2.0.
- `fonttools` (pip): MIT. `brotli` (pip): MIT.
