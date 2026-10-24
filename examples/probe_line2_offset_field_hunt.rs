//! Dump every field of the ` Line2` placement chain and score it against the
//! 11.53 mm page-y gap.
//!
//! Reads `test-file/DWG-0201GP06-01.pid` only. Columns are ` Line2`, Parametric
//! Manifold, and one static symbol from the 2026-09-07 byte-exact set (Ball
//! Valve Type 2, else Cap, else the other matched gauges). Nothing is written.
//!
//! ```powershell
//! cargo run --example probe_line2_offset_field_hunt
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_flavor_holders, decode_igsymbols, decode_symbol_informations, parse_psm_header,
    sheet_record_starts, PsmFlavorHolderDecoded, PsmSymbolInformationDecoded, PSM_ENVELOPE_LEN,
    PSM_TYPE_CODE_JFLAVOR_HOLDER, PSM_TYPE_CODE_JSHEET, PSM_TYPE_CODE_JSYMBOL_INFORMATION,
};
use pid_parse::PidParser;

const FIXTURE: &str = "test-file/DWG-0201GP06-01.pid";
const MATRIX_TAG: [u8; 4] = [0x02, 0x00, 0xA7, 0x50];
const TOL_MM: f64 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    U32,
    F32,
    F64,
}

struct Target {
    label: &'static str,
    mm: f64,
}

#[derive(Clone)]
struct Hit {
    where_: String,
    kind: Kind,
    raw: f64,
    reading: &'static str,
    mm: f64,
    target: String,
    delta: f64,
}

struct Placed {
    role: &'static str,
    symbol_file: String,
    symbol_path: String,
    stream: String,
    offset: usize,
    payload: Vec<u8>,
    jsite_ref: u32,
    def_sheet: u32,
    def_site: u32,
    insertion: (f64, f64),
    oid: u32,
}

struct RawRec {
    stream: String,
    at: usize,
    type_code: u16,
    flags: u16,
    skipped: bool,
    payload: Vec<u8>,
}

struct SiteRow {
    file: String,
    path: String,
}

