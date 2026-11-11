// Render a deck HTML -> per-slide PNG (2560x1440) [+ PDF] and run the layout gate.
//
// usage: node scripts/render.js <deck.html> <png-outdir> [--pdf out.pdf] [--only 3,4] [--nopng]
//
// Gate checks per slide (any hit = FAIL):
//   OUT_OF_SLIDE   element sticks out of the 1440x810 slide
//   TEXT_OVERFLOW  text box content larger than the box (clipped / spilling)
//   OUT_OF_BODY    element leaves the .bd body region (overlaps headline or takeaway bar)
//   BIG_GAP        > 44px empty gap between stacked body blocks
//   EMPTY_BOTTOM   body content ends above 85% of the body height (empty lower third)
//   OVERLAP        two cards/nodes overlap (mark intentional overlays with data-overlay)
// Deck-level: every @font-face must be "loaded", and no external request may happen.
// Elements inside [data-bleed] are exempt (e.g. the cover image).
//
// Browser: puppeteer's bundled Chrome by default (downloaded by `npm install`).
// Optional env CHROME_PATH=/path/to/chrome-or-chrome-headless-shell to use another browser.
// Exit code: 0 = GATE PASS, 1 = GATE FAIL or error.
const path = require('path');
const fs = require('fs');
const { pathToFileURL } = require('url');

let puppeteer;
try { puppeteer = require('puppeteer'); }
catch (e) {
  console.error('puppeteer not found. Run `npm install` in the kit folder (see README).');
  process.exit(1);
}

const args = process.argv.slice(2);
if (args.length < 2 || args[0].startsWith('--')) {
  console.error('usage: node scripts/render.js <deck.html> <png-outdir> [--pdf out.pdf] [--only 3,4] [--nopng]');
  process.exit(1);
}
const html = path.resolve(args[0]);
const outdir = path.resolve(args[1]);
const pdfIdx = args.indexOf('--pdf');
const pdfOut = pdfIdx >= 0 ? path.resolve(args[pdfIdx + 1]) : null;
const onlyIdx = args.indexOf('--only');
const only = onlyIdx >= 0 ? args[onlyIdx + 1].split(',').map(Number) : null;
const noPng = args.includes('--nopng');
if (!fs.existsSync(html)) { console.error('not found: ' + html); process.exit(1); }

const EXE = process.env.CHROME_PATH || undefined;
// chrome-headless-shell (puppeteer default) -> 'shell'; a full Chrome binary -> new headless (true)
const HEADLESS = EXE && !/headless[-_]shell/i.test(EXE) ? true : 'shell';

