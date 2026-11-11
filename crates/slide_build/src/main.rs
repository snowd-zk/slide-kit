use base64::prelude::*;
use clap::Parser;
use regex::{Captures, Regex};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "htmlfont_build",
    about = "deck-src.html -> self-contained HTML (fonts subset to woff2 + base64, images inlined as data URIs) [Rust 엔진]"
)]
struct Args {
    /// deck source HTML 경로
    #[arg(long)]
    src: PathBuf,

    /// 출력 HTML 경로 (생략 시 <src dir>/out/<src stem>.html)
    #[arg(long)]
    out: Option<PathBuf>,

    /// 폰트 디렉터리 경로 (생략 시 자동 탐색: <kit>/fonts)
    #[arg(long)]
    fonts: Option<PathBuf>,
}

fn find_fonts_dir(src_path: &Path, user_fonts: Option<&Path>) -> Option<PathBuf> {
    if let Some(f) = user_fonts {
        if f.is_dir() {
            return Some(f.to_path_buf());
        }
    }

    // src의 상위 디렉터리들을 탐색하여 fonts/ 폴더 탐색
    let mut curr = src_path.parent();
    while let Some(dir) = curr {
        let candidate = dir.join("fonts");
        if candidate.is_dir() {
            return Some(candidate);
        }
        curr = dir.parent();
    }

    // 현재 작업 디렉터리의 fonts/ 폴더
    let cwd_fonts = PathBuf::from("fonts");
    if cwd_fonts.is_dir() {
        return Some(cwd_fonts);
    }

    None
}