fn u16_at(p: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(p.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(p: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(p.get(at..at + 4)?.try_into().ok()?))
}

fn f32_at(p: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_le_bytes(p.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(p: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(p.get(at..at + 8)?.try_into().ok()?))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path).trim()
}

fn targets_for(ins_y_mm: f64, local_y_mm: f64) -> Vec<Target> {
    let mut t = vec![
        Target {
            label: "11.53",
            mm: 11.53,
        },
        Target {
            label: "12.70",
            mm: 12.70,
        },
        Target {
            label: "221.14-ins.y",
            mm: 221.14 - ins_y_mm,
        },
        Target {
            label: "ins.y+11.53",
            mm: ins_y_mm + 11.53,
        },
        Target {
            label: "ins.y+12.70",
            mm: ins_y_mm + 12.70,
        },
        Target {
            label: "221.14",
            mm: 221.14,
        },
        Target {
            label: "209.61+12.70",
            mm: 222.31,
        },
    ];
    if local_y_mm.is_finite() && local_y_mm.abs() > 1.0 {
        t.push(Target {
            label: "local.y+11.53",
            mm: local_y_mm + 11.53,
        });
        t.push(Target {
            label: "local.y+12.70",
            mm: local_y_mm + 12.70,
        });
    }
    t
}

fn readings(raw: f64) -> [(&'static str, f64); 3] {
    [
        ("米→mm", raw * 1000.0),
        ("当作mm", raw),
        ("英寸→mm", raw * 25.4),
    ]
}

fn consider(hits: &mut Vec<Hit>, where_: &str, kind: Kind, raw: f64, targets: &[Target]) {
    if !raw.is_finite() || raw == 0.0 || raw.abs() > 1.0e7 {
        return;
    }
    for (reading, mm) in readings(raw) {
        if !mm.is_finite() || mm.abs() > 10_000.0 {
            continue;
        }
        for target in targets {
            let delta = (mm - target.mm).abs();
            if delta <= TOL_MM {
                hits.push(Hit {
                    where_: where_.to_string(),
                    kind,
                    raw,
                    reading,
                    mm,
                    target: format!("{} ({:.4} mm)", target.label, target.mm),
                    delta,
                });
            }
        }
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::U32 => "u32",
        Kind::F32 => "f32",
        Kind::F64 => "f64",
    }
}

fn cell_text(payload: &[u8], kind: Kind, at: usize) -> String {
    match kind {
        Kind::U32 => u32_at(payload, at)
            .map(|v| format!("{v} (0x{v:X})"))
            .unwrap_or_else(|| "—".into()),
        Kind::F32 => f32_at(payload, at)
            .map(|v| {
                if v.is_finite() {
                    format!("{v:.8}")
                } else {
                    format!("{v}")
                }
            })
            .unwrap_or_else(|| "—".into()),
        Kind::F64 => f64_at(payload, at)
            .map(|v| {
                if v.is_finite() {
                    format!("{v:.10}")
                } else {
                    format!("{v}")
                }
            })
            .unwrap_or_else(|| "—".into()),
    }
}

fn conv_text(payload: &[u8], kind: Kind, at: usize) -> String {
    let raw = match kind {
        Kind::U32 => u32_at(payload, at).map(f64::from),
        Kind::F32 => f32_at(payload, at).map(f64::from),
        Kind::F64 => f64_at(payload, at),
    };
    let Some(raw) = raw else {
        return "—".into();
    };
    if !raw.is_finite() {
        return "非有限".into();
    }
    if raw == 0.0 {
        return "0".into();
    }
    let [a, b, c] = readings(raw);
    format!("{:.4} / {:.4} / {:.4}", a.1, b.1, c.1)
}

fn grid_keys(len: usize) -> BTreeSet<(Kind, usize)> {
    let mut keys = BTreeSet::new();
    let mut at = 0;
    while at + 4 <= len {
        keys.insert((Kind::U32, at));
        keys.insert((Kind::F32, at));
        at += 4;
    }
    at = 0;
    while at + 8 <= len {
        keys.insert((Kind::F64, at));
        at += 8;
    }
    keys
}

fn score_grid(hits: &mut Vec<Hit>, label: &str, payload: &[u8], targets: &[Target]) {
    for (kind, at) in grid_keys(payload.len()) {
        let raw = match kind {
            Kind::U32 => u32_at(payload, at).map(f64::from),
            Kind::F32 => f32_at(payload, at).map(f64::from),
            Kind::F64 => f64_at(payload, at),
        };
        if let Some(raw) = raw {
            consider(
                hits,
                &format!("{label} {}@{at}", kind_name(kind)),
                kind,
                raw,
                targets,
            );
        }
    }
}

fn score_unaligned(hits: &mut Vec<Hit>, label: &str, payload: &[u8], targets: &[Target]) {
    if payload.len() < 4 {
        return;
    }
    for at in 0..payload.len().saturating_sub(3) {
        if at % 4 == 0 {
            continue;
        }
        if let Some(raw) = f32_at(payload, at) {
            consider(
                hits,
                &format!("{label} f32@{at} unaligned"),
                Kind::F32,
                f64::from(raw),
                targets,
            );
        }
    }
    if payload.len() < 8 {
        return;
    }
    for at in 0..payload.len().saturating_sub(7) {
        if at % 8 == 0 {
            continue;
        }
        if let Some(raw) = f64_at(payload, at) {
            consider(
                hits,
                &format!("{label} f64@{at} unaligned"),
                Kind::F64,
                raw,
                targets,
            );
        }
    }
}

fn hex_dump(payload: &[u8]) -> String {
    let mut out = String::new();
    for start in (0..payload.len()).step_by(16) {
        let end = (start + 16).min(payload.len());
        let hex: Vec<String> = payload[start..end]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect();
        out.push_str(&format!("+{start:03}: {}\n", hex.join(" ")));
    }
    out
}

fn tag_at(payload: &[u8]) -> Option<usize> {
    payload.windows(4).position(|w| w == MATRIX_TAG)
}

fn sheets_words(payload: &[u8]) -> Option<(usize, u32, u32)> {
    let needle: Vec<u8> = "Sheets".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let at = payload
        .windows(needle.len())
        .position(|w| w == needle.as_slice())?;
    let after = at + needle.len();
    Some((after, u32_at(payload, after)?, u32_at(payload, after + 4)?))
}

type LoadedStreams = Vec<(String, Vec<u8>)>;

fn load_streams(path: &Path) -> Result<LoadedStreams, Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let paths: Vec<String> = cfb
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_string_lossy().replace('\\', "/"))
        .filter(|p| {
            let name = p.rsplit('/').next().unwrap_or("");
            name == "PSMcluster0" || name.starts_with("Sheet")
        })
        .collect();
    let mut out = Vec::new();
    for stream in paths {
        let mut data = Vec::new();
        cfb.open_stream(&stream)?.read_to_end(&mut data)?;
        out.push((stream, data));
    }
    Ok(out)
}

fn chain_records(stream: &str, data: &[u8]) -> Vec<RawRec> {
    let mut out = Vec::new();
    for at in sheet_record_starts(data) {
        let Some(header) = parse_psm_header(data, at) else {
            continue;
        };
        let end = match header
            .body_start
            .checked_add(header.bytes_to_follow as usize)
        {
            Some(end) if end <= data.len() => end,
            _ => continue,
        };
        out.push(RawRec {
            stream: stream.to_string(),
            at,
            type_code: header.type_code,
            flags: header.type_flags,
            skipped: header.native_reader_skips(),
            payload: data[header.body_start..end].to_vec(),
        });
    }
    out
}

fn find_rec(recs: &[RawRec], type_code: u16, oid: u32) -> Option<&RawRec> {
    recs.iter()
        .filter(|r| r.type_code == type_code && !r.skipped && u32_at(&r.payload, 0) == Some(oid))
        .min_by_key(|r| r.at)
}

fn si_same(a: &PsmSymbolInformationDecoded, b: &PsmSymbolInformationDecoded) -> bool {
    if (a.extents.0 - b.extents.0).abs() > 1e-9 || (a.extents.1 - b.extents.1).abs() > 1e-9 {
        return false;
    }
    if a.variables.len() != b.variables.len() {
        return false;
    }
    a.variables
        .iter()
        .zip(b.variables.iter())
        .all(|(left, right)| left.name == right.name && (left.value - right.value).abs() <= 1e-9)
}

fn storage_of(stream: &str) -> String {
    match stream.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => stream.to_string(),
    }
}