(async () => {
  const browser = await puppeteer.launch({
    executablePath: EXE, headless: HEADLESS,
    args: ['--no-sandbox', '--font-render-hinting=none', '--allow-file-access-from-files'],
  });
  const page = await browser.newPage();
  const fileUrl = pathToFileURL(html).href;
  const external = [];
  page.on('request', r => { const u = r.url(); if (!u.startsWith('data:') && u !== fileUrl) external.push(decodeURI(u)); });
  await page.setViewport({ width: 1440, height: 810, deviceScaleFactor: 2560 / 1440 });
  await page.goto(fileUrl, { waitUntil: 'load' });
  // force-load every declared face, then wait
  await page.evaluate(async () => {
    const ps = [];
    document.fonts.forEach(f => ps.push(f.load().catch(() => null)));
    await Promise.all(ps); await document.fonts.ready;
  });
  const gate = await page.evaluate(() => {
    const res = { fonts: [], slides: [] };
    document.fonts.forEach(f => res.fonts.push(`${f.family} ${f.weight} ${f.status}`));
    const slides = [...document.querySelectorAll('.slide')];
    const CARD = '.card,.nd,.stp,.mdl,.chip,.lane,.res,.tk,.hl,.win,.box,.cell,.dc,.ef,.ll,.msgl,.ganttbar,.hub';
    slides.forEach((s, i) => {
      const issues = [];
      const sr = s.getBoundingClientRect();
      const name = el => (el.className && el.className.baseVal === undefined ? (el.tagName.toLowerCase() + '.' + String(el.className).split(' ').join('.')) : el.tagName.toLowerCase()) + ' "' + (el.textContent || '').trim().slice(0, 28) + '"';
      s.querySelectorAll('*').forEach(el => {
        if (el.closest('[data-bleed]') || el.closest('defs')) return;
        const r = el.getBoundingClientRect();
        if (r.width === 0 && r.height === 0) return;
        if (r.left < sr.left - 0.5 || r.top < sr.top - 0.5 || r.right > sr.right + 0.5 || r.bottom > sr.bottom + 0.5)
          issues.push('OUT_OF_SLIDE ' + name(el) + ` [${Math.round(r.left - sr.left)},${Math.round(r.top - sr.top)},${Math.round(r.right - sr.left)},${Math.round(r.bottom - sr.top)}]`);
        const hasText = [...el.childNodes].some(n => n.nodeType === 3 && n.textContent.trim());
        const cs = getComputedStyle(el);
        if (hasText && cs.display !== 'inline' && el.clientWidth > 0) {
          if (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1)
            issues.push('TEXT_OVERFLOW ' + name(el) + ` sw${el.scrollWidth}/cw${el.clientWidth} sh${el.scrollHeight}/ch${el.clientHeight}`);
        }
      });
      // body region containment
      s.querySelectorAll('.bd').forEach(bd => {
        const br = bd.getBoundingClientRect();
        bd.querySelectorAll('*').forEach(el => {
          if (el.closest('[data-bleed]')) return;
          const r = el.getBoundingClientRect();
          if (r.width === 0 && r.height === 0) return;
          if (r.bottom > br.bottom + 1 || r.top < br.top - 1) issues.push('OUT_OF_BODY ' + name(el) + ` bottom ${Math.round(r.bottom - br.bottom)}`);
        });
        // large vertical gaps between stacked body blocks
        const kids = [...bd.children].map(c => c.getBoundingClientRect()).filter(r => r.height > 0).sort((a, b) => a.top - b.top);
        for (let k = 1; k < kids.length; k++) { const g = kids[k].top - kids[k - 1].bottom; if (g > 40) issues.push('BIG_GAP ' + Math.round(g) + 'px between body blocks ' + k); }
        // empty lower third check: content bottom must reach >= 85% of body height
        let maxB = 0; bd.querySelectorAll('*').forEach(el => { const r = el.getBoundingClientRect(); if (r.height > 0) maxB = Math.max(maxB, r.bottom); });
        if (maxB < br.top + br.height * 0.85) issues.push('EMPTY_BOTTOM content ends at ' + Math.round((maxB - br.top) / br.height * 100) + '% of body');
      });
      // overlap of cards
      const cards = [...s.querySelectorAll(CARD)].filter(el => !el.closest('[data-bleed]'));
      for (let a = 0; a < cards.length; a++) for (let b = a + 1; b < cards.length; b++) {
        const A = cards[a], B = cards[b];
        if (A.contains(B) || B.contains(A)) continue;
        if (A.dataset.overlay !== undefined || B.dataset.overlay !== undefined) continue;
        const ra = A.getBoundingClientRect(), rb = B.getBoundingClientRect();
        const w = Math.min(ra.right, rb.right) - Math.max(ra.left, rb.left);
        const h = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top);
        if (w > 1 && h > 1) issues.push('OVERLAP ' + name(A) + ' <> ' + name(B) + ` ${Math.round(w)}x${Math.round(h)}`);
      }
      res.slides.push({ n: i + 1, id: s.id, name: s.dataset.name || s.id, issues });
    });
    return res;
  });
  gate.external = external;
  const badFonts = gate.fonts.filter(f => !f.endsWith('loaded'));
  let fail = badFonts.length > 0 || external.length > 0 || gate.slides.length === 0;
  console.log('SLIDES:', gate.slides.length);
  console.log('FONTS:', gate.fonts.length, 'faces;', badFonts.length ? 'NOT LOADED: ' + badFonts.join(', ') : 'all loaded');
  console.log('EXTERNAL REQUESTS:', external.length ? external.join(', ') + '  (render the BUILT html, not deck-src.html)' : 'none');
  for (const s of gate.slides) {
    if (only && !only.includes(s.n)) continue;
    const tag = `S${String(s.n).padStart(2, '0')} ${s.name}`;
    if (s.issues.length) { fail = true; console.log(`${tag} GATE FAIL (${s.issues.length})`); s.issues.slice(0, 12).forEach(x => console.log('   ' + x)); }
    else console.log(`${tag} GATE PASS`);
  }
  console.log('GATE:', fail ? 'FAIL' : 'PASS');
  fs.mkdirSync(outdir, { recursive: true });
  fs.writeFileSync(path.join(outdir, 'gate-result.json'), JSON.stringify(gate, null, 1));

  if (!noPng) {
    const names = gate.slides.map(s => String(s.name).replace(/[\\/:*?"<>|\s]+/g, '-'));
    const handles = await page.$$('.slide');
    let n = 0;
    for (let i = 0; i < handles.length; i++) {
      if (only && !only.includes(i + 1)) continue;
      await handles[i].screenshot({ path: path.join(outdir, `${String(i + 1).padStart(2, '0')}-${names[i]}.png`) });
      n++;
    }
    console.log('PNG written:', n, '->', outdir);
  }
  if (pdfOut) {
    fs.mkdirSync(path.dirname(pdfOut), { recursive: true });
    await page.emulateMediaType('print');
    await page.pdf({ path: pdfOut, width: '1440px', height: '810px', printBackground: true, margin: { top: 0, right: 0, bottom: 0, left: 0 } });
    console.log('PDF written:', pdfOut);
  }
  await browser.close();
  process.exit(fail ? 1 : 0);
})().catch(e => { console.error('RENDER ERROR:', e && e.message ? e.message : e); process.exit(1); });
