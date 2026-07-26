//! Probe positional pairing between `JStyleOverride` records and
//! `igTextBox` records (Phase 35-D, second iteration).
//!
//! The first iteration (`probe_igtextbox_style_link.rs`) showed:
//! - u32 oid cross-references are dead ends (1/131 `field_f` hit; the
//!   `text.sub` hits are `0x0001_000N` length-prefix artifacts);
//! - `JStyleOverride.field_a/b` and `field_c/d` u32 pairs look like
//!   IEEE-754 halves of two f64 page coordinates (high words
//!   `0x3F?????`).
//!
//! This probe tests, per sheet:
//! 1. reinterpret `(field_a, field_b)` / `(field_c, field_d)` as f64
//!    and also try already-decoded `(field_3, field_4)`; measure the
//!    distance from each style record to the nearest `igTextBox`
//!    insertion -- if styles anchor at text positions, rotation can
//!    be attached positionally;
//! 2. scan each style's `raw_attribute_tail` for f64 values in the
//!    text-height domain (`0.0005..0.02` m) to locate the height
//!    field;
//! 3. count per-text trailer f32 pairs that pass a height/width
//!    plausibility gate, to quantify explicit-size coverage.
//!
//! Read-only: no parser, schema, or model change.

use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{decode_igtextboxes, decode_jstyle_overrides};
use pid_parse::PidPackage;

// "\u{5DE5}\u{827A}..." is the gongyi fixture (Chinese file name kept
// as escapes so this source stays pure ASCII).
const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
];

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn f64_from_u32_pair(low: u32, high: u32) -> f64 {
    f64::from_bits((u64::from(high) << 32) | u64::from(low))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_file_root();
    let mut all_min_dists: Vec<f64> = Vec::new();
    let mut rotated_report: Vec<String> = Vec::new();

    for rel in FIXTURES {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let pkg = PidPackage::from_path(&path)?;
        println!("\n=== {rel} ===");

        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            // Top-level sheets only for the pairing question.
            if sheet.path.matches('/').count() != 1 {
                continue;
            }
            let bytes = raw.data.as_slice();
            let texts = decode_igtextboxes(bytes);
            let styles = decode_jstyle_overrides(bytes);
            if texts.is_empty() || styles.is_empty() {
                continue;
            }
            println!(
                "  {}: {} igTextBox, {} JStyleOverride",
                sheet.path,
                texts.len(),
                styles.len()
            );

            let mut paired = 0usize;
            for (sidx, style) in styles.iter().enumerate() {
                let plausible = |v: f64| v.is_finite() && (-0.2..=1.4).contains(&v);
                // Hypothesis A: u32 pairs (a,b) / (c,d) are f64 halves.
                let pa = (
                    f64_from_u32_pair(style.field_a_u32, style.field_b_u32),
                    f64_from_u32_pair(style.field_c_u32, style.field_d_u32),
                );
                // Hypothesis B: already-decoded f64 fields 3/4.
                let pb = (style.field_3_f64, style.field_4_f64);
                let candidates: [(&str, (f64, f64)); 2] = [("ab/cd", pa), ("f3/f4", pb)];
                let mut best: Option<(&str, f64, usize, (f64, f64))> = None;
                for (label, p) in candidates {
                    if !(plausible(p.0) && plausible(p.1)) {
                        continue;
                    }
                    for (tidx, text) in texts.iter().enumerate() {
                        let dx = text.trailing_double_1 - p.0;
                        let dy = text.trailing_double_2 - p.1;
                        let d = dx.hypot(dy);
                        if best.is_none_or(|(_, bd, _, _)| d < bd) {
                            best = Some((label, d, tidx, p));
                        }
                    }
                }
                if let Some((label, d, tidx, p)) = best {
                    paired += 1;
                    all_min_dists.push(d);
                    let close = d < 0.02;
                    if close || style.field_2_f64.abs() > 1e-9 {
                        let line = format!(
                            "    style[{sidx}] {label} anchor=({:.4}, {:.4}) rot={:.4} f1={:.4} -> \
                             nearest text[{tidx}] {:?} ins=({:.4}, {:.4}) dist={:.4}{}",
                            p.0,
                            p.1,
                            style.field_2_f64,
                            style.field_1_f64,
                            texts[tidx].text.chars().take(14).collect::<String>(),
                            texts[tidx].trailing_double_1,
                            texts[tidx].trailing_double_2,
                            d,
                            if close { "  <= CLOSE" } else { "" },
                        );
                        println!("{line}");
                        if style.field_2_f64.abs() > 1e-9 {
                            rotated_report.push(line);
                        }
                    }
                } else {
                    println!(
                        "    style[{sidx}] no page-domain anchor (ab/cd=({:.3},{:.3}) f3/f4=({:.3},{:.3}))",
                        pa.0, pa.1, pb.0, pb.1
                    );
                }

                // Height-domain scan inside the attribute tail.
                let tail = &style.raw_attribute_tail;
                let mut height_hits: Vec<String> = Vec::new();
                for at in 0..tail.len().saturating_sub(7) {
                    let v = f64::from_le_bytes(tail[at..at + 8].try_into().unwrap());
                    if v.is_finite() && (0.0005..0.02).contains(&v) {
                        height_hits.push(format!("f64@{at}={v:.6}"));
                    }
                }
                if !height_hits.is_empty() && sidx < 6 {
                    println!(
                        "      tail({}B) height-domain: {}",
                        tail.len(),
                        height_hits.join(" ")
                    );
                }
            }
            println!("    paired {paired}/{} styles", styles.len());

            // Explicit per-text trailer sizes.
            let mut explicit = 0usize;
            for text in &texts {
                let start = text.byte_range.start + 6;
                let payload = &bytes[start..start + text.bytes_to_follow as usize];
                let text_end = 32 + usize::from(text.text_length) * 2;
                let trailer_at = text_end + 24;
                if payload.len() < trailer_at + 8 {
                    continue;
                }
                let h = f32::from_le_bytes(payload[trailer_at..trailer_at + 4].try_into()?);
                let w = f32::from_le_bytes(payload[trailer_at + 4..trailer_at + 8].try_into()?);
                if h.is_finite()
                    && w.is_finite()
                    && (0.0005..0.02).contains(&f64::from(h))
                    && w > 0.0
                {
                    explicit += 1;
                }
            }
            println!("    explicit trailer sizes: {explicit}/{}", texts.len());
        }
    }

    all_min_dists.sort_by(f64::total_cmp);
    println!("\n=== nearest-distance distribution (style anchor -> text insertion) ===");
    println!("  n={}", all_min_dists.len());
    if !all_min_dists.is_empty() {
        for q in [0.0, 0.25, 0.5, 0.75, 0.9, 1.0] {
            let idx = ((all_min_dists.len() - 1) as f64 * q) as usize;
            println!("  q{:>3}: {:.5}", (q * 100.0) as u32, all_min_dists[idx]);
        }
    }
    println!("\n=== rotated styles ({}) ===", rotated_report.len());
    for line in rotated_report.iter().take(30) {
        println!("{line}");
    }
    Ok(())
}