fn md_escape(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn table_grid(title: &str, cols: &[(&str, &[u8])]) -> String {
    let mut keys = BTreeSet::new();
    for (_, payload) in cols {
        keys.extend(grid_keys(payload.len()));
    }
    let mut out = format!("### {title}\n\n");
    out.push_str("| 偏移 | 类型 | ");
    for (name, _) in cols {
        out.push_str(&format!("{name} | {name} 换算 mm（米 / 当作mm / 英寸） | "));
    }
    out.push_str("\n|---:|:---|");
    for _ in cols {
        out.push_str("---|---|");
    }
    out.push('\n');
    for (kind, at) in keys {
        out.push_str(&format!("| {at} | {} |", kind_name(kind)));
        for (_, payload) in cols {
            if at
                + match kind {
                    Kind::F64 => 8,
                    _ => 4,
                }
                <= payload.len()
            {
                out.push_str(&format!(
                    " {} | {} |",
                    md_escape(&cell_text(payload, kind, at)),
                    md_escape(&conv_text(payload, kind, at))
                ));
            } else {
                out.push_str(" — | — |");
            }
        }
        out.push('\n');
    }
    out.push('\n');
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let doc = PidParser::new().parse_file(&path)?;
    let mut sites: BTreeMap<u32, SiteRow> = BTreeMap::new();
    for site in &doc.jsites {
        let Some(id) = site.name.strip_prefix("JSite").and_then(|s| s.parse().ok()) else {
            continue;
        };
        let path = site
            .symbol_path
            .clone()
            .or_else(|| site.local_symbol_path.clone())
            .unwrap_or_default();
        let file = if site.symbol_name.as_deref().unwrap_or("").trim().is_empty() {
            file_name(&path).to_string()
        } else {
            site.symbol_name.clone().unwrap_or_default()
        };
        sites.insert(id, SiteRow { file, path });
    }

    let streams = load_streams(&path)?;
    let mut placed = Vec::new();
    for (stream, data) in &streams {
        if !stream.rsplit('/').next().unwrap_or("").starts_with("Sheet") {
            continue;
        }
        for sym in decode_igsymbols(data) {
            let start = sym.byte_range.start + PSM_ENVELOPE_LEN;
            let payload = data.get(start..sym.byte_range.end).unwrap_or_default();
            let site = sites.get(&sym.jsite_ref);
            let file = site.map(|s| s.file.clone()).unwrap_or_default();
            let symbol_path = site.map(|s| s.path.clone()).unwrap_or_default();
            let norm = file.trim();
            let role = if norm.eq_ignore_ascii_case("Line2.sym") {
                "Line2"
            } else if norm.eq_ignore_ascii_case("Parametric Manifold.sym") {
                "Manifold"
            } else if norm.eq_ignore_ascii_case("Ball Valve Type 2.sym") {
                "BallValve2"
            } else if norm.eq_ignore_ascii_case("Cap.sym") {
                "Cap"
            } else if norm.eq_ignore_ascii_case("LG-Magnetic Float Gauge.sym") {
                "LG"
            } else {
                continue;
            };
            placed.push(Placed {
                role,
                symbol_file: file,
                symbol_path,
                stream: stream.clone(),
                offset: sym.byte_range.start,
                payload: payload.to_vec(),
                jsite_ref: sym.jsite_ref,
                def_sheet: sym.definition_sheet_ref,
                def_site: sym.definition_site_ref,
                insertion: sym.insertion,
                oid: sym.oid,
            });
        }
    }

    let control_role = ["BallValve2", "Cap", "LG"]
        .into_iter()
        .find(|role| placed.iter().any(|p| p.role == *role))
        .unwrap_or("BallValve2");

    let mut chosen: Vec<&Placed> = Vec::new();
    for role in ["Line2", "Manifold", control_role] {
        let mut group: Vec<&Placed> = placed.iter().filter(|p| p.role == role).collect();
        if role == "Line2" {
            group.sort_by(|a, b| {
                let da = (a.insertion.0 - 0.03398).abs() + (a.insertion.1 - 0.05721).abs();
                let db = (b.insertion.0 - 0.03398).abs() + (b.insertion.1 - 0.05721).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        } else if role == "Manifold" {
            group.sort_by_key(|p| u8::from(p.def_site != 396));
        }
        if let Some(best) = group.first() {
            chosen.push(*best);
        }
    }

    let line2 = pick_owned(&chosen, "Line2");
    let ins_y_mm = line2.map(|p| p.insertion.1 * 1000.0).unwrap_or(57.21);
    let mut recs = Vec::new();
    for (stream, data) in &streams {
        if stream.ends_with("/PSMcluster0") || stream.rsplit('/').next() == Some("PSMcluster0") {
            recs.extend(chain_records(stream, data));
        }
    }

    let line2_si = line2.and_then(|p| instance_si(&recs, p));
    let local_y_mm = line2_si
        .map(|(_, si)| si.extents.1 * 1000.0)
        .unwrap_or(152.4);
    let targets = targets_for(ins_y_mm, local_y_mm);

    let mut hits = Vec::new();
    let mut md = String::new();
    md.push_str("# 探针原文（probe_line2_offset_field_hunt）\n\n");
    md.push_str(&format!(
        "夹具 `{FIXTURE}`。对照符号角色 `{control_role}`（09-07 逐字节对上库的那批：优先 Ball Valve Type 2，其次 Cap，其次 LG-Magnetic Float Gauge）。\n\n"
    ));
    md.push_str(&format!(
        "命中阈值 ±{TOL_MM} mm。换算三列是「按米×1000 / 原数当作 mm / 按英寸×25.4」。组合靶：221.14−insertion.y = {:.4} mm（insertion.y 取本轮 Line2 放置），local.y 取 Line2 SymbolInformation.extents.y = {:.4} mm。\n\n",
        221.14 - ins_y_mm,
        local_y_mm
    ));

    md.push_str("## 选中的放置\n\n");
    md.push_str("| 角色 | 符号 | jsite | 记录 oid | 流 | 偏移 | insertion mm | def site | def sheet | payload |\n");
    md.push_str("|---|---|---:|---:|---|---:|---|---:|---:|---:|\n");
    for p in &chosen {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | 0x{:X} | ({:.4}, {:.4}) | {} | {} | {} |\n",
            p.role,
            md_escape(p.symbol_file.trim()),
            p.jsite_ref,
            p.oid,
            md_escape(&p.stream),
            p.offset,
            p.insertion.0 * 1000.0,
            p.insertion.1 * 1000.0,
            p.def_site,
            p.def_sheet,
            p.payload.len()
        ));
    }
    md.push_str("\n同名放置全部列出（上面表格只倒第一优先那条的字段）：\n\n");
    for p in &placed {
        if ["Line2", "Manifold", control_role].contains(&p.role) {
            md.push_str(&format!(
                "- {} oid {} @ {}+0x{:X} ins=({:.4},{:.4}) mm def={}/{} jsite={}\n",
                p.role,
                p.oid,
                p.stream,
                p.offset,
                p.insertion.0 * 1000.0,
                p.insertion.1 * 1000.0,
                p.def_site,
                p.def_sheet,
                p.jsite_ref
            ));
            md.push_str(&format!("  path `{}`\n", md_escape(&p.symbol_path)));
        }
    }
    md.push('\n');

    let mut payloads: Vec<(&str, Vec<u8>)> = Vec::new();
    for p in &chosen {
        payloads.push((p.role, p.payload.clone()));
        score_grid(
            &mut hits,
            &format!("{} igSymbol2d", p.role),
            &p.payload,
            &targets,
        );
        score_unaligned(
            &mut hits,
            &format!("{} igSymbol2d", p.role),
            &p.payload,
            &targets,
        );
    }
    let cols: Vec<(&str, &[u8])> = payloads.iter().map(|(n, p)| (*n, p.as_slice())).collect();
    md.push_str(&table_grid(
        "igSymbol2d 放置记录：按 4 字节 u32/f32 与 8 字节 f64 对齐的全部槽",
        &cols,
    ));
    md.push_str("## igSymbol2d 具名字段（含未命名间隙）\n\n");
    for p in &chosen {
        md.push_str(&format!("### {} 具名走查\n\n", p.role));
        md.push_str(&named_symbol(&p.payload));
        md.push_str("```\n");
        md.push_str(&hex_dump(&p.payload));
        md.push_str("```\n\n");
    }

    md.push_str("## JFlavorHolder\n\n");
    let mut flavor_cols: Vec<(String, Vec<u8>)> = Vec::new();
    for p in &chosen {
        let holders = flavors_for(&recs, p);
        if holders.is_empty() {
            md.push_str(&format!(
                "- {}：没有 link==jsite_ref {} 的实例 JFlavorHolder。\n",
                p.role, p.jsite_ref
            ));
            continue;
        }
        for (rec, decoded) in holders {
            let label = format!(
                "{} flavor oid {} {}",
                p.role,
                decoded.oid,
                if decoded.variant == 1 {
                    "instance"
                } else {
                    "template"
                }
            );
            md.push_str(&format!(
                "### {label}\n\n流 `{}` @ 0x{:X}，payload {} 字节，link={}，variant={}，values={:?}\n\n",
                rec.stream,
                rec.at,
                rec.payload.len(),
                decoded.link,
                decoded.variant,
                decoded.values
            ));
            if let Some((after, a, b)) = sheets_words(&rec.payload) {
                md.push_str(&format!(
                    "\"Sheets\" 后两个 u32 @ payload+{after}: {a} (0x{a:X}) , {b} (0x{b:X})\n\n"
                ));
                consider(
                    &mut hits,
                    &format!("{label} sheets_u32_a"),
                    Kind::U32,
                    f64::from(a),
                    &targets,
                );
                consider(
                    &mut hits,
                    &format!("{label} sheets_u32_b"),
                    Kind::U32,
                    f64::from(b),
                    &targets,
                );
                for (i, word) in [a, b].into_iter().enumerate() {
                    consider(
                        &mut hits,
                        &format!("{label} sheets_u32_{i} as f32 bits"),
                        Kind::F32,
                        f64::from(f32::from_bits(word)),
                        &targets,
                    );
                }
            } else {
                md.push_str("没有找到 UTF-16 \"Sheets\"。\n\n");
            }
            md.push_str(&named_flavor(&rec.payload));
            score_grid(&mut hits, &label, &rec.payload, &targets);
            score_unaligned(&mut hits, &label, &rec.payload, &targets);
            flavor_cols.push((label, rec.payload.clone()));
            md.push_str("```\n");
            md.push_str(&hex_dump(&rec.payload));
            md.push_str("```\n\n");
        }
    }

    md.push_str("## SymbolInformation\n\n");
    let mut si_cols: Vec<(String, Vec<u8>)> = Vec::new();
    for p in &chosen {
        if let Some((rec, si)) = instance_si(&recs, p) {
            dump_si(
                &mut md,
                &mut hits,
                &targets,
                &format!("{} instance SI", p.role),
                rec,
                &si,
                &mut si_cols,
            );
            if let Some((trec, tsi)) = template_si(&recs, rec, &si) {
                dump_si(
                    &mut md,
                    &mut hits,
                    &targets,
                    &format!("{} template SI", p.role),
                    trec,
                    &tsi,
                    &mut si_cols,
                );
                if let Some(holder) = template_flavor(&recs, tsi.oid) {
                    let label = format!("{} template flavor oid {}", p.role, holder.0.oid_of());
                    md.push_str(&format!(
                        "### {label}\n\n流 `{}` @ 0x{:X} symbol_information_ref={:?} link={}\n\n",
                        holder.1.stream,
                        holder.1.at,
                        holder.0.symbol_information_ref,
                        holder.0.link
                    ));
                    if let Some((after, a, b)) = sheets_words(&holder.1.payload) {
                        md.push_str(&format!(
                            "\"Sheets\" 后两个 u32 @ payload+{after}: {a} (0x{a:X}) , {b} (0x{b:X})\n\n"
                        ));
                    }
                    md.push_str(&named_flavor(&holder.1.payload));
                    score_grid(&mut hits, &label, &holder.1.payload, &targets);
                    score_unaligned(&mut hits, &label, &holder.1.payload, &targets);
                    flavor_cols.push((label, holder.1.payload.clone()));
                }
            } else {
                md.push_str(&format!(
                    "- {}：没有找到 extents/变量都相同的模板 SymbolInformation。\n",
                    p.role
                ));
            }
        } else {
            md.push_str(&format!("- {}：没有配上实例 SymbolInformation。\n", p.role));
        }
    }
    if !si_cols.is_empty() {
        let view: Vec<(&str, &[u8])> = si_cols
            .iter()
            .map(|(n, p)| (n.as_str(), p.as_slice()))
            .collect();
        md.push_str(&table_grid("SymbolInformation 对齐槽并排", &view));
    }
    if !flavor_cols.is_empty() {
        let view: Vec<(&str, &[u8])> = flavor_cols
            .iter()
            .map(|(n, p)| (n.as_str(), p.as_slice()))
            .collect();
        md.push_str(&table_grid("JFlavorHolder 对齐槽并排", &view));
    }

    md.push_str("## JSheet 头部\n\n");
    md.push_str("本仓 `jsheet_oids` 只读 oid。下面把每条选中 `0x0114` 的负载从 +0 起按 u32/f32/f64 全部列出，未命名字节都在表里。\n\n");
    let mut sheet_cols: Vec<(String, Vec<u8>)> = Vec::new();
    let mut wanted: Vec<(String, u32)> = Vec::new();
    for p in &chosen {
        wanted.push((format!("{} def sheet", p.role), p.def_sheet));
    }
    if let Some(p) = chosen.iter().find(|p| p.role == "Line2") {
        let _ = p;
        wanted.push(("Line2 template sheet anchor 501".into(), 501));
    }
    for (label, oid) in wanted {
        match find_rec(&recs, PSM_TYPE_CODE_JSHEET, oid) {
            Some(rec) => {
                md.push_str(&format!(
                    "### {label} oid {oid}\n\n流 `{}` @ 0x{:X}，flags={}，skipped={}，payload {} 字节\n\n",
                    rec.stream,
                    rec.at,
                    rec.flags,
                    rec.skipped,
                    rec.payload.len()
                ));
                md.push_str("```\n");
                md.push_str(&hex_dump(&rec.payload));
                md.push_str("```\n\n");
                score_grid(
                    &mut hits,
                    &format!("{label} oid {oid}"),
                    &rec.payload,
                    &targets,
                );
                score_unaligned(
                    &mut hits,
                    &format!("{label} oid {oid}"),
                    &rec.payload,
                    &targets,
                );
                sheet_cols.push((format!("{label} oid {oid}"), rec.payload.clone()));
            }
            None => md.push_str(&format!(
                "- {label} oid {oid}：集群流里没有这条 live JSheet。\n"
            )),
        }
    }
    if !sheet_cols.is_empty() {
        let view: Vec<(&str, &[u8])> = sheet_cols
            .iter()
            .map(|(n, p)| (n.as_str(), p.as_slice()))
            .collect();
        md.push_str(&table_grid("JSheet 对齐槽并排", &view));
    }

    md.push_str("## 这两个缓存里的 JSheet oid\n\n");
    for storage in ["/JSite396", "/JSite329"] {
        md.push_str(&format!("`{storage}`:\n"));
        for rec in recs.iter().filter(|r| {
            r.type_code == PSM_TYPE_CODE_JSHEET && !r.skipped && storage_of(&r.stream) == storage
        }) {
            let oid = u32_at(&rec.payload, 0).unwrap_or(0);
            md.push_str(&format!(
                "- oid {oid} payload {} @ {}+0x{:X}\n",
                rec.payload.len(),
                rec.stream,
                rec.at
            ));
        }
        md.push('\n');
    }

    md.push_str("## 两个缓存里的小浮点\n\n");
    md.push_str("只列 `|f32|∈(1e-4, 10)` 且不是 1，以及 `|f64|∈(1e-4, 5)` 且不是 1。用来判断 0.5 这种 f32 是不是某张 sheet 独有。\n\n");
    for rec in recs.iter().filter(|r| {
        r.type_code == PSM_TYPE_CODE_JSHEET
            && !r.skipped
            && (storage_of(&r.stream) == "/JSite396" || storage_of(&r.stream) == "/JSite329")
    }) {
        let oid = u32_at(&rec.payload, 0).unwrap_or(0);
        let word4 = u32_at(&rec.payload, 4).unwrap_or(0);
        let mut notes = vec![format!("+4={word4} (0x{word4:X})")];
        let mut at = 0;
        while at + 4 <= rec.payload.len() {
            if let Some(v) = f32_at(&rec.payload, at) {
                let v = f64::from(v);
                if v.is_finite() && (1e-4..10.0).contains(&v.abs()) && (v - 1.0).abs() > 1e-4 {
                    notes.push(format!("f32@{at}={v:.6}"));
                }
            }
            at += 4;
        }
        at = 0;
        while at + 8 <= rec.payload.len() {
            if let Some(v) = f64_at(&rec.payload, at) {
                if v.is_finite() && (1e-4..5.0).contains(&v.abs()) && (v - 1.0).abs() > 1e-6 {
                    notes.push(format!("f64@{at}={v:.6}"));
                }
            }
            at += 8;
        }
        if notes.is_empty() {
            md.push_str(&format!("- JSheet {oid} @ `{}`：无\n", rec.stream));
        } else {
            md.push_str(&format!(
                "- JSheet {oid} @ `{}`：{}\n",
                rec.stream,
                notes.join("；")
            ));
        }
    }
    md.push('\n');

    native_vs_pun(&mut md, &hits);
    print!("{md}");
    Ok(())
}

fn pick_owned<'a>(chosen: &[&'a Placed], role: &str) -> Option<&'a Placed> {
    chosen.iter().copied().find(|p| p.role == role)
}

