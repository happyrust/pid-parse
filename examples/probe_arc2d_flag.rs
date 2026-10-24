//! Does an `igArc2d` record's trailing byte say which way the arc runs?
//!
//! `0x007E` `igEllipticalArc2d` answers for itself: its last byte is `0` for
//! the clockwise traverse and `1` for the counter-clockwise one, measured on
//! A01's four vessel heads and the diaphragm actuator's dome
//! (`docs/analysis/2026-10-20-the-actuator-dome-is-on-the-wrong-side.md`).
//! `0x0061` `igArc2d` has the same payload shape with the same trailing byte,
//! but that byte has stayed "carried for audit".
//!
//! This probe dumps every arc record in a `.pid` -- sheet streams and nested
//! symbol caches alike -- with the byte's value next to the two angles, so the
//! census can be read against the reference ink.
//!
//! ```text
//! cargo run --example probe_arc2d_flag -- "<drawing.pid>" [--all]
//! ```
//!
//! Read-only.

use std::collections::BTreeMap;
use std::path::PathBuf;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const CHAIN_START: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
const ARC_PAYLOAD_LEN: usize = 59;
const CURVE_GEOMETRY_AT: usize = 18;
const TYPE_ARC: u16 = 0x0061;
const COORDINATE_LIMIT: f64 = 1e6;
const TAU: f64 = std::f64::consts::TAU;

struct Arc {
    at: usize,
    oid: u32,
    sheet_layer_ref: u32,
    index: u32,
    center: (f64, f64),
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    flag: u8,
}

fn u16_le(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map(u16::from_le_bytes)
}

fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}

fn f64_le(b: &[u8], at: usize) -> Option<f64> {
    b.get(at..at + 8)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .map(f64::from_le_bytes)
}

/// The minor sweep the writer draws today: clockwise from `start` to `end`.
fn minor_sweep_deg(start: f64, end: f64) -> f64 {
    let cw = (start - end).rem_euclid(TAU);
    if cw > std::f64::consts::PI + 1e-9 {
        -(TAU - cw).to_degrees()
    } else {
        -cw.to_degrees()
    }
}

/// Walk the `PSMcluster0` record chain that holds the arc at `arc_at`, print
/// every record it carries, and return how many the walk accounted for. The
/// body is what the direction bit has to make sense of: a wave symbol's two
/// semicircles sit at mirrored centres with the same two angles, so only the
/// flag can tell the S from a pair of C's.
fn dump_body(bytes: &[u8], arc_at: usize) {
    let Some(start) = (0..arc_at.min(bytes.len()))
        .rev()
        .find(|at| at + 4 <= bytes.len() && u32_le(bytes, *at) == Some(CLUSTER_MAGIC))
    else {
        println!("no cluster magic before byte {arc_at}");
        return;
    };
    println!("body stream at {start}.. (arc at {arc_at})");
    let mut at = start + CHAIN_START;
    let mut walked = 0usize;
    while at + PSM_ENVELOPE_LEN <= bytes.len() {
        let (Some(raw), Some(length)) = (u16_le(bytes, at), u32_le(bytes, at + 2)) else {
            break;
        };
        if raw == 0 {
            break;
        }
        let body = at + PSM_ENVELOPE_LEN;
        let Some(end) = body.checked_add(length as usize) else {
            break;
        };
        let Some(payload) = bytes.get(body..end) else {
            break;
        };
        let type_code = raw & 0x0FFF;
        let shape = match type_code {
            0x0018 if payload.len() >= 50 => format!(
                "line     ({:>9.5},{:>9.5}) -> ({:>9.5},{:>9.5})",
                f64_le(payload, 18).unwrap_or(f64::NAN),
                f64_le(payload, 26).unwrap_or(f64::NAN),
                f64_le(payload, 34).unwrap_or(f64::NAN),
                f64_le(payload, 42).unwrap_or(f64::NAN)
            ),
            0x0059 if payload.len() >= 42 => format!(
                "circle   ({:>9.5},{:>9.5}) r {:>9.5}  flag {}",
                f64_le(payload, 18).unwrap_or(f64::NAN),
                f64_le(payload, 26).unwrap_or(f64::NAN),
                f64_le(payload, 34).unwrap_or(f64::NAN),
                payload.get(42).copied().unwrap_or(0)
            ),
            0x0061 if payload.len() >= ARC_PAYLOAD_LEN => format!(
                "arc      ({:>9.5},{:>9.5}) r {:>9.5}  {:>8.2} -> {:>8.2} deg  cw {:>7.2}  flag {}",
                f64_le(payload, 18).unwrap_or(f64::NAN),
                f64_le(payload, 26).unwrap_or(f64::NAN),
                f64_le(payload, 34).unwrap_or(f64::NAN),
                f64_le(payload, 42).unwrap_or(f64::NAN).to_degrees(),
                f64_le(payload, 50).unwrap_or(f64::NAN).to_degrees(),
                (f64_le(payload, 42).unwrap_or(0.0) - f64_le(payload, 50).unwrap_or(0.0))
                    .rem_euclid(TAU)
                    .to_degrees(),
                payload[ARC_PAYLOAD_LEN - 1]
            ),
            0x0084 => format!("polyline ({} bytes)", payload.len()),
            0x004D => format!("text     ({} bytes)", payload.len()),
            other => format!("type 0x{other:04X} ({length} bytes)"),
        };
        if at.abs_diff(arc_at) <= 2_000 {
            println!("  @{at:<8} {shape}");
        }
        walked += 1;
        at = end;
    }
    println!("  walk ended at {at} after {walked} records");
}

