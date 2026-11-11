use std::{
    fs,
    path::PathBuf,
    time::Instant,
};

use anyhow::{Context, Result, anyhow};
use chromiumoxide::{
    browser::{Browser, BrowserConfig},
    cdp::browser_protocol::{
        emulation::SetDeviceMetricsOverrideParams,
        page::{CaptureScreenshotFormat, CaptureScreenshotParams, PrintToPdfParams, Viewport},
    },
};
use clap::Parser;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

/// Antigravity High-Speed Rust Slide Renderer & 6-Gate Layout Inspector
#[derive(Parser, Debug)]
#[command(
    name = "slide_render",
    about = "Antigravity Native Rust Slide Renderer & 6-Gate Layout Inspector (chromiumoxide + tokio)"
)]
pub struct Cli {
    /// Path to built deck HTML (e.g. decks/<slug>/out/<slug>.html)
    #[arg(value_name = "DECK_HTML")]
    pub html: PathBuf,

    /// Directory to output per-slide 2560x1440 PNGs and gate-result.json
    #[arg(value_name = "PNG_OUTDIR")]
    pub outdir: PathBuf,

    /// Optional path to export integrated vector PDF (e.g. --pdf decks/<slug>/out/<slug>.pdf)
    #[arg(long, value_name = "OUT_PDF")]
    pub pdf: Option<PathBuf>,

    /// Only render/inspect specific comma-separated slide indices (e.g. --only 3,4)
    #[arg(long, value_delimiter = ',', value_name = "INDICES")]
    pub only: Option<Vec<usize>>,

    /// Skip PNG generation and only run layout gate check
    #[arg(long)]
    pub nopng: bool,