fn instance_si<'a>(
    recs: &'a [RawRec],
    placed: &Placed,
) -> Option<(&'a RawRec, PsmSymbolInformationDecoded)> {
    let holders = flavors_for(recs, placed);
    let holder_oids: Vec<u32> = holders.iter().map(|(_, d)| d.oid).collect();
    let storage = format!("/JSite{}", placed.def_site);
    let mut infos = Vec::new();
    for rec in recs {
        if rec.type_code != PSM_TYPE_CODE_JSYMBOL_INFORMATION || rec.skipped {
            continue;
        }
        if storage_of(&rec.stream) != storage {
            continue;
        }
        let decoded = decode_symbol_informations(&wrap_record(rec));
        if let Some(si) = decoded.into_iter().next() {
            infos.push((rec, si));
        }
    }
    if let Some(index) = infos
        .iter()
        .position(|(_, si)| holder_oids.contains(&si.parent_ref))
    {
        let (rec, si) = &infos[index];
        return Some((*rec, si.clone()));
    }
    let counts: Vec<usize> = holders
        .iter()
        .map(|(_, holder)| holder.values.len())
        .collect();
    let by_count: Vec<usize> = infos
        .iter()
        .enumerate()
        .filter(|(_, (_, si))| counts.contains(&si.variables.len()))
        .map(|(index, _)| index)
        .collect();
    if by_count.len() == 1 {
        let (rec, si) = &infos[by_count[0]];
        return Some((*rec, si.clone()));
    }
    if infos.len() == 1 {
        let (rec, si) = &infos[0];
        return Some((*rec, si.clone()));
    }
    None
}