fn get_mime_type(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "svg" => Some("image/svg+xml"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn extract_cmap_from_font(font_path: &Path) -> Result<HashSet<u32>, String> {
    let data = fs::read(font_path).map_err(|e| format!("폰트 파일 읽기 실패 {:?}: {}", font_path, e))?;
    let face = ttf_parser::Face::parse(&data, 0)
        .map_err(|e| format!("폰트 파싱 실패 {:?}: {:?}", font_path, e))?;

    let mut codepoints = HashSet::new();
    if let Some(cmap) = face.tables().cmap {
        for subtable in cmap.subtables {
            subtable.codepoints(|cp| {
                codepoints.insert(cp);
            });
        }
    }
    Ok(codepoints)
}

fn run_font_subset(font_path: &Path, text_file_path: &Path) -> Result<Vec<u8>, String> {
    let out_file = tempfile::Builder::new()
        .suffix(".woff2")
        .tempfile()
        .map_err(|e| format!("임시 파일 생성 실패: {}", e))?;
    let out_path = out_file.path().to_path_buf();

    // 1. pyftsubset 시도
    let text_arg = format!("--text-file={}", text_file_path.display());
    let out_arg = format!("--output-file={}", out_path.display());

    let mut cmd = Command::new("pyftsubset");
    cmd.arg(font_path)
        .arg(&text_arg)
        .arg("--flavor=woff2")
        .arg("--layout-features=*")
        .arg(&out_arg);

    let mut res = cmd.output();

    // 2. pyftsubset이 없으면 python3 -m fontTools.subset 시도
    if res.is_err() || !res.as_ref().unwrap().status.success() {
        let mut py_cmd = Command::new("python3");
        py_cmd
            .arg("-m")
            .arg("fontTools.subset")
            .arg(font_path)
            .arg(&text_arg)
            .arg("--flavor=woff2")
            .arg("--layout-features=*")
            .arg(&out_arg);
        res = py_cmd.output();
    }

    match res {
        Ok(out) => {
            if !out.status.success() {
                let err_msg = String::from_utf8_lossy(&out.stderr);
                return Err(format!("폰트 서브셋 실행 오류: {}", err_msg.trim()));
            }
            let bytes = fs::read(&out_path)
                .map_err(|e| format!("서브셋 출력 파일 읽기 오류: {}", e))?;
            Ok(bytes)
        }
        Err(e) => Err(format!("폰트 서브셋 도구 실행 불가: {}", e)),
    }
}

fn main() {
    let args = Args::parse();

    let src = match args.src.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Error: source file not found: {:?}", args.src);
            std::process::exit(1);
        }
    };
    let src_dir = src.parent().unwrap_or(Path::new("."));

    let out_path = match args.out {
        Some(p) => p,
        None => {
            let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("deck");
            src_dir.join("out").join(format!("{}.html", stem))
        }
    };

    if let Some(parent) = out_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let fonts_dir = find_fonts_dir(&src, args.fonts.as_deref());

    let s = match fs::read_to_string(&src) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error: failed to read source HTML: {}", e);
            std::process::exit(1);
        }
    };

    let mut errors: Vec<String> = Vec::new();

    // ---- visible text (for glyph coverage check) ----
    let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_tags = Regex::new(r"(?is)<[^>]+>").unwrap();

    let no_style = re_style.replace_all(&s, "");
    let no_script = re_script.replace_all(&no_style, "");
    let text_only = re_tags.replace_all(&no_script, "");
    let visible_decoded = html_escape::decode_html_entities(&text_only);
    let vis_chars: HashSet<char> = visible_decoded.chars().filter(|c| !c.is_whitespace()).collect();

    // subset text = every char in the source + printable ASCII (0x20..0x7e)
    let mut subset_chars: HashSet<char> = html_escape::decode_html_entities(&s)
        .chars()
        .filter(|&c| (c as u32) >= 0x20)
        .collect();
    for c in 0x20u8..=0x7Eu8 {
        subset_chars.insert(c as char);
    }

    let mut sorted_chars: Vec<char> = subset_chars.into_iter().collect();
    sorted_chars.sort();
    let chars_str: String = sorted_chars.iter().collect();

    let mut tf = tempfile::Builder::new()
        .suffix(".txt")
        .tempfile()
        .expect("임시 텍스트 파일 생성 실패");
    tf.write_all(chars_str.as_bytes()).expect("임시 파일 쓰기 실패");
    tf.flush().expect("임시 파일 플러시 실패");
    let tf_path = tf.path().to_path_buf();

    let mut family_cmaps: BTreeMap<String, HashSet<u32>> = BTreeMap::new();
    let mut total_font = 0usize;
    let mut faces = 0usize;

    // 1단계: 모든 @font-face 블록에서 고유 폰트 파일 목록 수집
    let re_font_face = Regex::new(r"(?s)@font-face\s*\{[^}]*\}").unwrap();
    let re_fam = Regex::new(r#"font-family:\s*['"]?([^;'"]+)"#).unwrap();
    let re_url = Regex::new(r#"url\(['"]?([^)'"]+\.(?:ttf|otf|woff2?))['"]?\)\s*format\(['"][a-z0-9]+['"]\)"#).unwrap();

    let mut font_tasks: Vec<(String, PathBuf, String)> = Vec::new(); // (fam, font_path, orig_url)
    let mut unique_font_paths: HashSet<PathBuf> = HashSet::new();

    for f_match in re_font_face.find_iter(&s) {
        let block = f_match.as_str();
        let fam = if let Some(fcaps) = re_fam.captures(block) {
            fcaps[1].trim().to_string()
        } else {
            "?".to_string()
        };

        for u_match in re_url.captures_iter(block) {
            let orig_url = u_match[0].to_string();
            let font_rel = &u_match[1];
            let font_name = Path::new(font_rel).file_name().and_then(|n| n.to_str()).unwrap_or(font_rel);

            let mut font_file_path: Option<PathBuf> = None;
            if let Some(ref fdir) = fonts_dir {
                let candidate = fdir.join(font_name);
                if candidate.exists() {
                    font_file_path = Some(candidate);
                }
            }

            if font_file_path.is_none() {
                let alt = src_dir.join(font_rel);
                if alt.exists() {
                    font_file_path = Some(alt);
                }
            }

            if let Some(p) = font_file_path {
                unique_font_paths.insert(p.clone());
                font_tasks.push((fam.clone(), p, orig_url));
            } else {
                let fdir_display = fonts_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| "none".to_string());
                errors.push(format!("font file not found: {} (looked in {})", font_name, fdir_display));
            }
        }
    }

    // 2단계: 고유 폰트 파일 병렬 서브셋팅 (멀티스레드 가속)
    let mut thread_handles = Vec::new();
    for p in unique_font_paths {
        let tf_clone = tf_path.clone();
        let handle = std::thread::spawn(move || {
            let cmap = extract_cmap_from_font(&p);
            let subset_res = run_font_subset(&p, &tf_clone);
            (p, cmap, subset_res)
        });
        thread_handles.push(handle);
    }

    let mut subset_cache: HashMap<PathBuf, Vec<u8>> = HashMap::new();
    for h in thread_handles {
        if let Ok((p, cmap_res, subset_res)) = h.join() {
            if let Ok(cmap) = cmap_res {
                for (fam, fp, _) in &font_tasks {
                    if *fp == p {
                        family_cmaps.entry(fam.clone()).or_default().extend(cmap.clone());
                    }
                }
            }
            match subset_res {
                Ok(data) => {
                    subset_cache.insert(p, data);
                }
                Err(e) => {
                    errors.push(format!("font subsetting failed for {:?}: {}", p, e));
                }
            }
        }
    }

    // 3단계: HTML 내 @font-face 인라인 치환
    let s_fonts_inlined = re_font_face.replace_all(&s, |caps: &Captures| {
        let block = &caps[0];
        re_url.replace_all(block, |ucaps: &Captures| {
            let orig_url = &ucaps[0];
            let font_rel = &ucaps[1];
            let font_name = Path::new(font_rel).file_name().and_then(|n| n.to_str()).unwrap_or(font_rel);

            let mut resolved: Option<PathBuf> = None;
            if let Some(ref fdir) = fonts_dir {
                let candidate = fdir.join(font_name);
                if candidate.exists() {
                    resolved = Some(candidate);
                }
            }
            if resolved.is_none() {
                let alt = src_dir.join(font_rel);
                if alt.exists() {
                    resolved = Some(alt);
                }
            }

            if let Some(p) = resolved {
                if let Some(data) = subset_cache.get(&p) {
                    total_font += data.len();
                    faces += 1;
                    let b64 = BASE64_STANDARD.encode(data);
                    return format!("url(data:font/woff2;base64,{}) format('woff2')", b64);
                }
            }
            orig_url.to_string()
        }).to_string()
    }).to_string();

    drop(tf);

    // ---- glyph coverage ----
    let mut report: Vec<(String, Vec<char>)> = Vec::new();
    let mut body_fail: Vec<char> = Vec::new();

    let body_fams: Vec<String> = {
        let ends_with_p: Vec<String> = family_cmaps.keys().filter(|f| f.ends_with('P')).cloned().collect();
        if !ends_with_p.is_empty() {
            ends_with_p
        } else {
            family_cmaps.keys().take(1).cloned().collect()
        }
    };

    for (fam, cmap) in &family_cmaps {
        let mut miss: Vec<char> = vis_chars
            .iter()
            .filter(|&c| !cmap.contains(&(*c as u32)))
            .cloned()
            .collect();
        miss.sort();
        if body_fams.contains(fam) && !miss.is_empty() {
            body_fail = miss.clone();
        }
        report.push((fam.clone(), miss));
    }

    // ---- inline local images (src="..." and CSS url(...)) ----
    let mut image_cache: HashMap<String, String> = HashMap::new();

    let mut make_data_uri = |rel: &str| -> String {
        if let Some(cached) = image_cache.get(rel) {
            return cached.clone();
        }
        let p = src_dir.join(rel);
        let ext = Path::new(rel).extension().and_then(|e| e.to_str()).unwrap_or("");
        let mime = get_mime_type(ext);

        if !p.exists() || mime.is_none() {
            errors.push(format!("image not found or unsupported: {}", rel));
            image_cache.insert(rel.to_string(), rel.to_string());
            return rel.to_string();
        }

        match fs::read(&p) {
            Ok(bytes) => {
                let b64 = BASE64_STANDARD.encode(&bytes);
                let uri = format!("data:{};base64,{}", mime.unwrap(), b64);
                image_cache.insert(rel.to_string(), uri.clone());
                uri
            }
            Err(e) => {
                errors.push(format!("failed to read image {}: {}", rel, e));
                image_cache.insert(rel.to_string(), rel.to_string());
                rel.to_string()
            }
        }
    };

    let re_src = Regex::new(r#"src="([^"'\)]+\.(?:jpe?g|png|svg|webp|gif))""#).unwrap();
    let s_src_inlined = re_src.replace_all(&s_fonts_inlined, |caps: &Captures| {
        let rel = &caps[1];
        if rel.starts_with("data:") || rel.starts_with('#') || rel.starts_with("http:") || rel.starts_with("https:") || rel.starts_with("//") {
            return caps[0].to_string();
        }
        format!(r#"src="{}""#, make_data_uri(rel))
    }).to_string();

    let re_css_url = Regex::new(r#"url\(['"]?([^"'\)]+\.(?:jpe?g|png|svg|webp|gif))['"]?\)"#).unwrap();
    let s_final = re_css_url.replace_all(&s_src_inlined, |caps: &Captures| {
        let rel = &caps[1];
        if rel.starts_with("data:") || rel.starts_with('#') || rel.starts_with("http:") || rel.starts_with("https:") || rel.starts_with("//") {
            return caps[0].to_string();
        }
        format!(r#"url({})"#, make_data_uri(rel))
    }).to_string();

    // ---- sanity: no remaining external/relative references ----
    let mut leftovers: Vec<String> = Vec::new();

    let re_tag_ref = Regex::new(r#"(?:src|href)="([^"]*)""#).unwrap();
    for m in re_tag_ref.captures_iter(&s_final) {
        let val = &m[1];
        if !val.starts_with("data:") && !val.starts_with('#') {
            leftovers.push(m[0].to_string());
        }
    }

    let re_url_ref = Regex::new(r#"url\(([^)]*)\)"#).unwrap();
    for m in re_url_ref.captures_iter(&s_final) {
        let val = m[1].trim_matches(|c| c == '\'' || c == '"');
        if !val.starts_with("data:") && !val.starts_with('#') {
            leftovers.push(m[0].to_string());
        }
    }

    let re_import = Regex::new(r#"@import[^;]*;"#).unwrap();
    for m in re_import.find_iter(&s_final) {
        leftovers.push(m.as_str().to_string());
    }

    if let Err(e) = fs::write(&out_path, &s_final) {
        eprintln!("Error writing output HTML {:?}: {}", out_path, e);
        std::process::exit(1);
    }

    let out_size_kb = fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0) as f64 / 1024.0;
    let font_size_kb = total_font as f64 / 1024.0;

    println!("OUT {}", out_path.display());
    println!(" size {:.0} KB, fonts {:.0} KB ({} faces, {} chars)", out_size_kb, font_size_kb, faces, sorted_chars.len());
    for (fam, miss) in &report {
        let tag = if body_fams.contains(fam) {
            "BODY"
        } else {
            "heading (falls back to body font)"
        };
        let miss_str: String = miss.iter().collect();
        println!(" visible chars missing in {} [{}]: {}", fam, tag, if miss_str.is_empty() { "none" } else { &miss_str });
    }

    if leftovers.is_empty() {
        println!(" leftover external refs: none");
    } else {
        println!(" leftover external refs: {:?}", leftovers);
    }

    for err in &errors {
        println!(" ERROR {}", err);
    }

    let ok = body_fail.is_empty() && leftovers.is_empty() && errors.is_empty();
    println!("BUILD: {}", if ok { "OK" } else { "FAIL" });

    std::process::exit(if ok { 0 } else { 1 });
}
