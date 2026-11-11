#!/usr/bin/env python3
"""deck-src.html -> self-contained HTML (fonts subset to woff2 + base64, images inlined as data URIs).

usage:
  python3 scripts/build.py --src decks/my-deck/deck-src.html --out decks/my-deck/out/my-deck.html
  options: --fonts <dir>   (default: <kit>/fonts)

Font files are matched by FILE NAME: any @font-face url('.../Pretendard-Bold.otf') is looked up
as <fonts>/Pretendard-Bold.otf, so the relative path written in the source does not matter.
Images: every src="..." / url(...) that points to a local file (relative to the source file) is inlined.

Exit code 0 = OK, 1 = missing glyphs in the body font, missing files, or leftover external references.
"""
import argparse, base64, html, os, re, subprocess, sys, tempfile
from html.parser import HTMLParser

try:
    from fontTools.ttLib import TTFont
except ImportError:
    sys.exit('fontTools not installed: run  python3 -m pip install fonttools brotli  (see README)')

KIT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUST_BIN = os.path.join(os.path.dirname(os.path.abspath(__file__)), "build")
GLOBAL_RUST_BIN = "/Users/SD/Documents/Antigravity/bin/htmlfont_build"

# Rust 네이티브 빌더 우선 실행
if not os.environ.get("FORCE_PYTHON_BUILD"):
    bin_target = RUST_BIN if os.path.exists(RUST_BIN) else (GLOBAL_RUST_BIN if os.path.exists(GLOBAL_RUST_BIN) else None)
    if bin_target:
        res = subprocess.run([bin_target] + sys.argv[1:])
        sys.exit(res.returncode)

ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
ap.add_argument('--src', required=True, help='deck source HTML')
ap.add_argument('--out', help='output HTML (default: <src dir>/out/<src name>.html)')
ap.add_argument('--fonts', default=os.path.join(KIT, 'fonts'), help='font directory (default: <kit>/fonts)')
a = ap.parse_args()

SRC = os.path.abspath(a.src)
SRC_DIR = os.path.dirname(SRC)
OUT = os.path.abspath(a.out) if a.out else os.path.join(SRC_DIR, 'out', os.path.splitext(os.path.basename(SRC))[0] + '.html')
FONTS = os.path.abspath(a.fonts)
os.makedirs(os.path.dirname(OUT), exist_ok=True)
s = open(SRC, encoding='utf-8').read()
errors = []

# ---- visible text (for glyph coverage check) ----
class T(HTMLParser):
    def __init__(self):
        super().__init__(); self.buf = []; self.skip = 0
    def handle_starttag(self, t, attrs):
        if t in ('style', 'script'): self.skip += 1
    def handle_endtag(self, t):
        if t in ('style', 'script'): self.skip -= 1
    def handle_data(self, d):
        if not self.skip: self.buf.append(d)
tp = T(); tp.feed(s)
visible = html.unescape(''.join(tp.buf))
vis_chars = {c for c in visible if not c.isspace()}

# subset text = every char in the source (superset of visible text, incl. CSS content:'') + printable ASCII
chars = set(html.unescape(s)) | {chr(c) for c in range(0x20, 0x7f)}
chars = {c for c in chars if ord(c) >= 0x20}
tf = tempfile.NamedTemporaryFile('w', suffix='.txt', delete=False, encoding='utf-8')
tf.write(''.join(sorted(chars))); tf.close()

family_cmaps = {}   # family -> set of codepoints
total_font = 0
faces = 0

def subset(path):
    fd, out = tempfile.mkstemp(suffix='.woff2'); os.close(fd)
    subprocess.run([sys.executable, '-m', 'fontTools.subset', path, f'--text-file={tf.name}',
                    '--flavor=woff2', '--layout-features=*', f'--output-file={out}'], check=True)
    data = open(out, 'rb').read(); os.unlink(out)
    return data

def repl_face(m):
    global total_font, faces
    block = m.group(0)
    fam = re.search(r"font-family:\s*['\"]?([^;'\"]+)", block)
    fam = fam.group(1).strip() if fam else '?'
    def repl_url(u):
        global total_font, faces
        name = os.path.basename(u.group(1))
        path = os.path.join(FONTS, name)
        if not os.path.exists(path):
            alt = os.path.normpath(os.path.join(SRC_DIR, u.group(1)))
            path = alt if os.path.exists(alt) else None
        if not path:
            errors.append(f'font file not found: {name} (looked in {FONTS})')
            return u.group(0)
        family_cmaps.setdefault(fam, set()).update(TTFont(path).getBestCmap().keys())
        data = subset(path)
        total_font += len(data); faces += 1
        return "url(data:font/woff2;base64," + base64.b64encode(data).decode() + ") format('woff2')"
    return re.sub(r"url\(['\"]?([^)'\"]+\.(?:ttf|otf|woff2?))['\"]?\)\s*format\(['\"][a-z0-9]+['\"]\)", repl_url, block)

s = re.sub(r'@font-face\s*\{[^}]*\}', repl_face, s)
os.unlink(tf.name)

# ---- glyph coverage: body font (family with most faces / named *P*) must cover all visible chars ----
report = []
body_fail = []
for fam, cmap in family_cmaps.items():
    miss = sorted(c for c in vis_chars if ord(c) not in cmap)
    report.append((fam, miss))
body_fams = [f for f in family_cmaps if f.endswith('P')] or list(family_cmaps)[:1]
for fam, miss in report:
    if fam in body_fams and miss:
        body_fail = miss

# ---- inline local images (src="..." and CSS url(...)) ----
MIME = {'jpg': 'image/jpeg', 'jpeg': 'image/jpeg', 'png': 'image/png', 'svg': 'image/svg+xml',
        'webp': 'image/webp', 'gif': 'image/gif'}
cache = {}
def data_uri(rel):
    if rel in cache: return cache[rel]
    p = os.path.normpath(os.path.join(SRC_DIR, rel))
    ext = rel.rsplit('.', 1)[-1].lower()
    if not os.path.exists(p) or ext not in MIME:
        errors.append(f'image not found or unsupported: {rel}')
        cache[rel] = rel; return rel
    cache[rel] = f'data:{MIME[ext]};base64,' + base64.b64encode(open(p, 'rb').read()).decode()
    return cache[rel]
LOCAL = r'(?!data:|#|https?:|//)([^"\')]+\.(?:jpe?g|png|svg|webp|gif))'
s = re.sub(r'src="' + LOCAL + '"', lambda m: 'src="' + data_uri(m.group(1)) + '"', s)
s = re.sub(r"url\(['\"]?" + LOCAL + r"['\"]?\)", lambda m: 'url(' + data_uri(m.group(1)) + ')', s)

# ---- sanity: no remaining external/relative references ----
leftover = re.findall(r'(?:src|href)="(?!data:|#)[^"]*"|url\((?![\'"]?data:|#)[^)]*\)|@import[^;]*;', s)

open(OUT, 'w', encoding='utf-8').write(s)
print(f'OUT {OUT}')
print(f' size {os.path.getsize(OUT)/1024:.0f} KB, fonts {total_font/1024:.0f} KB ({faces} faces, {len(chars)} chars)')
for fam, miss in report:
    tag = 'BODY' if fam in body_fams else 'heading (falls back to body font)'
    print(f" visible chars missing in {fam} [{tag}]:", ''.join(miss) or 'none')
print(' leftover external refs:', leftover or 'none')
for e in errors: print(' ERROR', e)
ok = not (body_fail or leftover or errors)
print('BUILD:', 'OK' if ok else 'FAIL')
sys.exit(0 if ok else 1)