fn template_si<'a>(
    recs: &'a [RawRec],
    instance_rec: &RawRec,
    instance: &PsmSymbolInformationDecoded,
) -> Option<(&'a RawRec, PsmSymbolInformationDecoded)> {
    let home = storage_of(&instance_rec.stream);
    for rec in recs {
        if rec.type_code != PSM_TYPE_CODE_JSYMBOL_INFORMATION || rec.skipped {
            continue;
        }
        if storage_of(&rec.stream) == home {
            continue;
        }
        let decoded = decode_symbol_informations(&wrap_record(rec));
        if let Some(si) = decoded.into_iter().next() {
            if si_same(instance, &si) {
                return Some((rec, si));
            }
        }
    }
    None
}

fn template_flavor(recs: &[RawRec], si_oid: u32) -> Option<(PsmFlavorHolderDecoded, &RawRec)> {
    for rec in recs {
        if rec.type_code != PSM_TYPE_CODE_JFLAVOR_HOLDER || rec.skipped {
            continue;
        }
        let decoded = decode_flavor_holders(&wrap_record(rec));
        if let Some(holder) = decoded.into_iter().next() {
            if holder.symbol_information_ref == Some(si_oid) {
                return Some((holder, rec));
            }
        }
    }
    None
}

fn flavors_for<'a>(
    recs: &'a [RawRec],
    placed: &Placed,
) -> Vec<(&'a RawRec, PsmFlavorHolderDecoded)> {
    let mut out = Vec::new();
    for rec in recs {
        if rec.type_code != PSM_TYPE_CODE_JFLAVOR_HOLDER || rec.skipped {
            continue;
        }
        let decoded = decode_flavor_holders(&wrap_record(rec));
        if let Some(holder) = decoded.into_iter().next() {
            if holder.variant == 1 && holder.link == placed.jsite_ref {
                out.push((rec, holder));
            }
        }
    }
    out
}