    /// Optional explicit path to Chrome / Chromium / chrome-headless-shell binary
    #[arg(long, env = "CHROME_PATH")]
    pub chrome_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub fonts: Vec<String>,
    pub slides: Vec<SlideGate>,
    #[serde(default)]
    pub external: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideGate {
    pub n: usize,
    pub id: String,
    pub name: String,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlideRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn detect_chrome_binary(custom: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(p) = custom {
        if p.exists() {
            return Some(p.clone());
        }
    }
    if let Ok(env_path) = std::env::var("CHROME_PATH") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }

    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Users/SD/.cache/puppeteer/chrome-headless-shell/mac_arm-154.0.8037.57/chrome-headless-shell-mac-arm64/chrome-headless-shell",
        "/Users/SD/.cache/puppeteer/chrome/mac_arm-154.0.8037.57/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];

    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

const GATE_JS: &str = r#"
(() => {
  const res = { fonts: [], slides: [], external: [] };
  document.fonts.forEach(f => res.fonts.push(`${f.family} ${f.weight} ${f.status}`));
  
  // Detect external resources
  const resources = performance.getEntriesByType('resource');
  const currentHref = window.location.href;
  resources.forEach(r => {
    const u = r.name;
    if (!u.startsWith('data:') && !u.startsWith('blob:') && u !== currentHref) {
      res.external.push(decodeURI(u));
    }
  });

  document.querySelectorAll('img[src], script[src], link[rel="stylesheet"]').forEach(el => {
    const src = el.src || el.href;
    if (src && !src.startsWith('data:') && !src.startsWith('file:') && !src.startsWith('blob:')) {
      if (!res.external.includes(src)) res.external.push(decodeURI(src));
    }
  });

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
      for (let k = 1; k < kids.length; k++) { const g = kids[k].top - kids[k - 1].bottom; if (g > 44) issues.push('BIG_GAP ' + Math.round(g) + 'px between body blocks ' + k); }
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
})()
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if !cli.html.exists() {
        return Err(anyhow!("Deck HTML file not found: {}", cli.html.display()));
    }

    let abs_html = fs::canonicalize(&cli.html)?;
    let file_url = url::Url::from_file_path(&abs_html)
        .map_err(|_| anyhow!("Failed to convert path to file URL: {}", abs_html.display()))?;

    let chrome_bin = detect_chrome_binary(cli.chrome_path.as_ref()).ok_or_else(|| {
        anyhow!("Local Chrome/Chromium executable not found. Install Chrome or set CHROME_PATH.")
    })?;

    let unique_profile = std::env::temp_dir().join(format!(
        "antigravity_render_{}_{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));

    let config = BrowserConfig::builder()
        .chrome_executable(chrome_bin)
        .user_data_dir(&unique_profile)
        .no_sandbox()
        .arg("--headless=new")
        .arg("--font-render-hinting=none")
        .arg("--allow-file-access-from-files")
        .arg("--disable-gpu")
        .arg("--disable-dev-shm-usage")
        .arg("--hide-scrollbars")
        .window_size(1440, 810)
        .build()
        .map_err(|e| anyhow!("Failed to build BrowserConfig: {}", e))?;

    let (mut browser, mut handler) = Browser::launch(config).await?;

    let bg_handle = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    let page = browser.new_page(file_url.as_str()).await?;
    let _ = page.wait_for_navigation().await;

    // Set 1440x810 viewport with 2560/1440 deviceScaleFactor for 2560x1440 QHD screenshot rendering
    let device_metrics = SetDeviceMetricsOverrideParams::builder()
        .width(1440)
        .height(810)
        .device_scale_factor(2560.0 / 1440.0)
        .mobile(false)
        .build()
        .map_err(|e| anyhow!("Failed to build SetDeviceMetricsOverrideParams: {}", e))?;

    page.execute(device_metrics).await?;

    // Force load all declared fonts and wait for fonts ready
    let font_load_script = r#"
    (async () => {
      const ps = [];
      document.fonts.forEach(f => ps.push(f.load().catch(() => null)));
      await Promise.all(ps);
      await document.fonts.ready;
      return true;
    })()
    "#;
    let _ = page.evaluate(font_load_script).await?;

    // Run layout gate check
    let eval_res = page.evaluate(GATE_JS).await?;
    let gate: GateResult = eval_res
        .into_value()
        .context("Failed to deserialize gate result")?;

    let bad_fonts: Vec<&String> = gate
        .fonts
        .iter()
        .filter(|f| !f.ends_with("loaded"))
        .collect();

    let mut fail = !bad_fonts.is_empty() || !gate.external.is_empty() || gate.slides.is_empty();

    println!("SLIDES: {}", gate.slides.len());
    println!(
        "FONTS: {} faces; {}",
        gate.fonts.len(),
        if bad_fonts.is_empty() {
            "all loaded".to_string()
        } else {
            format!("NOT LOADED: {}", bad_fonts.into_iter().cloned().collect::<Vec<_>>().join(", "))
        }
    );

    println!(
        "EXTERNAL REQUESTS: {}",
        if gate.external.is_empty() {
            "none".to_string()
        } else {
            format!(
                "{} (render the BUILT html, not deck-src.html)",
                gate.external.join(", ")
            )
        }
    );

    for s in &gate.slides {
        if let Some(ref only_list) = cli.only {
            if !only_list.contains(&s.n) {
                continue;
            }
        }
        let tag = format!("S{:02} {}", s.n, s.name);
        if !s.issues.is_empty() {
            fail = true;
            println!("{} GATE FAIL ({})", tag, s.issues.len());
            for issue in s.issues.iter().take(12) {
                println!("   {}", issue);
            }
        } else {
            println!("{} GATE PASS", tag);
        }
    }

    println!("GATE: {}", if fail { "FAIL" } else { "PASS" });

    // Save gate-result.json
    fs::create_dir_all(&cli.outdir)?;
    let gate_json_path = cli.outdir.join("gate-result.json");
    fs::write(&gate_json_path, serde_json::to_string_pretty(&gate)?)?;

    // Render 2560x1440 PNG screenshots
    if !cli.nopng {
        let mut rendered_count = 0;
        for s in &gate.slides {
            if let Some(ref only_list) = cli.only {
                if !only_list.contains(&s.n) {
                    continue;
                }
            }

            // Get slide bounding rect relative to document page
            let scroll_script = format!(
                r#"
                (() => {{
                  const slides = document.querySelectorAll('.slide');
                  const idx = {};
                  if (idx >= slides.length) return null;
                  const target = slides[idx];
                  const r = target.getBoundingClientRect();
                  return {{
                    x: r.left + window.scrollX,
                    y: r.top + window.scrollY,
                    width: r.width,
                    height: r.height
                  }};
                }})()
                "#,
                s.n - 1
            );

            let rect_res = page.evaluate(scroll_script.as_str()).await?;
            if let Ok(rect) = rect_res.into_value::<SlideRect>() {
                let clip = Viewport {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                    scale: 1.0,
                };

                let screenshot_params = CaptureScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .clip(clip)
                    .capture_beyond_viewport(true)
                    .build();

                let png_bytes = page.screenshot(screenshot_params).await?;

                let safe_name: String = s
                    .name
                    .chars()
                    .map(|c| {
                        if r#"\/:*?"<>| "#.contains(c) {
                            '-'
                        } else {
                            c
                        }
                    })
                    .collect();

                let file_name = format!("{:02}-{}.png", s.n, safe_name);
                let out_file = cli.outdir.join(file_name);
                fs::write(&out_file, &png_bytes)?;
                rendered_count += 1;
            }
        }
        println!("PNG written: {} -> {}", rendered_count, cli.outdir.display());
    }

    // Render integrated PDF if requested
    if let Some(ref pdf_out) = cli.pdf {
        if let Some(parent) = pdf_out.parent() {
            fs::create_dir_all(parent)?;
        }

        // Emulate print media
        let emulate_print_script = r#"
        (() => {
          const style = document.createElement('style');
          style.textContent = '@media print { .deck { display: block !important; padding: 0 !important; } .slide { page-break-after: always !important; break-after: page !important; } }';
          document.head.appendChild(style);
        })()
        "#;
        let _ = page.evaluate(emulate_print_script).await;

        let pdf_params = PrintToPdfParams::builder()
            .landscape(true)
            .paper_width(15.0) // 1440px / 96dpi = 15.0 in
            .paper_height(8.4375) // 810px / 96dpi = 8.4375 in
            .margin_top(0.0)
            .margin_bottom(0.0)
            .margin_left(0.0)
            .margin_right(0.0)
            .print_background(true)
            .prefer_css_page_size(true)
            .build();

        let pdf_bytes = page.pdf(pdf_params).await?;
        fs::write(pdf_out, &pdf_bytes)?;
        println!("PDF written: {}", pdf_out.display());
    }

    let _ = page.close().await;
    let _ = browser.close().await;
    bg_handle.abort();
    let _ = fs::remove_dir_all(&unique_profile);

    if fail {
        std::process::exit(1);
    }

    Ok(())
}

