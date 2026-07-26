//! Probe `igTextBox` style linkage and height/rotation candidates
//! (Phase 35-D).
//!
//! Open questions this probe answers with fixture bytes:
//! 1. Do `JStyleOverride` u32 fields reference `igTextBox` oids (or
//!    the other way around)?
//! 2. What do the undecoded `igTextBox` regions hold — the 12-byte
//!    sub-field block at payload `+18..30` and the 12-byte trailer
//!    after the 3 trailing doubles? Interpretations are printed as
//!    u16 / u32 / f32 / f64 so height-like magnitudes (2–5 mm in a
//!    ~1.2 m page, i.e. 0.002..0.005) stand out.
//! 3. For any matched pair, how do `JStyleOverride` f64 fields relate
//!    to the text (rotation clusters, height-like values)?
//!
//! Read-only: no parser, schema, or model change.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{decode_igtextboxes, decode_jstyle_overrides};
use pid_parse::PidPackage;

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "工艺管道及仪表流程-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
    "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn f64_at(bytes: &[u8], at: usize) -> Option<f64> {
    let c = bytes.get(at..at + 8)?;
    Some(f64::from_le_bytes(c.try_into().ok()?))
}

fn f32_at(bytes: &[u8], at: usize) -> Option<f32> {
    let c = bytes.get(at..at + 4)?;
    Some(f32::from_le_bytes(c.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let c = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes(c.try_into().ok()?))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_file_root();
    // connector-name -> (hits, candidates)
    let mut connector_hits: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();

    for rel in FIXTURES {
        let path = root.join(rel);
        if !path.exists() {
            println!("--- {rel}: MISSING, skipped");
            continue;
        }
        let pkg = PidPackage::from_path(&path)?;
        println!("\n=== {rel} ===");

        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            let texts = decode_igtextboxes(bytes);
            let styles = decode_jstyle_overrides(bytes);
            if texts.is_empty() && styles.is_empty() {
                continue;
            }
            println!(
                "  {}: {} igTextBox, {} JStyleOverride",
                sheet.path,
                texts.len(),
                styles.len()
            );

            let text_oids: BTreeMap<u32, usize> =
                texts.iter().enumerate().map(|(i, t)| (t.oid, i)).collect();
            let style_oids: BTreeMap<u32, usize> =
                styles.iter().enumerate().map(|(i, s)| (s.oid, i)).collect();

            // Direction A: JStyleOverride u32 fields -> igTextBox oid.
            for style in &styles {
                let fields: [(&'static str, u32); 8] = [
                    ("style.oid", style.oid),
                    ("style.field_a", style.field_a_u32),
                    ("style.field_b", style.field_b_u32),
                    ("style.field_c", style.field_c_u32),
                    ("style.field_d", style.field_d_u32),
                    ("style.field_e", style.field_e_u32),
                    ("style.field_f", style.field_f_u32),
                    ("style.field_g", style.field_g_u32),
                ];
                for (name, value) in fields {
                    let entry = connector_hits.entry(name).or_insert((0, 0));
                    entry.1 += 1;
                    if text_oids.contains_key(&value) {
                        entry.0 += 1;
                        println!(
                            "    LINK {name}={value} -> igTextBox[{}] {:?} \
                             (style oid={} rot={:.4} f1={:.6} f3={:.6} f4={:.6})",
                            text_oids[&value],
                            texts[text_oids[&value]].text,
                            style.oid,
                            style.field_2_f64,
                            style.field_1_f64,
                            style.field_3_f64,
                            style.field_4_f64,
                        );
                    }
                }
            }

            // Direction B: igTextBox sub-fields / trailer u32s -> JStyleOverride oid.
            for (index, text) in texts.iter().enumerate() {
                let start = text.byte_range.start + 6; // payload start
                let payload_len = text.bytes_to_follow as usize;
                let payload = &bytes[start..start + payload_len];
                let text_end = 32 + usize::from(text.text_length) * 2;
                let doubles_end = text_end + 24;
                let trailer = &payload[doubles_end..payload_len.min(doubles_end + 12)];
                let sub = &payload[18..30];

                for (name, at_block, block) in [
                    ("text.sub", 18usize, sub),
                    ("text.trailer", doubles_end, trailer),
                ] {
                    for at in (0..block.len().saturating_sub(3)).step_by(2) {
                        if let Some(v) = u32_at(block, at) {
                            let entry = connector_hits
                                .entry(if name == "text.sub" {
                                    "text.sub->style"
                                } else {
                                    "text.trailer->style"
                                })
                                .or_insert((0, 0));
                            if at == 0 {
                                entry.1 += 1;
                            }
                            if style_oids.contains_key(&v) {
                                entry.0 += 1;
                                println!(
                                    "    LINK {name}+{at} (abs +{}) = {v} -> JStyleOverride[{}] \
                                     for text {:?}",
                                    at_block + at,
                                    style_oids[&v],
                                    text.text
                                );
                            }
                        }
                    }
                }

                // Numeric interpretations of the two undecoded blocks.
                if index < 4 {
                    println!(
                        "    [{index}] oid={} len={} d3={:.6} text={:?}",
                        text.oid,
                        text.text_length,
                        text.trailing_double_3,
                        text.text.chars().take(18).collect::<String>()
                    );
                    println!("        sub    +18..30: {}", hex(sub));
                    println!(
                        "               u16s={:?}",
                        (0..sub.len() - 1)
                            .step_by(2)
                            .map(|i| u16::from_le_bytes([sub[i], sub[i + 1]]))
                            .collect::<Vec<_>>()
                    );
                    println!("        trailer +{doubles_end}: {}", hex(trailer));
                    let mut interp = Vec::new();
                    for at in [0usize, 4, 8] {
                        if let Some(v) = f32_at(trailer, at) {
                            if v.is_finite() && v.abs() > 1e-8 && v.abs() < 1e4 {
                                interp.push(format!("f32@{at}={v:.6}"));
                            }
                        }
                        if let Some(v) = u32_at(trailer, at) {
                            interp.push(format!("u32@{at}={v}"));
                        }
                    }
                    for at in [0usize, 4] {
                        if let Some(v) = f64_at(trailer, at) {
                            if v.is_finite() && v.abs() > 1e-8 && v.abs() < 1e4 {
                                interp.push(format!("f64@{at}={v:.6}"));
                            }
                        }
                    }
                    println!("               {}", interp.join(" "));
                }
            }

            // JStyleOverride field dumps (first few) for height clustering.
            for (index, style) in styles.iter().enumerate().take(4) {
                println!(
                    "    style[{index}] oid={} a={} b={} c={} d={} e={} f={} g={} h={} i={} \
                     f1={:.6} rot={:.6} f3={:.6} f4={:.6} tail={}B",
                    style.oid,
                    style.field_a_u32,
                    style.field_b_u32,
                    style.field_c_u32,
                    style.field_d_u32,
                    style.field_e_u32,
                    style.field_f_u32,
                    style.field_g_u32,
                    style.field_h_u16,
                    style.field_i_u16,
                    style.field_1_f64,
                    style.field_2_f64,
                    style.field_3_f64,
                    style.field_4_f64,
                    style.raw_attribute_tail.len(),
                );
            }
        }
    }

    println!("\n=== connector hit rates ===");
    for (name, (hits, total)) in &connector_hits {
        println!("  {name}: {hits}/{total}");
    }
    Ok(())
}