fn wrap_record(rec: &RawRec) -> Vec<u8> {
    let mut data = vec![0u8; 8];
    data.extend_from_slice(&rec.type_code.to_le_bytes());
    data.extend_from_slice(&(rec.payload.len() as u32).to_le_bytes());
    data.extend_from_slice(&rec.payload);
    data
}

fn dump_si(
    md: &mut String,
    hits: &mut Vec<Hit>,
    targets: &[Target],
    label: &str,
    rec: &RawRec,
    si: &PsmSymbolInformationDecoded,
    cols: &mut Vec<(String, Vec<u8>)>,
) {
    md.push_str(&format!(
        "### {label}\n\noid {} parent {} kind {} flags {} extents ({:.10}, {:.10}) 流 `{}` @ 0x{:X} payload {}\n\n",
        si.oid,
        si.parent_ref,
        si.kind,
        si.flags,
        si.extents.0,
        si.extents.1,
        rec.stream,
        rec.at,
        rec.payload.len()
    ));
    md.push_str("| 变量 | 值 m | 值 mm | value_ref |\n|---|---:|---:|---:|\n");
    for var in &si.variables {
        md.push_str(&format!(
            "| {} | {:.10} | {:.4} | {} |\n",
            md_escape(&var.name),
            var.value,
            var.value * 1000.0,
            var.value_ref
        ));
        consider(
            hits,
            &format!("{label} var {}", var.name),
            Kind::F64,
            var.value,
            targets,
        );
    }
    if si.variables.is_empty() {
        md.push_str("| （无变量） | | | |\n");
    }
    md.push('\n');
    consider(
        hits,
        &format!("{label} extents.x"),
        Kind::F64,
        si.extents.0,
        targets,
    );
    consider(
        hits,
        &format!("{label} extents.y"),
        Kind::F64,
        si.extents.1,
        targets,
    );
    score_grid(hits, label, &rec.payload, targets);
    score_unaligned(hits, label, &rec.payload, targets);
    md.push_str("```\n");
    md.push_str(&hex_dump(&rec.payload));
    md.push_str("```\n\n");
    cols.push((label.to_string(), rec.payload.clone()));
}