fn scan(bytes: &[u8]) -> Vec<Arc> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + PSM_ENVELOPE_LEN + ARC_PAYLOAD_LEN <= bytes.len() {
        let raw = u16_le(bytes, at).unwrap_or(0);
        if raw & 0x0FFF == TYPE_ARC && u32_le(bytes, at + 2) == Some(ARC_PAYLOAD_LEN as u32) {
            let body = at + PSM_ENVELOPE_LEN;
            let payload = &bytes[body..body + ARC_PAYLOAD_LEN];
            let doubles: Option<[f64; 5]> = (0..5)
                .map(|slot| f64_le(payload, CURVE_GEOMETRY_AT + slot * 8))
                .collect::<Option<Vec<f64>>>()
                .map(|v| [v[0], v[1], v[2], v[3], v[4]]);
            if let (
                Some([cx, cy, radius, start_angle, end_angle]),
                Some(oid),
                Some(layer),
                Some(index),
            ) = (
                doubles,
                u32_le(payload, 0),
                u32_le(payload, 8),
                u32_le(payload, 14),
            ) {
                let in_domain = [cx, cy, radius, start_angle, end_angle]
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= COORDINATE_LIMIT);
                if in_domain && radius > 0.0 {
                    out.push(Arc {
                        at,
                        oid,
                        sheet_layer_ref: layer,
                        index,
                        center: (cx, cy),
                        radius,
                        start_angle,
                        end_angle,
                        flag: payload[ARC_PAYLOAD_LEN - 1],
                    });
                }
            }
        }
        at += 1;
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: <drawing.pid> [--all]");
    let mode = args.next();
    let all = mode.as_deref() == Some("--all");
    let body_at: Option<usize> = mode
        .as_deref()
        .and_then(|flag| flag.strip_prefix("--body="))
        .and_then(|value| value.parse().ok());
    let bytes = std::fs::read(PathBuf::from(&path))?;
    let arcs = scan(&bytes);
    println!("{} records in {path}", arcs.len());

    let mut by_flag: BTreeMap<u8, usize> = BTreeMap::new();
    for arc in &arcs {
        *by_flag.entry(arc.flag).or_default() += 1;
    }
    println!("flag census: {by_flag:?}");

    // The rule under test: `flag == 0` draws clockwise from start to end,
    // `flag == 1` counter-clockwise. Both sweeps reach the same end; they are
    // complements. The minor-sweep column is what the writer draws today.
    let mut crosstab: BTreeMap<(u8, &str), usize> = BTreeMap::new();
    for arc in &arcs {
        let cw = (arc.start_angle - arc.end_angle)
            .rem_euclid(TAU)
            .to_degrees();
        let bucket = if cw < 89.5 {
            "cw<90"
        } else if cw < 179.5 {
            "cw90-180"
        } else if cw < 180.5 {
            "cw=180"
        } else {
            "cw>180"
        };
        *crosstab.entry((arc.flag, bucket)).or_default() += 1;
    }
    println!("flag x clockwise sweep: {crosstab:?}");

    if let Some(arc_at) = body_at {
        dump_body(&bytes, arc_at);
    }
    if all {
        for arc in &arcs {
            let minor = minor_sweep_deg(arc.start_angle, arc.end_angle);
            let cw = (arc.start_angle - arc.end_angle)
                .rem_euclid(TAU)
                .to_degrees();
            println!(
                "  @{:<8} oid {:<6} layer {:<7} index {:<5} c ({:>10.4},{:>10.4}) r {:>9.6} \
                 {:>8.2} -> {:>8.2} deg  cw {cw:>7.2} minor {minor:>8.2}  flag {}",
                arc.at,
                arc.oid,
                arc.sheet_layer_ref,
                arc.index,
                arc.center.0,
                arc.center.1,
                arc.radius,
                arc.start_angle.to_degrees(),
                arc.end_angle.to_degrees(),
                arc.flag
            );
        }
    }
    Ok(())
}
