//! Candidate display-scale fields in the five corpus drawings.
//!
//! Read-only census for plan U3: JSheet small floats, ViewFilterSet /
//! SheetView, page-frame and symbol scales, `0x002F` bytes after the dash
//! pattern, `0x002E` width-kind candidates, and `_Data.xml` keys. Nothing
//! here is written back to a drawing.
//!
//! ```powershell
//! cargo run --example probe_dash_scale_field_candidates
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::sheet_record_starts;

const FIXTURES: &[(&str, &str)] = &[
    ("0201", "test-file/DWG-0201GP06-01.pid"),
    ("0202", "test-file/DWG-0202GP06-01.pid"),
    ("D06", "test-file/D06.pid"),
    (
        "工艺管道-1",
        "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    ),
    ("A01", "test-file/export-test/publish-data/A01/A01.pid"),
];

const XML_SIDECARS: &[(&str, &str)] = &[
    (
        "0202",
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Data.xml",
    ),
    (
        "0202",
        "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01_Meta.xml",
    ),
    ("A01", "test-file/export-test/publish-data/A01/A01_Data.xml"),
    ("A01", "test-file/export-test/publish-data/A01/A01_Meta.xml"),
];

const CHAIN_MAGIC: u32 = 0x6C90_F544;
const JSHEET: u16 = 0x0114;
const VIEW_FILTER: u16 = 0x0057;
const VIEW_FILTER_ROOT: u16 = 0x0060;
const SHEET_VIEW: u16 = 0x0076;
const SIMPLE_LINE: u16 = 0x002E;
const SIMPLE_DASH: u16 = 0x002F;
const OVERRIDE: u16 = 0x0030;
const SMARTFRAME: u16 = 0x003D;
const POINT_SYMBOL: u16 = 0x0032;
const SYMBOL: u16 = 0x00CE;
const MATRIX_TAG: [u8; 4] = [0x02, 0x00, 0xA7, 0x50];

/// U1 period-method scale, and the two neighbouring hypotheses.
const U1: f64 = 1.67;
const U1_TOL: f64 = 0.03;
const INCH: f64 = 1.75;
const INCH_TOL: f64 = 0.02;
const WIDTH_RATIO: f64 = 1.80;
const WIDTH_TOL: f64 = 0.08;

struct Rec {
    type_code: u16,
    payload: Vec<u8>,
    storage: String,
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f32_at(data: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(data: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

fn near(v: f64, target: f64, tol: f64) -> bool {
    v.is_finite() && (v - target).abs() <= tol
}

fn num(v: f64) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    let rounded = format!("{v:.8}");
    let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// How `v` sits against U1 / 1.75 / the 0.70 mm line-width ratio.
fn classify(v: f64) -> Vec<String> {
    if !v.is_finite() || v == 0.0 {
        return Vec::new();
    }
    let forms = [
        ("原值", v),
        ("1/原值", 1.0 / v),
        ("×25.4", v * 25.4),
        ("÷25.4", v / 25.4),
    ];
    let mut out = Vec::new();
    for (via, x) in forms {
        if near(x, U1, U1_TOL) {
            out.push(format!(
                "{via}={}: 落在 1.67±0.03（距 5/3={:.4}）",
                num(x),
                (x - 5.0 / 3.0).abs()
            ));
        } else if near(x, INCH, INCH_TOL) {
            out.push(format!("{via}={}: 落在 1.75±0.02", num(x)));
        } else if near(x, WIDTH_RATIO, WIDTH_TOL) {
            out.push(format!("{via}={}: 落在线宽比例 1.80±0.08", num(x)));
        }
    }
    out
}

fn bump(map: &mut BTreeMap<String, usize>, key: String) {
    *map.entry(key).or_default() += 1;
}

fn show_counts(title: &str, counts: &BTreeMap<String, usize>) {
    println!("{title}");
    if counts.is_empty() {
        println!("  （无）");
        return;
    }
    for (key, n) in counts {
        println!("  {n:>5}  {key}");
    }
}

fn load(path: &Path) -> Vec<Rec> {
    let file =
        std::fs::File::open(path).unwrap_or_else(|err| panic!("open {}: {err}", path.display()));
    let mut cfb =
        CompoundFile::open(file).unwrap_or_else(|err| panic!("cfb {}: {err}", path.display()));
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .collect();
    let mut out = Vec::new();
    for stream_path in paths {
        let Ok(mut stream) = cfb.open_stream(&stream_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
            continue;
        }
        let storage = stream_path
            .rsplit_once('/')
            .map_or("/", |(parent, _)| parent)
            .to_string();
        for at in sheet_record_starts(&data) {
            let Some(type_code) = u16_at(&data, at).map(|word| word & 0x3FFF) else {
                continue;
            };
            let Some(len) = u32_at(&data, at + 2).map(|len| len as usize) else {
                continue;
            };
            let Some(payload) = data.get(at + 6..at + 6 + len) else {
                continue;
            };
            out.push(Rec {
                type_code,
                payload: payload.to_vec(),
                storage: storage.clone(),
            });
        }
    }
    out
}

fn small_f32(v: f32) -> bool {
    v.is_finite() && (1e-5..20.0).contains(&v.abs()) && (v.abs() - 1.0).abs() > 1e-5
}

fn jsheet(label: &str, recs: &[Rec]) {
    println!("\n== {label} JSheet 0x0114 ==");
    let sheets: Vec<&Rec> = recs.iter().filter(|rec| rec.type_code == JSHEET).collect();
    println!("条数 {}", sheets.len());
    let mut lengths: BTreeMap<usize, usize> = BTreeMap::new();
    let mut floats: BTreeMap<String, usize> = BTreeMap::new();
    let mut slot_184: BTreeMap<String, usize> = BTreeMap::new();
    let mut hits: BTreeMap<String, usize> = BTreeMap::new();
    for rec in sheets {
        *lengths.entry(rec.payload.len()).or_default() += 1;
        if rec.payload.len() >= 192 {
            let a = f32_at(&rec.payload, 184).unwrap_or(f32::NAN);
            let b = f32_at(&rec.payload, 188).unwrap_or(f32::NAN);
            bump(
                &mut slot_184,
                format!("+184={} +188={}", num(f64::from(a)), num(f64::from(b))),
            );
            if (f64::from(a) - 0.5).abs() < 1e-6 {
                let oid = u32_at(&rec.payload, 0).unwrap_or(0);
                println!(
                    "  单独的 0.5 记录: storage {} oid {oid} +184={} +188={}",
                    rec.storage,
                    num(f64::from(a)),
                    num(f64::from(b))
                );
                for (via, x) in [
                    ("原值", f64::from(a)),
                    ("1/原值", 1.0 / f64::from(a)),
                    ("×25.4", f64::from(a) * 25.4),
                    ("÷25.4", f64::from(a) / 25.4),
                ] {
                    println!("    {via} = {}", num(x));
                }
                for (via, x) in [
                    ("原值", f64::from(b)),
                    ("1/原值", 1.0 / f64::from(b)),
                    ("×25.4", f64::from(b) * 25.4),
                    ("÷25.4", f64::from(b) / 25.4),
                ] {
                    println!("    +188 {via} = {}", num(x));
                }
            }
        } else {
            bump(
                &mut slot_184,
                format!("payload {} 字节，无 +184", rec.payload.len()),
            );
        }
        let mut at = 0;
        while at + 4 <= rec.payload.len() {
            if let Some(v) = f32_at(&rec.payload, at).filter(|v| small_f32(*v)) {
                let key = format!("f32 +{at} = {}", num(f64::from(v)));
                bump(&mut floats, key.clone());
                for hit in classify(f64::from(v)) {
                    bump(&mut hits, format!("{key} → {hit}"));
                }
            }
            at += 4;
        }
    }
    show_counts(
        "长度",
        &lengths
            .into_iter()
            .map(|(len, n)| (format!("{len} 字节"), n))
            .collect(),
    );
    show_counts("+184/+188（够长才有）", &slot_184);
    show_counts("对齐 f32，|v|∈(1e-5,20) 且不是 1", &floats);
    show_counts("这些 f32 对 U1 的换算", &hits);
}

fn view_records(label: &str, recs: &[Rec]) {
    println!("\n== {label} 视图记录 ==");
    let mut vfs_widths: BTreeMap<String, usize> = BTreeMap::new();
    let mut vfs_active: BTreeMap<String, usize> = BTreeMap::new();
    let mut vfs_hits: BTreeMap<String, usize> = BTreeMap::new();
    let mut n_vfs = 0usize;
    let mut n_vfs_closed = 0usize;
    for rec in recs.iter().filter(|rec| rec.type_code == VIEW_FILTER) {
        n_vfs += 1;
        let Some(active) = u32_at(&rec.payload, 32) else {
            continue;
        };
        bump(&mut vfs_active, format!("+32={active}"));
        // Walk the published layout far enough to collect override widths,
        // and separately test every aligned f64 against the scale bands.
        if walk_overrides(&rec.payload, &mut vfs_widths) {
            n_vfs_closed += 1;
        }
        let mut at = 0;
        while at + 8 <= rec.payload.len() {
            if let Some(v) = f64_at(&rec.payload, at) {
                for hit in classify(v) {
                    bump(&mut vfs_hits, format!("f64 +{at} = {} → {hit}", num(v)));
                }
            }
            at += 4;
        }
    }
    println!("0x0057 {n_vfs} 条，按显示态布局收得下覆盖表的 {n_vfs_closed} 条");
    show_counts("0x0057 +32", &vfs_active);
    show_counts("0x0057 覆盖里的线宽（mm）", &vfs_widths);
    show_counts("0x0057 里对上 U1 的 f64", &vfs_hits);

    let roots: Vec<&Rec> = recs
        .iter()
        .filter(|rec| rec.type_code == VIEW_FILTER_ROOT)
        .collect();
    println!("0x0060 {} 条", roots.len());
    let mut root_lens: BTreeMap<String, usize> = BTreeMap::new();
    let mut root_words: BTreeMap<String, usize> = BTreeMap::new();
    for rec in roots {
        bump(&mut root_lens, format!("{} 字节", rec.payload.len()));
        let words: Vec<String> = (0..rec.payload.len().saturating_sub(3))
            .step_by(4)
            .filter_map(|at| u32_at(&rec.payload, at).map(|word| format!("+{at}={word}")))
            .collect();
        bump(&mut root_words, words.join(" "));
    }
    show_counts("0x0060 长度", &root_lens);
    show_counts("0x0060 对齐 u32", &root_words);

    println!("-- SheetView 0x0076 --");
    for rec in recs.iter().filter(|rec| rec.type_code == SHEET_VIEW) {
        println!("  storage {} len {}", rec.storage, rec.payload.len());
        let mut at = 0;
        while at + 4 <= rec.payload.len() {
            let word = u32_at(&rec.payload, at).unwrap_or(0);
            let f32v = f32_at(&rec.payload, at).unwrap_or(f32::NAN);
            let extra = if at + 8 <= rec.payload.len() {
                f64_at(&rec.payload, at)
                    .map(|v| format!(" f64={}", num(v)))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            if word != 0 || small_f32(f32v) {
                println!(
                    "    +{at:<3} u32={word} f32={}{extra}",
                    num(f64::from(f32v))
                );
            }
            at += 4;
        }
    }
}

/// Override widths inside a `0x0057` payload. `true` when the override table
/// itself parses; the layer-name tail is not required.
fn walk_overrides(payload: &[u8], widths: &mut BTreeMap<String, usize>) -> bool {
    let mut at = 38usize;
    for _ in 0..6 {
        if payload.get(at) != Some(&0xFF) {
            return false;
        }
        let Some(len) = u16_at(payload, at + 1) else {
            return false;
        };
        at = at.saturating_add(3).saturating_add(usize::from(len));
    }
    let Some(n) = u16_at(payload, at) else {
        return false;
    };
    if u16_at(payload, at + 2) != Some(2) {
        return false;
    }
    at += 4;
    for _ in 0..n {
        let Some(kind) = payload.get(at + 2).copied() else {
            return false;
        };
        at += 6;
        if kind & 2 != 0 {
            let Some(width) = f64_at(payload, at + 4) else {
                return false;
            };
            bump(widths, format!("{} mm", num(width * 1000.0)));
            at += 12;
        }
        at += 4;
    }
    true
}

fn dashes(label: &str, recs: &[Rec]) {
    println!("\n== {label} 0x002F 虚线 ==");
    let dashes: Vec<&Rec> = recs
        .iter()
        .filter(|rec| rec.type_code == SIMPLE_DASH)
        .collect();
    println!("条数 {}", dashes.len());
    let mut fit = 0usize;
    let mut remainders: BTreeMap<String, usize> = BTreeMap::new();
    let mut patterns: BTreeMap<String, usize> = BTreeMap::new();
    let mut head_nonzero: BTreeMap<String, usize> = BTreeMap::new();
    let mut head_hits: BTreeMap<String, usize> = BTreeMap::new();
    for rec in &dashes {
        let n = u16_at(&rec.payload, 48).unwrap_or(u16::MAX);
        let expect = 50usize.saturating_add(8 * usize::from(n));
        let style_id = u32_at(&rec.payload, 14).unwrap_or(0);
        if expect == rec.payload.len() && (n as usize) < 32 {
            fit += 1;
            let pattern: Vec<String> = (0..n as usize)
                .filter_map(|i| f64_at(&rec.payload, 50 + 8 * i).map(num))
                .collect();
            bump(
                &mut patterns,
                format!("id {style_id} N={n} [{}]", pattern.join(", ")),
            );
        } else {
            let tail = if expect < rec.payload.len() && (n as usize) < 32 {
                let extra = &rec.payload[expect..];
                format!("余 {} 字节 {}", extra.len(), hex(extra))
            } else {
                format!("N={n} 期望 {expect} 实际 {}", rec.payload.len())
            };
            bump(&mut remainders, format!("id {style_id} {tail}"));
        }
        let head = &rec.payload[26.min(rec.payload.len())..48.min(rec.payload.len())];
        let nonzero = head.iter().filter(|byte| **byte != 0).count();
        bump(&mut head_nonzero, format!("+26..+47 非零字节 {nonzero}"));
        let mut at = 0;
        while at + 8 <= 48.min(rec.payload.len()) {
            if let Some(v) = f64_at(&rec.payload, at) {
                for hit in classify(v) {
                    bump(
                        &mut head_hits,
                        format!("id {style_id} f64 +{at}={} → {hit}", num(v)),
                    );
                }
            }
            at += 4;
        }
    }
    println!("恰好 50+8N 的 {fit}/{}", dashes.len());
    show_counts("对不上 50+8N 的", &remainders);
    show_counts("图案", &patterns);
    show_counts("头部 +26..+47", &head_nonzero);
    show_counts("头部 f64 对上 U1 的", &head_hits);

    println!("-- 每条 0x002F 头部 u32（+26 起）与图案 --");
    for rec in dashes {
        let n = u16_at(&rec.payload, 48).unwrap_or(0);
        let style_id = u32_at(&rec.payload, 14).unwrap_or(0);
        let words: Vec<String> = (26..48)
            .step_by(4)
            .filter_map(|at| u32_at(&rec.payload, at).map(|word| format!("+{at}={word}")))
            .collect();
        let pattern: Vec<String> = (0..n as usize)
            .filter_map(|i| f64_at(&rec.payload, 50 + 8 * i).map(num))
            .collect();
        let head_scale = f64_at(&rec.payload, 34)
            .map(num)
            .unwrap_or_else(|| "无".into());
        println!(
            "  {} id {style_id} len {} N={n} [{}] f64+34={head_scale} {}",
            rec.storage,
            rec.payload.len(),
            pattern.join(", "),
            words.join(" ")
        );
    }
}

fn lines(label: &str, recs: &[Rec]) {
    println!("\n== {label} 0x002E 线样式 ==");
    let lines: Vec<&Rec> = recs
        .iter()
        .filter(|rec| rec.type_code == SIMPLE_LINE)
        .collect();
    println!("条数 {}", lines.len());
    let mut lengths: BTreeMap<String, usize> = BTreeMap::new();
    let mut columns: BTreeMap<usize, BTreeMap<String, usize>> = BTreeMap::new();
    let mut unit_like: BTreeMap<String, usize> = BTreeMap::new();
    let offsets = [0usize, 4, 8, 12, 16, 20, 24, 26, 30, 42, 46, 50, 54];
    for rec in &lines {
        bump(&mut lengths, format!("{} 字节", rec.payload.len()));
        for at in offsets {
            if let Some(word) = u32_at(&rec.payload, at) {
                bump(columns.entry(at).or_default(), word.to_string());
            }
        }
        let mut at = 0;
        while at + 2 <= rec.payload.len() {
            if let Some(word) = u16_at(&rec.payload, at) {
                if matches!(word, 11..=13) {
                    bump(&mut unit_like, format!("u16 +{at} = {word}"));
                }
            }
            at += 2;
        }
    }
    show_counts("长度", &lengths);
    for at in offsets {
        let Some(values) = columns.get(&at) else {
            continue;
        };
        if values.len() <= 8 {
            let text = values
                .iter()
                .map(|(value, n)| format!("{value}×{n}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!("  u32 +{at:<3} {text}");
        } else {
            println!("  u32 +{at:<3} {} 种取值", values.len());
        }
    }
    show_counts("u16 等于 11/12/13（styleUnits 候选）", &unit_like);

    println!("-- 指向虚线的 0x002E（+54 = dash id）--");
    let dash_ids: Vec<u32> = recs
        .iter()
        .filter(|rec| rec.type_code == SIMPLE_DASH)
        .filter_map(|rec| u32_at(&rec.payload, 14))
        .collect();
    for rec in &lines {
        let Some(dash_id) = u32_at(&rec.payload, 54) else {
            continue;
        };
        if dash_id == 0 || !dash_ids.contains(&dash_id) {
            continue;
        }
        let style_id = u32_at(&rec.payload, 14).unwrap_or(0);
        let mode = u32_at(&rec.payload, 46).unwrap_or(0);
        let colour = u32_at(&rec.payload, 50).unwrap_or(0);
        let width = f64_at(&rec.payload, 34)
            .map(|v| num(v * 1000.0))
            .unwrap_or_else(|| "无".into());
        let base: Vec<String> = (0..30)
            .step_by(2)
            .filter_map(|at| u16_at(&rec.payload, at).map(|word| format!("+{at}={word}")))
            .collect();
        println!(
            "  {} line id {style_id} → dash {dash_id} 宽 {width} mm +46={mode} +50={colour:#X} 基类u16 [{}]",
            rec.storage,
            base.join(" ")
        );
    }
}

fn overrides(label: &str, recs: &[Rec]) {
    println!("\n== {label} 0x0030 覆盖的比例位 ==");
    let rows: Vec<&Rec> = recs
        .iter()
        .filter(|rec| rec.type_code == OVERRIDE)
        .collect();
    println!("条数 {}", rows.len());
    let mut masks: BTreeMap<String, usize> = BTreeMap::new();
    let mut scales: BTreeMap<String, usize> = BTreeMap::new();
    let mut bit2 = 0usize;
    for rec in rows {
        let mask = u32_at(&rec.payload, 30).unwrap_or(0);
        bump(&mut masks, format!("{mask:#X}"));
        if mask & 4 != 0 {
            bit2 += 1;
        }
        let a = f64_at(&rec.payload, 50)
            .map(num)
            .unwrap_or_else(|| "无".into());
        let b = f64_at(&rec.payload, 58)
            .map(num)
            .unwrap_or_else(|| "无".into());
        let c = f64_at(&rec.payload, 66)
            .map(num)
            .unwrap_or_else(|| "无".into());
        bump(&mut scales, format!("+50={a} +58={b} +66={c}"));
    }
    println!("掩码 bit2（比例）置位 {bit2}");
    show_counts("掩码", &masks);
    show_counts("+50/+58/+66", &scales);
}

fn frames_and_symbols(label: &str, recs: &[Rec]) {
    println!("\n== {label} 页框 0x003D / 符号比例 ==");
    let mut pages: BTreeMap<String, usize> = BTreeMap::new();
    let mut other: BTreeMap<String, usize> = BTreeMap::new();
    for rec in recs.iter().filter(|rec| rec.type_code == SMARTFRAME) {
        let w = f64_at(&rec.payload, 76);
        let h = f64_at(&rec.payload, 84);
        let aspect = f64_at(&rec.payload, 148);
        bump(
            &mut pages,
            format!(
                "{}×{} mm aspect {}",
                w.map(|v| num(v * 1000.0)).unwrap_or_else(|| "无".into()),
                h.map(|v| num(v * 1000.0)).unwrap_or_else(|| "无".into()),
                aspect.map(num).unwrap_or_else(|| "无".into())
            ),
        );
        let mut at = 0;
        while at + 8 <= rec.payload.len() {
            if let Some(v) = f64_at(&rec.payload, at) {
                let known = w.is_some_and(|page| (v - page).abs() < 1e-9)
                    || h.is_some_and(|page| (v - page).abs() < 1e-9)
                    || aspect.is_some_and(|page| (v - page).abs() < 1e-9)
                    || near(v, 0.0, 1e-12)
                    || near(v, 1.0, 1e-9)
                    || near(v, std::f64::consts::FRAC_1_SQRT_2, 1e-4);
                if !known && v.is_finite() && (1e-4..100.0).contains(&v.abs()) {
                    let hits = classify(v);
                    let mark = if hits.is_empty() {
                        "不命中".to_string()
                    } else {
                        hits.join("；")
                    };
                    bump(&mut other, format!("f64 +{at}={} {mark}", num(v)));
                }
            }
            at += 4;
        }
    }
    show_counts("页幅", &pages);
    show_counts("页框里除 0/1/页幅/1/√2 之外的 f64", &other);

    let mut scales: BTreeMap<String, usize> = BTreeMap::new();
    let mut scale_hits: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen = 0usize;
    for rec in recs.iter().filter(|rec| rec.type_code == SYMBOL) {
        let Some(tag_at) = rec
            .payload
            .windows(4)
            .position(|window| window == MATRIX_TAG)
        else {
            continue;
        };
        let scale_at = tag_at + 4 + 48;
        let Some(scale) = f64_at(&rec.payload, scale_at) else {
            continue;
        };
        seen += 1;
        bump(&mut scales, num(scale));
        for hit in classify(scale) {
            bump(&mut scale_hits, format!("{} → {hit}", num(scale)));
        }
    }
    println!("igSymbol2d 读到均匀比例的 {seen} 条");
    show_counts("符号均匀比例", &scales);
    show_counts("符号比例对 U1", &scale_hits);
}

fn point_symbols(label: &str, recs: &[Rec]) {
    println!("\n== {label} 0x0032 点标记里的小 f64 ==");
    let mut values: BTreeMap<String, usize> = BTreeMap::new();
    for rec in recs.iter().filter(|rec| rec.type_code == POINT_SYMBOL) {
        let mut at = 0;
        while at + 8 <= rec.payload.len() {
            if let Some(v) = f64_at(&rec.payload, at) {
                if v.is_finite() && (1e-6..5.0).contains(&v.abs()) && !near(v, 1.0, 1e-9) {
                    let hits = classify(v);
                    let mark = if hits.is_empty() {
                        String::new()
                    } else {
                        format!(" → {}", hits.join("；"))
                    };
                    bump(
                        &mut values,
                        format!("+{at}={} ({} mm){mark}", num(v), num(v * 1000.0)),
                    );
                }
            }
            at += 4;
        }
    }
    show_counts("点标记 f64", &values);
}

fn safe_band(label: &str, recs: &[Rec]) {
    println!("\n== {label} 全记录对齐浮点里落在比例带的 ==");
    let mut hits: BTreeMap<String, usize> = BTreeMap::new();
    for rec in recs {
        let mut at = 0;
        while at + 4 <= rec.payload.len() {
            if let Some(v) = f32_at(&rec.payload, at) {
                for hit in classify(f64::from(v)) {
                    bump(
                        &mut hits,
                        format!(
                            "f32 {:#06X} +{at} = {} → {hit}",
                            rec.type_code,
                            num(f64::from(v))
                        ),
                    );
                }
            }
            if at + 8 <= rec.payload.len() {
                if let Some(v) = f64_at(&rec.payload, at) {
                    // Reciprocal and ÷25.4 land inside ordinary sheet
                    // coordinates, so on geometry they are not a field.
                    let styleish = matches!(
                        rec.type_code,
                        JSHEET
                            | VIEW_FILTER
                            | VIEW_FILTER_ROOT
                            | SHEET_VIEW
                            | SIMPLE_LINE
                            | SIMPLE_DASH
                            | OVERRIDE
                            | SMARTFRAME
                            | POINT_SYMBOL
                    );
                    for hit in classify(v) {
                        let reciprocal = hit.starts_with("1/原值") || hit.starts_with("÷25.4");
                        if reciprocal && !styleish {
                            continue;
                        }
                        bump(
                            &mut hits,
                            format!("f64 {:#06X} +{at} = {} → {hit}", rec.type_code, num(v)),
                        );
                    }
                }
            }
            at += 4;
        }
    }
    show_counts("命中", &hits);
}

fn xml_keys(label: &str, text: &str) {
    println!("\n== {label} XML 键 ==");
    let mut keys: BTreeMap<String, usize> = BTreeMap::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'=' && bytes.get(i + 1) == Some(&b'"') {
            let mut start = i;
            while start > 0
                && (bytes[start - 1].is_ascii_alphanumeric()
                    || bytes[start - 1] == b'_'
                    || bytes[start - 1] == b':')
            {
                start -= 1;
            }
            let name = &text[start..i];
            let lower = name.to_ascii_lowercase();
            let interesting = [
                "scale", "unit", "width", "view", "ratio", "dash", "linetype", "dpi", "pitch",
                "metric", "inch",
            ]
            .iter()
            .any(|needle| lower.contains(needle));
            if interesting {
                let value_end = text[i + 2..]
                    .find('"')
                    .map(|end| i + 2 + end)
                    .unwrap_or(i + 2);
                let value = &text[i + 2..value_end.min(text.len())];
                let shown = if value.len() > 80 {
                    &value[..80]
                } else {
                    value
                };
                bump(&mut keys, format!("{name}=\"{shown}\""));
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    show_counts(
        "名字里带 scale/unit/width/view/ratio/dash/linetype/dpi/pitch/metric/inch 的属性",
        &keys,
    );
}

fn utf16_mentions(label: &str, path: &Path) {
    println!("\n== {label} 复合文件里的单位/比例字样 ==");
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(err) => {
            println!("  打不开: {err}");
            return;
        }
    };
    let mut cfb = match CompoundFile::open(file) {
        Ok(cfb) => cfb,
        Err(err) => {
            println!("  不是 CFB: {err}");
            return;
        }
    };
    let paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .collect();
    let needles = [
        "Scale",
        "Units",
        "Metric",
        "Imperial",
        "Inch",
        "Millimet",
        "Line Width",
        "LineWidth",
        "ltscale",
    ];
    let mut hits: BTreeMap<String, usize> = BTreeMap::new();
    for stream_path in paths {
        let Ok(mut stream) = cfb.open_stream(&stream_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        for needle in needles {
            let encoded: Vec<u8> = needle
                .encode_utf16()
                .flat_map(|unit| unit.to_le_bytes())
                .collect();
            if data.windows(encoded.len()).any(|window| window == encoded) {
                bump(&mut hits, format!("{stream_path} 含 UTF-16「{needle}」"));
            }
        }
    }
    show_counts("流内字样", &hits);
}

/// `style.dll` reads the base-class word at payload `+12` as a unit code:
/// `units = (w & 7) + 10`, which is 11, 12, or 13. The 10-10 override note
/// called this word a dash index; the commander correction is that it is the
/// unit code the view conversion picks `f0` / `f1` / `f2` with.
fn unit_codes(label: &str, recs: &[Rec]) {
    println!("\n== {label} 基类 +12 单位码 ==");
    let mut hist: BTreeMap<String, usize> = BTreeMap::new();
    for rec in recs {
        if !matches!(rec.type_code, 0x002C | SIMPLE_LINE | SIMPLE_DASH | OVERRIDE) {
            continue;
        }
        let family = match rec.type_code {
            0x002C => "002C",
            SIMPLE_LINE => "002E",
            SIMPLE_DASH => "002F",
            OVERRIDE => "0030",
            _ => "?",
        };
        let Some(raw) = u16_at(&rec.payload, 12) else {
            bump(&mut hist, format!("{family} 载荷短于 +12"));
            continue;
        };
        let units = (raw & 7) + 10;
        bump(&mut hist, format!("{family} raw={raw} units={units}"));
    }
    show_counts("raw u16 与 (w & 7)+10", &hist);

    println!("-- 每条 0x002F 的单位码、头部模式低 4 位、+34 f64 --");
    for rec in recs.iter().filter(|rec| rec.type_code == SIMPLE_DASH) {
        let raw = u16_at(&rec.payload, 12).unwrap_or(0);
        let style_id = u32_at(&rec.payload, 14).unwrap_or(0);
        let n = u16_at(&rec.payload, 48).unwrap_or(0);
        let pattern: Vec<String> = (0..n as usize)
            .filter_map(|index| f64_at(&rec.payload, 50 + 8 * index).map(num))
            .collect();
        let modes: Vec<String> = [26usize, 30, 42, 46]
            .into_iter()
            .filter_map(|at| {
                u32_at(&rec.payload, at).map(|word| format!("+{at}={word:#X}/low4={}", word & 0xF))
            })
            .collect();
        let scale = f64_at(&rec.payload, 34)
            .map(num)
            .unwrap_or_else(|| "无".into());
        println!(
            "  {} id {style_id} raw={raw} units={} N={n} [{}] f64+34={scale} {}",
            rec.storage,
            (raw & 7) + 10,
            pattern.join(", "),
            modes.join(" ")
        );
    }

    if label == "0201" {
        println!("-- 0201 JSheet oid 119 与 501 的小 f32 --");
        for rec in recs.iter().filter(|rec| rec.type_code == JSHEET) {
            let oid = u32_at(&rec.payload, 0).unwrap_or(0);
            if oid != 119 && oid != 501 {
                continue;
            }
            println!(
                "  oid {oid} storage {} len {}",
                rec.storage,
                rec.payload.len()
            );
            let mut at = 0;
            while at + 4 <= rec.payload.len() {
                if let Some(value) = f32_at(&rec.payload, at).filter(|value| small_f32(*value)) {
                    println!("    f32 +{at} = {}", num(f64::from(value)));
                }
                at += 4;
            }
        }
        println!("-- 0201 覆盖 oid 9，以及 +78 指向虚线 8 的覆盖 --");
        for rec in recs.iter().filter(|rec| rec.type_code == OVERRIDE) {
            let oid = u32_at(&rec.payload, 0).unwrap_or(0);
            let dash = u32_at(&rec.payload, 78).unwrap_or(0);
            if oid != 9 && dash != 8 {
                continue;
            }
            let raw = u16_at(&rec.payload, 12).unwrap_or(0);
            let mask = u32_at(&rec.payload, 30).unwrap_or(0);
            let mode = u32_at(&rec.payload, 46).unwrap_or(0);
            println!(
                "  override oid {oid} storage {} mask={mask:#X} +78={dash} +12 raw={raw} units={} +46={mode}",
                rec.storage,
                (raw & 7) + 10
            );
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    println!(
        "U1 对照带: 1.67±0.03（含 5/3={:.6} 与 160/96={:.6}）、1.75±0.02、线宽 1.80±0.08",
        5.0 / 3.0,
        160.0 / 96.0
    );
    println!("换算只取 原值 / 1/原值 / ×25.4 / ÷25.4");
    for (label, rel) in FIXTURES {
        let path = Path::new(rel);
        println!("\n######## {label} {rel} ########");
        let recs = load(path);
        println!("记录链记录 {} 条", recs.len());
        unit_codes(label, &recs);
        jsheet(label, &recs);
        view_records(label, &recs);
        dashes(label, &recs);
        lines(label, &recs);
        overrides(label, &recs);
        frames_and_symbols(label, &recs);
        point_symbols(label, &recs);
        safe_band(label, &recs);
        utf16_mentions(label, path);
    }
    println!("\n######## _Data.xml / _Meta.xml ########");
    println!("0201、D06、工艺管道-1：test-file 下没有旁路 _Data.xml / _Meta.xml");
    for (label, rel) in XML_SIDECARS {
        let text = std::fs::read_to_string(rel).unwrap_or_else(|err| panic!("read {rel}: {err}"));
        xml_keys(&format!("{label} {rel}"), &text);
    }
}