fn named_symbol(payload: &[u8]) -> String {
    let mut out = String::from("| 字段 | 偏移 | 值 |\n|---|---:|---|\n");
    let mut covered = vec![false; payload.len()];
    let mut row = |name: &str, at: usize, len: usize, text: String, covered: &mut [bool]| {
        out.push_str(&format!("| {name} | {at} | {text} |\n"));
        let end = (at + len).min(covered.len());
        if at < end {
            for slot in &mut covered[at..end] {
                *slot = true;
            }
        }
    };
    if let Some(v) = u32_at(payload, 0) {
        row("oid", 0, 4, format!("{v}"), &mut covered);
    }
    if let Some(v) = u32_at(payload, 4) {
        row("parent_ref", 4, 4, format!("{v}"), &mut covered);
    }
    if let Some(v) = u32_at(payload, 8) {
        row("sheet_layer_ref", 8, 4, format!("{v}"), &mut covered);
    }
    if let Some(v) = u16_at(payload, 12) {
        row("sub_type_word", 12, 2, format!("{v}"), &mut covered);
    }
    if payload.len() >= 29 {
        row(
            "style_ref",
            25,
            4,
            u32_at(payload, 25)
                .map(|v| format!("{v}"))
                .unwrap_or_default(),
            &mut covered,
        );
    }
    if let Some(tag) = tag_at(payload) {
        if tag >= 4 {
            row(
                "jsite_ref",
                tag - 4,
                4,
                u32_at(payload, tag - 4)
                    .map(|v| format!("{v}"))
                    .unwrap_or_default(),
                &mut covered,
            );
        }
        row("matrix_tag", tag, 4, "02 00 A7 50".into(), &mut covered);
        let names = [
            "m00",
            "m01",
            "m10",
            "m11",
            "insertion.x",
            "insertion.y",
            "f64_1.0",
        ];
        for (i, name) in names.iter().enumerate() {
            let at = tag + 4 + i * 8;
            if let Some(v) = f64_at(payload, at) {
                row(
                    name,
                    at,
                    8,
                    format!("{v:.10} → {:.4} mm（按米）", v * 1000.0),
                    &mut covered,
                );
            }
        }
        let tail = tag + 4 + 7 * 8;
        let names_tail = ["flags", "has_membassy", "tail_u32_0"];
        if tail + 12 <= payload.len() {
            for (i, name) in names_tail.iter().enumerate() {
                let at = tail + i * 4;
                row(
                    name,
                    at,
                    4,
                    u32_at(payload, at)
                        .map(|v| format!("{v} (0x{v:X})"))
                        .unwrap_or_default(),
                    &mut covered,
                );
            }
        }
        if payload.len() >= 8 {
            row(
                "definition_sheet_ref",
                payload.len() - 8,
                4,
                u32_at(payload, payload.len() - 8)
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                &mut covered,
            );
            row(
                "definition_site_ref",
                payload.len() - 4,
                4,
                u32_at(payload, payload.len() - 4)
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                &mut covered,
            );
        }
        let mid_end = payload.len().saturating_sub(8);
        let mut at = tail + 12;
        let mut n = 0;
        while at + 4 <= mid_end {
            row(
                &format!("tail_mid_{n}"),
                at,
                4,
                u32_at(payload, at)
                    .map(|v| format!("{v} (0x{v:X})"))
                    .unwrap_or_default(),
                &mut covered,
            );
            at += 4;
            n += 1;
        }
    }
    let mut at = 0;
    while at < covered.len() {
        if covered[at] {
            at += 1;
            continue;
        }
        let start = at;
        while at < covered.len() && !covered[at] {
            at += 1;
        }
        let hex: Vec<String> = payload[start..at]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect();
        row("unnamed", start, at - start, hex.join(" "), &mut [false; 0]);
    }
    let accounted = covered.iter().filter(|c| **c).count();
    out.push_str(&format!(
        "\n具名覆盖 {accounted}/{} 字节（unnamed 行是没被具名字段吃掉的字节）。\n\n",
        payload.len()
    ));
    out
}

fn named_flavor(payload: &[u8]) -> String {
    let mut out = String::from("| 字段 | 偏移 | 值 |\n|---|---:|---|\n");
    let mut push = |name: &str, at: usize, text: String| {
        out.push_str(&format!("| {name} | {at} | {text} |\n"));
    };
    if let Some(v) = u32_at(payload, 0) {
        push("oid", 0, v.to_string());
    }
    if let Some(v) = u32_at(payload, 4) {
        push("zero_4", 4, format!("{v} (0x{v:X})"));
    }
    if let Some(v) = u32_at(payload, 8) {
        push("zero_8", 8, format!("{v} (0x{v:X})"));
    }
    if let Some(v) = u16_at(payload, 12) {
        push("zero_12", 12, v.to_string());
    }
    if let Some(v) = u32_at(payload, 14) {
        push("link", 14, format!("{v} (0x{v:X})"));
    }
    if let Some(v) = u16_at(payload, 18) {
        push("count", 18, v.to_string());
    }
    if let Some(v) = u16_at(payload, 20) {
        push("lead_0101", 20, format!("0x{v:04X}"));
    }
    if let Some(v) = u32_at(payload, 22) {
        push("variant", 22, v.to_string());
    }
    if let Some((after, a, b)) = sheets_words(payload) {
        push("sheets_u32_a", after, format!("{a} (0x{a:X})"));
        push("sheets_u32_b", after + 4, format!("{b} (0x{b:X})"));
        let mut at = after + 8;
        let mut n = 0;
        while at + 13 <= payload.len() {
            let lead = payload.get(at..at + 5).unwrap_or_default();
            let value = f64_at(payload, at + 5).unwrap_or(f64::NAN);
            push(
                &format!("value_{n}"),
                at,
                format!(
                    "lead {:02X?} value {value:.10} → {:.4} mm（按米）",
                    lead,
                    value * 1000.0
                ),
            );
            at += 13;
            n += 1;
        }
        if at < payload.len() {
            let hex: Vec<String> = payload[at..].iter().map(|b| format!("{b:02X}")).collect();
            push("tail_after_values", at, hex.join(" "));
        }
    }
    out.push('\n');
    out
}

fn native_vs_pun(md: &mut String, hits: &[Hit]) {
    md.push_str("## 命中\n\n");
    if hits.is_empty() {
        md.push_str("命中字段：无\n\n已查全部字段，无。对齐槽与未对齐 f32/f64 滑动窗都没有落在任一靶的 ±0.5 mm 内。\n");
        return;
    }
    let mut ordered = hits.to_vec();
    ordered.sort_by(|a, b| {
        a.delta
            .partial_cmp(&b.delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let aligned: Vec<&Hit> = ordered
        .iter()
        .filter(|h| !h.where_.contains("unaligned"))
        .collect();
    let unaligned = ordered.len() - aligned.len();
    md.push_str(&format!(
        "对齐槽命中 {} 条，未对齐滑动窗另有 {unaligned} 条（未对齐不进下表，避免字节错位凑出来的浮点）。\n\n",
        aligned.len()
    ));
    md.push_str(
        "| 位置 | 类型 | 读法 | 原值 | mm | 靶 | Δ mm |\n|---|---|---|---:|---:|---|---:|\n",
    );
    for hit in &aligned {
        md.push_str(&format!(
            "| {} | {} | {} | {:.8} | {:.4} | {} | {:.4} |\n",
            md_escape(&hit.where_),
            kind_name(hit.kind),
            hit.reading,
            hit.raw,
            hit.mm,
            md_escape(&hit.target),
            hit.delta
        ));
    }
    let native: Vec<&Hit> = ordered
        .iter()
        .filter(|h| {
            matches!(h.kind, Kind::F64 | Kind::F32)
                && h.reading == "米→mm"
                && !h.where_.contains("unaligned")
        })
        .collect();
    md.push('\n');
    if native.is_empty() {
        md.push_str(
            "命中字段：无\n\n已查全部字段，无。按文件单位米（×1000 = mm）没有任何对齐的 f32/f64 落在 11.53、12.70 或上列组合靶的 ±0.5 mm 内。表里其余行是把整数当毫米/英寸，或未对齐字节碰巧组成浮点。\n",
        );
    } else {
        let names: Vec<String> = native
            .iter()
            .map(|h| format!("{}（{} {:.4} mm）", h.where_, h.reading, h.mm))
            .collect();
        md.push_str(&format!("命中字段：{}\n", names.join("；")));
    }
}

trait OidOf {
    fn oid_of(&self) -> u32;
}

impl OidOf for PsmFlavorHolderDecoded {
    fn oid_of(&self) -> u32 {
        self.oid
    }
}
