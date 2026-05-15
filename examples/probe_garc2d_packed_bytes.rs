//! Probe every PSM type `0x0030` (`GArc2d` candidate) record across
//! all `Sheet*` streams of the registry fixtures and dump the full
//! 64-byte payload, classifying every potential field both as `f64`
//! and as packed integers.
//!
//! Phase 14 §6.1 future-slice — the current `decode_primitive_arcs`
//! assumes the payload is `8 × f64` (center.xy, axis_a.xy,
//! axis_ratio, padded sweep_direction byte, sweep_start_angle,
//! sweep_end_angle). Earlier hand-pick probes (only 5 hardcoded
//! offsets in `probe_garc2d_bytes.rs`) suggested bytes 32..63 are
//! packed integers, not pure f64s, and that `axis_a.y` is actually
//! a rotation angle rather than a vector component. This probe
//! gathers cross-fixture statistics over **every** 0x0030 hit so
//! we can decide whether to rename DTO fields and / or relax the
//! `axis_a.y ≈ 0` validation gate.
//!
//! The probe applies **no** field-level filter beyond the PSM
//! header invariants (14-bit type code == 0x0030, plausible
//! `bytes_to_follow` ≥ 64 fitting the stream). All field
//! classification statistics are reported per fixture per
//! `Sheet*` stream.
//!
//! Run with:
//! ```bash
//! cargo run --release --example probe_garc2d_packed_bytes
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

/// Cross-reference known geometry / metadata PSM type codes that the
/// 0x0030 record family is observed to reference in `+32..33`.
const KNOWN_REFERENCED_TYPES: &[(u16, &str)] = &[
    (0x0010, "0x0010 sub-record"),
    (0x0013, "0x0013 unknown"),
    (0x0018, "igLine2d"),
    (0x001E, "0x001E unknown"),
    (0x0022, "0x0022 unknown"),
    (0x0030, "0x0030 j2d-self-ref"),
    (0x004D, "igTextBox"),
    (0x005E, "igPoint2d"),
    (0x0084, "igLineString2d"),
    (0x00AC, "0x00AC unknown"),
    (0x00CE, "igSymbol2d"),
    (0x00FA, "GraphicGroup"),
    (0x3FE6, "GLine2d"),
];

/// PSM record header is 18 bytes: `type(2) + bytes_to_follow(4) +
/// oid(4) + aux(8)`. The 64-byte `GArc2d` inner payload begins at
/// `offset + 18`. The `bytes_to_follow` field counts every byte
/// *after* the 6-byte `type + btf` prefix, so it includes `oid +
/// aux + payload + any optional attribute tail`.
const PSM_RECORD_HEADER_LEN: usize = 18;
const PSM_TYPE_CODE_GARC2D: u16 = 0x0030;
const GARC2D_PAYLOAD_LEN: usize = 64;
/// `bytes_to_follow` minimum that can still hold the `oid(4) +
/// aux(8) + GArc2d payload(64)` = 76 bytes after the
/// `type + bytes_to_follow` prefix.
const GARC2D_MIN_BYTES_TO_FOLLOW: u32 = 76;

#[derive(Debug, Clone)]
struct Garc2dCandidate {
    offset: usize,
    type_flags: u16,
    bytes_to_follow: u32,
    oid: u32,
    aux: [u8; 8],
    payload: [u8; GARC2D_PAYLOAD_LEN],
    tail: Vec<u8>,
}

#[derive(Debug, Default)]
struct FieldDoubleStats {
    finite_count: usize,
    non_finite_count: usize,
    abs_in_unit_count: usize,
    abs_in_kilo_count: usize,
    abs_above_kilo_count: usize,
    abs_zero_count: usize,
    denormalized_count: usize,
    near_pi_half_count: usize,
    near_pi_count: usize,
    near_three_pi_half_count: usize,
    near_two_pi_count: usize,
    near_zero_count: usize,
    other_count: usize,
    histogram_buckets: BTreeMap<i64, usize>,
}

#[derive(Debug, Default)]
struct PerStreamStats {
    hits: usize,
    bytes_to_follow_dist: BTreeMap<u32, usize>,
    byte_40_value_dist: BTreeMap<u8, usize>,
    byte_41_47_all_zero_count: usize,
    byte_41_47_any_nonzero_count: usize,
    field_stats: BTreeMap<usize, FieldDoubleStats>,
    packed_u16_quad_at_32: BTreeMap<(u16, u16, u16, u16), usize>,
    packed_u32_pair_at_32: BTreeMap<(u32, u32), usize>,
    packed_u16_quad_at_48: BTreeMap<(u16, u16, u16, u16), usize>,
    packed_u32_pair_at_48: BTreeMap<(u32, u32), usize>,
    packed_u16_quad_at_56: BTreeMap<(u16, u16, u16, u16), usize>,
    packed_u32_pair_at_56: BTreeMap<(u32, u32), usize>,
    axis_a_y_zero_count: usize,
    axis_a_y_pi_half_count: usize,
    axis_a_y_pi_count: usize,
    axis_a_y_three_pi_half_count: usize,
    axis_a_y_other_finite_count: usize,
    axis_ratio_in_unit_range_count: usize,
    axis_ratio_denormalized_count: usize,
    axis_ratio_other_count: usize,
}

fn double_at(payload: &[u8; GARC2D_PAYLOAD_LEN], pos: usize) -> f64 {
    let bytes: [u8; 8] = payload[pos..pos + 8].try_into().expect("8 bytes");
    f64::from_le_bytes(bytes)
}

fn u64_at(payload: &[u8; GARC2D_PAYLOAD_LEN], pos: usize) -> u64 {
    let bytes: [u8; 8] = payload[pos..pos + 8].try_into().expect("8 bytes");
    u64::from_le_bytes(bytes)
}

fn u32_at(payload: &[u8; GARC2D_PAYLOAD_LEN], pos: usize) -> u32 {
    let bytes: [u8; 4] = payload[pos..pos + 4].try_into().expect("4 bytes");
    u32::from_le_bytes(bytes)
}

fn u16_at(payload: &[u8; GARC2D_PAYLOAD_LEN], pos: usize) -> u16 {
    let bytes: [u8; 2] = payload[pos..pos + 2].try_into().expect("2 bytes");
    u16::from_le_bytes(bytes)
}

fn classify_double(stats: &mut FieldDoubleStats, value: f64) {
    if !value.is_finite() {
        stats.non_finite_count += 1;
        stats.other_count += 1;
        return;
    }
    stats.finite_count += 1;
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7FF) as i32;
    let abs = value.abs();
    if abs == 0.0 {
        stats.abs_zero_count += 1;
    } else if exponent == 0 {
        stats.denormalized_count += 1;
    } else if abs <= 1.0 {
        stats.abs_in_unit_count += 1;
    } else if abs <= 1_000.0 {
        stats.abs_in_kilo_count += 1;
    } else {
        stats.abs_above_kilo_count += 1;
    }
    let pi = std::f64::consts::PI;
    let tol = 1e-6;
    if abs < tol {
        stats.near_zero_count += 1;
    } else if (value - pi / 2.0).abs() < tol || (value + pi / 2.0).abs() < tol {
        stats.near_pi_half_count += 1;
    } else if (value - pi).abs() < tol || (value + pi).abs() < tol {
        stats.near_pi_count += 1;
    } else if (value - 1.5 * pi).abs() < tol || (value + 1.5 * pi).abs() < tol {
        stats.near_three_pi_half_count += 1;
    } else if (value - 2.0 * pi).abs() < tol {
        stats.near_two_pi_count += 1;
    } else {
        stats.other_count += 1;
    }
    if abs > 0.0 && exponent != 0 {
        let bucket = (value.abs().log10() * 4.0).round() as i64;
        *stats.histogram_buckets.entry(bucket).or_insert(0) += 1;
    }
}

fn probe_garc2d_candidates(data: &[u8]) -> Vec<Garc2dCandidate> {
    let mut out = Vec::new();
    if data.len() < PSM_RECORD_HEADER_LEN + GARC2D_PAYLOAD_LEN {
        return out;
    }
    let max_offset = data.len() - (PSM_RECORD_HEADER_LEN + GARC2D_PAYLOAD_LEN);
    let mut off = 0usize;
    while off <= max_offset {
        let type_word = u16::from_le_bytes([data[off], data[off + 1]]);
        let type_code = type_word & 0x3FFF;
        if type_code != PSM_TYPE_CODE_GARC2D {
            off += 1;
            continue;
        }
        let type_flags = type_word >> 14;
        let bytes_to_follow =
            u32::from_le_bytes([data[off + 2], data[off + 3], data[off + 4], data[off + 5]]);
        if !(GARC2D_MIN_BYTES_TO_FOLLOW..=100_000).contains(&bytes_to_follow) {
            off += 1;
            continue;
        }
        let record_end = match off
            .checked_add(6)
            .and_then(|after_header| after_header.checked_add(bytes_to_follow as usize))
        {
            Some(end) => end,
            None => {
                off += 1;
                continue;
            }
        };
        if record_end > data.len() {
            off += 1;
            continue;
        }
        let oid = u32::from_le_bytes([data[off + 6], data[off + 7], data[off + 8], data[off + 9]]);
        let mut aux = [0u8; 8];
        aux.copy_from_slice(&data[off + 10..off + 18]);
        let payload_start = off + PSM_RECORD_HEADER_LEN;
        let payload_end = payload_start + GARC2D_PAYLOAD_LEN;
        let mut payload = [0u8; GARC2D_PAYLOAD_LEN];
        payload.copy_from_slice(&data[payload_start..payload_end]);
        let tail = data[payload_end..record_end].to_vec();
        out.push(Garc2dCandidate {
            offset: off,
            type_flags,
            bytes_to_follow,
            oid,
            aux,
            payload,
            tail,
        });
        off = record_end.max(off + 1);
    }
    out
}

fn collect_stream_stats(candidates: &[Garc2dCandidate]) -> PerStreamStats {
    let mut stats = PerStreamStats::default();
    for cand in candidates {
        stats.hits += 1;
        *stats
            .bytes_to_follow_dist
            .entry(cand.bytes_to_follow)
            .or_insert(0) += 1;

        let payload = &cand.payload;
        *stats.byte_40_value_dist.entry(payload[40]).or_insert(0) += 1;
        let pad_zero = payload[41..48].iter().all(|b| *b == 0);
        if pad_zero {
            stats.byte_41_47_all_zero_count += 1;
        } else {
            stats.byte_41_47_any_nonzero_count += 1;
        }

        for &pos in &[0usize, 8, 16, 24, 32, 48, 56] {
            let entry = stats.field_stats.entry(pos).or_default();
            classify_double(entry, double_at(payload, pos));
        }

        let axis_a_y = double_at(payload, 24);
        let pi = std::f64::consts::PI;
        let tol = 1e-6;
        if axis_a_y.abs() < tol {
            stats.axis_a_y_zero_count += 1;
        } else if (axis_a_y - pi / 2.0).abs() < tol {
            stats.axis_a_y_pi_half_count += 1;
        } else if (axis_a_y - pi).abs() < tol {
            stats.axis_a_y_pi_count += 1;
        } else if (axis_a_y - 1.5 * pi).abs() < tol {
            stats.axis_a_y_three_pi_half_count += 1;
        } else if axis_a_y.is_finite() {
            stats.axis_a_y_other_finite_count += 1;
        }

        let axis_ratio = double_at(payload, 32);
        if axis_ratio.is_finite() {
            let bits = axis_ratio.to_bits();
            let exponent = ((bits >> 52) & 0x7FF) as i32;
            if exponent == 0 && axis_ratio != 0.0 {
                stats.axis_ratio_denormalized_count += 1;
            } else if (0.0..=1.0 + 1e-6).contains(&axis_ratio) {
                stats.axis_ratio_in_unit_range_count += 1;
            } else {
                stats.axis_ratio_other_count += 1;
            }
        } else {
            stats.axis_ratio_other_count += 1;
        }

        let q32 = (
            u16_at(payload, 32),
            u16_at(payload, 34),
            u16_at(payload, 36),
            u16_at(payload, 38),
        );
        *stats.packed_u16_quad_at_32.entry(q32).or_insert(0) += 1;
        *stats
            .packed_u32_pair_at_32
            .entry((u32_at(payload, 32), u32_at(payload, 36)))
            .or_insert(0) += 1;

        let q48 = (
            u16_at(payload, 48),
            u16_at(payload, 50),
            u16_at(payload, 52),
            u16_at(payload, 54),
        );
        *stats.packed_u16_quad_at_48.entry(q48).or_insert(0) += 1;
        *stats
            .packed_u32_pair_at_48
            .entry((u32_at(payload, 48), u32_at(payload, 52)))
            .or_insert(0) += 1;

        let q56 = (
            u16_at(payload, 56),
            u16_at(payload, 58),
            u16_at(payload, 60),
            u16_at(payload, 62),
        );
        *stats.packed_u16_quad_at_56.entry(q56).or_insert(0) += 1;
        *stats
            .packed_u32_pair_at_56
            .entry((u32_at(payload, 56), u32_at(payload, 60)))
            .or_insert(0) += 1;
    }
    stats
}

fn print_field_stats(field: &str, stats: &FieldDoubleStats) {
    println!(
        "    {field}: finite={} non_finite={} abs<=1={} abs<=1k={} abs>1k={} zero={} denormalized={} near_zero={} pi/2={} pi={} 3pi/2={} 2pi={} other_finite={}",
        stats.finite_count,
        stats.non_finite_count,
        stats.abs_in_unit_count,
        stats.abs_in_kilo_count,
        stats.abs_above_kilo_count,
        stats.abs_zero_count,
        stats.denormalized_count,
        stats.near_zero_count,
        stats.near_pi_half_count,
        stats.near_pi_count,
        stats.near_three_pi_half_count,
        stats.near_two_pi_count,
        stats.other_count
    );
    if !stats.histogram_buckets.is_empty() {
        let mut buckets: Vec<_> = stats.histogram_buckets.iter().collect();
        buckets.sort_by_key(|(bucket, _)| **bucket);
        let summary = buckets
            .iter()
            .map(|(bucket, count)| {
                let lo = 10f64.powf(**bucket as f64 / 4.0);
                let hi = 10f64.powf((**bucket + 1) as f64 / 4.0);
                format!("[{:.2e}..{:.2e}):{}", lo, hi, count)
            })
            .collect::<Vec<_>>()
            .join(" ");
        println!("        |x|-buckets (log10/4): {summary}");
    }
}

fn print_byte_dist(label: &str, dist: &BTreeMap<u8, usize>) {
    let mut sorted: Vec<_> = dist.iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    let summary = sorted
        .iter()
        .take(8)
        .map(|(value, count)| format!("0x{value:02X}:{count}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("    {label}: {summary}");
}

fn print_u32_dist(label: &str, dist: &BTreeMap<u32, usize>, limit: usize) {
    let mut sorted: Vec<_> = dist.iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    let summary = sorted
        .iter()
        .take(limit)
        .map(|(value, count)| format!("{value}:{count}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("    {label}: {summary}");
}

fn print_packed_u16_quad(label: &str, dist: &BTreeMap<(u16, u16, u16, u16), usize>, limit: usize) {
    let mut sorted: Vec<_> = dist.iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    println!("    {label} (top {limit} by count):");
    for ((a, b, c, d), count) in sorted.iter().take(limit) {
        println!("      [0x{a:04X} 0x{b:04X} 0x{c:04X} 0x{d:04X}]: {count}");
    }
}

fn print_packed_u32_pair(label: &str, dist: &BTreeMap<(u32, u32), usize>, limit: usize) {
    let mut sorted: Vec<_> = dist.iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    println!("    {label} (top {limit} by count):");
    for ((a, b), count) in sorted.iter().take(limit) {
        println!("      [0x{a:08X} 0x{b:08X}]: {count}");
    }
}

fn report_axis_a_x_vs_center_x(candidates: &[Garc2dCandidate]) {
    let mut exact_byte_eq = 0usize;
    let mut close_1e_9 = 0usize;
    let mut close_1e_6 = 0usize;
    let mut close_1e_3 = 0usize;
    let mut farther = 0usize;
    let mut sample_diffs: Vec<(u32, f64, f64, f64)> = Vec::new();
    for cand in candidates {
        let payload = &cand.payload;
        let byte_eq = payload[0..8] == payload[16..24];
        let cx = double_at(payload, 0);
        let ax = double_at(payload, 16);
        let diff = (ax - cx).abs();
        if byte_eq {
            exact_byte_eq += 1;
        }
        if diff < 1e-9 {
            close_1e_9 += 1;
        } else if diff < 1e-6 {
            close_1e_6 += 1;
        } else if diff < 1e-3 {
            close_1e_3 += 1;
        } else {
            farther += 1;
            if sample_diffs.len() < 6 {
                sample_diffs.push((cand.oid, cx, ax, diff));
            }
        }
    }
    println!(
        "    +0..7 == +16..23 byte_eq={} <1e-9={} <1e-6={} <1e-3={} >=1e-3={}",
        exact_byte_eq, close_1e_9, close_1e_6, close_1e_3, farther
    );
    for (oid, cx, ax, diff) in &sample_diffs {
        println!("      oid={oid} center.x={cx:+.6} +16..23={ax:+.6} diff={diff:.6e}");
    }
}

fn report_referenced_type_buckets(candidates: &[Garc2dCandidate]) {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<u16, Vec<&Garc2dCandidate>> = BTreeMap::new();
    for cand in candidates {
        let ref_type = u16_at(&cand.payload, 32);
        buckets.entry(ref_type).or_default().push(cand);
    }
    println!(
        "    +32..33 referenced_type buckets ({} distinct):",
        buckets.len()
    );
    let mut sorted: Vec<_> = buckets.iter().collect();
    sorted.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for (ref_type, bucket) in sorted.iter().take(10) {
        let mut subkind_dist: BTreeMap<u16, usize> = BTreeMap::new();
        let mut field_38_dist: BTreeMap<u16, usize> = BTreeMap::new();
        let mut field_42_dist: BTreeMap<u16, usize> = BTreeMap::new();
        let mut field_44_dist: BTreeMap<u16, usize> = BTreeMap::new();
        let mut field_46_dist: BTreeMap<u16, usize> = BTreeMap::new();
        let mut field_48_u32_dist: BTreeMap<u32, usize> = BTreeMap::new();
        let mut field_52_u32_dist: BTreeMap<u32, usize> = BTreeMap::new();
        let mut field_56_u32_dist: BTreeMap<u32, usize> = BTreeMap::new();
        let mut field_60_u32_dist: BTreeMap<u32, usize> = BTreeMap::new();
        for cand in *bucket {
            let p = &cand.payload;
            *subkind_dist.entry(u16_at(p, 34)).or_insert(0) += 1;
            *field_38_dist.entry(u16_at(p, 38)).or_insert(0) += 1;
            *field_42_dist.entry(u16_at(p, 42)).or_insert(0) += 1;
            *field_44_dist.entry(u16_at(p, 44)).or_insert(0) += 1;
            *field_46_dist.entry(u16_at(p, 46)).or_insert(0) += 1;
            *field_48_u32_dist.entry(u32_at(p, 48)).or_insert(0) += 1;
            *field_52_u32_dist.entry(u32_at(p, 52)).or_insert(0) += 1;
            *field_56_u32_dist.entry(u32_at(p, 56)).or_insert(0) += 1;
            *field_60_u32_dist.entry(u32_at(p, 60)).or_insert(0) += 1;
        }
        println!(
            "      ref_type=0x{:04X} count={} sub@+34 distinct={} field@+38 distinct={} field@+42 distinct={} field@+48u32 distinct={}",
            ref_type,
            bucket.len(),
            subkind_dist.len(),
            field_38_dist.len(),
            field_42_dist.len(),
            field_48_u32_dist.len()
        );
        if subkind_dist.len() <= 6 {
            let summary = subkind_dist
                .iter()
                .map(|(value, count)| format!("0x{value:04X}:{count}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!("        +34 sub : {summary}");
        }
        if field_42_dist.len() <= 6 {
            let summary = field_42_dist
                .iter()
                .map(|(value, count)| format!("0x{value:04X}:{count}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!("        +42 u16 : {summary}");
        }
        if field_44_dist.len() <= 6 {
            let summary = field_44_dist
                .iter()
                .map(|(value, count)| format!("0x{value:04X}:{count}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!("        +44 u16 : {summary}");
        }
        if field_46_dist.len() <= 6 {
            let summary = field_46_dist
                .iter()
                .map(|(value, count)| format!("0x{value:04X}:{count}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!("        +46 u16 : {summary}");
        }
    }
}

fn report_attribute_tail_sweep_angles(candidates: &[Garc2dCandidate]) {
    use std::collections::BTreeMap;
    let mut total_f64_slots = 0usize;
    let mut finite_in_two_pi_nonzero = 0usize;
    let mut finite_denormalized_or_zero = 0usize;
    let mut finite_other = 0usize;
    let mut nan_inf = 0usize;
    let mut near_known_angle_hits: Vec<(u32, usize, f64, &'static str)> = Vec::new();
    let mut interesting_offsets: BTreeMap<usize, usize> = BTreeMap::new();
    let mut one_dot_zero_offsets: BTreeMap<usize, usize> = BTreeMap::new();
    let two_pi = 2.0 * std::f64::consts::PI;
    let known_angles: &[(f64, &str)] = &[
        (std::f64::consts::FRAC_PI_4, "pi/4"),
        (std::f64::consts::FRAC_PI_3, "pi/3"),
        (std::f64::consts::FRAC_PI_2, "pi/2"),
        (2.0 * std::f64::consts::FRAC_PI_3, "2pi/3"),
        (3.0 * std::f64::consts::FRAC_PI_4, "3pi/4"),
        (std::f64::consts::PI, "pi"),
        (5.0 * std::f64::consts::FRAC_PI_4, "5pi/4"),
        (4.0 * std::f64::consts::FRAC_PI_3, "4pi/3"),
        (3.0 * std::f64::consts::FRAC_PI_2, "3pi/2"),
        (5.0 * std::f64::consts::FRAC_PI_3, "5pi/3"),
        (7.0 * std::f64::consts::FRAC_PI_4, "7pi/4"),
        (two_pi, "2pi"),
        (1.0, "1.0"),
    ];
    for cand in candidates {
        let tail = &cand.tail;
        if tail.len() < 8 {
            continue;
        }
        let max_off = tail.len() - 8;
        let mut off = 0usize;
        while off <= max_off {
            total_f64_slots += 1;
            let chunk: [u8; 8] = tail[off..off + 8].try_into().expect("8 bytes");
            let value = f64::from_le_bytes(chunk);
            if !value.is_finite() {
                nan_inf += 1;
                off += 8;
                continue;
            }
            let abs = value.abs();
            let bits = value.to_bits();
            let exponent = ((bits >> 52) & 0x7FF) as i32;
            let denormalized_or_zero = exponent == 0;
            if denormalized_or_zero {
                finite_denormalized_or_zero += 1;
            } else if abs <= two_pi + 1e-6 {
                finite_in_two_pi_nonzero += 1;
                *interesting_offsets.entry(off).or_insert(0) += 1;
                if (value - 1.0).abs() < 1e-12 {
                    *one_dot_zero_offsets.entry(off).or_insert(0) += 1;
                }
                for (known, name) in known_angles {
                    if (value - known).abs() < 1e-6 || (value + known).abs() < 1e-6 {
                        if near_known_angle_hits.len() < 32 {
                            near_known_angle_hits.push((cand.oid, off, value, name));
                        }
                        break;
                    }
                }
            } else {
                finite_other += 1;
            }
            off += 8;
        }
    }
    println!(
        "    attribute tail f64 scan: total_slots={} finite_in_[-2pi,2pi]_normal={} denormalized_or_zero={} other_finite={} nan_inf={}",
        total_f64_slots,
        finite_in_two_pi_nonzero,
        finite_denormalized_or_zero,
        finite_other,
        nan_inf
    );
    if !interesting_offsets.is_empty() {
        let mut sorted: Vec<_> = interesting_offsets.iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
        println!("      tail offsets with normal f64 in [-2pi,2pi] (top 12):");
        for (off, count) in sorted.iter().take(12) {
            println!("        tail+{off:03}: {count} records");
        }
    }
    if !one_dot_zero_offsets.is_empty() {
        let mut sorted: Vec<_> = one_dot_zero_offsets.iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
        println!("      tail offsets where f64 == 1.0 (top 12):");
        for (off, count) in sorted.iter().take(12) {
            println!("        tail+{off:03}: {count} records");
        }
    }
    if !near_known_angle_hits.is_empty() {
        println!(
            "      tail f64 hits near known angles (oid, tail_off, value, name): showing {}",
            near_known_angle_hits.len()
        );
        for (oid, off, value, name) in &near_known_angle_hits {
            println!("        oid={oid} tail+{off:03} value={value:+.6} name={name}");
        }
    }
}

/// Scan the entire `Sheet*` stream for every plausible PSM record and
/// return a map of `oid -> list of PSM type codes that own that oid`.
///
/// Bounds: any header with `bytes_to_follow` in `[8, 100_000]` that
/// fits in the stream is considered a candidate, regardless of whether
/// it is a known geometry type. Multiple owners per oid are kept so we
/// can detect oid reuse across record kinds.
fn build_known_oid_table(data: &[u8]) -> BTreeMap<u32, BTreeSet<u16>> {
    let mut out: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
    if data.len() < 10 {
        return out;
    }
    let max_off = data.len() - 10;
    let mut off = 0usize;
    while off <= max_off {
        let type_word = u16::from_le_bytes([data[off], data[off + 1]]);
        let type_code = type_word & 0x3FFF;
        let bytes_to_follow =
            u32::from_le_bytes([data[off + 2], data[off + 3], data[off + 4], data[off + 5]]);
        if !(8..=100_000).contains(&bytes_to_follow) {
            off += 1;
            continue;
        }
        let after_header = off + 6;
        let Some(record_end) = after_header.checked_add(bytes_to_follow as usize) else {
            off += 1;
            continue;
        };
        if record_end > data.len() {
            off += 1;
            continue;
        }
        let oid = u32::from_le_bytes([data[off + 6], data[off + 7], data[off + 8], data[off + 9]]);
        if oid != 0 && oid < 10_000_000 {
            out.entry(oid).or_default().insert(type_code);
        }
        off += 1;
    }
    out
}

fn report_field_24_by_btf_bucket(candidates: &[Garc2dCandidate]) {
    let mut buckets: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
    for cand in candidates {
        let value = double_at(&cand.payload, 24);
        buckets.entry(cand.bytes_to_follow).or_default().push(value);
    }
    println!("    +24..31 by btf bucket (rotation vs sweep_extent hypothesis):");
    let pi = std::f64::consts::PI;
    let tol = 1e-6;
    for (btf, values) in &buckets {
        let mut zero = 0usize;
        let mut pi_half = 0usize;
        let mut pi_ct = 0usize;
        let mut three_pi_half = 0usize;
        let mut two_pi_ct = 0usize;
        let mut other = 0usize;
        for v in values {
            let a = v.abs();
            if a < tol {
                zero += 1;
            } else if (v - pi / 2.0).abs() < tol {
                pi_half += 1;
            } else if (v - pi).abs() < tol {
                pi_ct += 1;
            } else if (v - 1.5 * pi).abs() < tol {
                three_pi_half += 1;
            } else if (v - 2.0 * pi).abs() < tol {
                two_pi_ct += 1;
            } else {
                other += 1;
            }
        }
        println!(
            "      btf={btf} ({} hits): zero={zero} pi/2={pi_half} pi={pi_ct} 3pi/2={three_pi_half} 2pi={two_pi_ct} other={other}",
            values.len()
        );
    }
}

fn report_btf_bucket_tail_signature(candidates: &[Garc2dCandidate]) {
    let mut buckets: BTreeMap<u32, Vec<&Garc2dCandidate>> = BTreeMap::new();
    for cand in candidates {
        buckets.entry(cand.bytes_to_follow).or_default().push(cand);
    }
    println!("    tail head signature by btf bucket (first 8 bytes of tail):");
    for (btf, group) in &buckets {
        let mut head_dist: BTreeMap<[u8; 8], usize> = BTreeMap::new();
        let mut tag_like_count = 0usize;
        for cand in group {
            if cand.tail.len() < 8 {
                continue;
            }
            let head: [u8; 8] = cand.tail[..8].try_into().expect("8 bytes");
            *head_dist.entry(head).or_insert(0) += 1;
            if cand.tail.len() >= 6 {
                let length_prefix =
                    u32::from_le_bytes([cand.tail[0], cand.tail[1], cand.tail[2], cand.tail[3]]);
                let char_count = u16::from_le_bytes([cand.tail[4], cand.tail[5]]);
                if length_prefix > 0
                    && length_prefix < 10_000
                    && char_count > 0
                    && char_count < 1_000
                    && length_prefix.saturating_sub(u32::from(char_count) * 2) < 20
                {
                    tag_like_count += 1;
                }
            }
        }
        println!(
            "      btf={btf} ({} records): distinct_heads={} plant_tag_like_prefix={}",
            group.len(),
            head_dist.len(),
            tag_like_count
        );
        if head_dist.len() <= 6 {
            for (head, count) in &head_dist {
                let hex: String = head
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("        [{hex}]: {count}");
            }
        }
    }
}

fn report_cross_record_references(
    candidates: &[Garc2dCandidate],
    known_oids: &BTreeMap<u32, BTreeSet<u16>>,
) {
    let mut payload_oid_offsets: BTreeMap<usize, usize> = BTreeMap::new();
    let mut tail_oid_offsets: BTreeMap<usize, usize> = BTreeMap::new();
    let mut type_hit_dist: BTreeMap<u16, usize> = BTreeMap::new();
    let mut record_has_payload_hit = 0usize;
    let mut record_has_tail_hit = 0usize;
    let mut sample_hits: Vec<(u32, usize, &'static str, u32, u16)> = Vec::new();
    for cand in candidates {
        let mut payload_hit = false;
        let mut tail_hit = false;
        for off in (32..GARC2D_PAYLOAD_LEN.saturating_sub(3)).step_by(2) {
            let value = u32_at(&cand.payload, off);
            if value == 0 || value > 10_000_000 {
                continue;
            }
            if let Some(types) = known_oids.get(&value) {
                if !types.contains(&0x0030) || types.len() > 1 {
                    *payload_oid_offsets.entry(off).or_insert(0) += 1;
                    payload_hit = true;
                    if sample_hits.len() < 18 {
                        let label = type_name_for_set(types);
                        sample_hits.push((cand.oid, off, label, value, ref_type_for_set(types)));
                    }
                    for t in types {
                        if *t != 0x0030 {
                            *type_hit_dist.entry(*t).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
        if cand.tail.len() >= 4 {
            let max = cand.tail.len() - 4;
            let mut off = 0usize;
            while off <= max {
                let value = u32::from_le_bytes([
                    cand.tail[off],
                    cand.tail[off + 1],
                    cand.tail[off + 2],
                    cand.tail[off + 3],
                ]);
                if value != 0 && value < 10_000_000 {
                    if let Some(types) = known_oids.get(&value) {
                        if !types.contains(&0x0030) || types.len() > 1 {
                            *tail_oid_offsets.entry(off).or_insert(0) += 1;
                            tail_hit = true;
                            for t in types {
                                if *t != 0x0030 {
                                    *type_hit_dist.entry(*t).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
                off += 2;
            }
        }
        if payload_hit {
            record_has_payload_hit += 1;
        }
        if tail_hit {
            record_has_tail_hit += 1;
        }
    }
    println!(
        "    cross-record oid references: records_with_payload_hit={} records_with_tail_hit={} (of {} candidates)",
        record_has_payload_hit,
        record_has_tail_hit,
        candidates.len()
    );
    if !payload_oid_offsets.is_empty() {
        let mut sorted: Vec<_> = payload_oid_offsets.iter().collect();
        sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        println!("      payload offsets where u32 matches a known foreign-type oid (top 10):");
        for (off, count) in sorted.iter().take(10) {
            println!("        payload+{off:03}: {count} hits");
        }
    }
    if !tail_oid_offsets.is_empty() {
        let mut sorted: Vec<_> = tail_oid_offsets.iter().collect();
        sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        println!("      tail offsets where u32 matches a known foreign-type oid (top 10):");
        for (off, count) in sorted.iter().take(10) {
            println!("        tail+{off:03}: {count} hits");
        }
    }
    if !type_hit_dist.is_empty() {
        let mut sorted: Vec<_> = type_hit_dist.iter().collect();
        sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        println!("      referenced PSM type distribution (top 10):");
        for (t, count) in sorted.iter().take(10) {
            let label = KNOWN_REFERENCED_TYPES
                .iter()
                .find(|(k, _)| *k == **t)
                .map(|(_, n)| *n)
                .unwrap_or("?");
            println!("        0x{t:04X} ({label}): {count}");
        }
    }
    if !sample_hits.is_empty() {
        println!("      sample payload hits (oid, off, referenced_type, ref_oid, ref_type):");
        for (oid, off, label, value, t) in &sample_hits {
            println!(
                "        oid={oid} payload+{off:03} -> {label} ref_oid={value} ref_type=0x{t:04X}"
            );
        }
    }
}

fn type_name_for_set(types: &BTreeSet<u16>) -> &'static str {
    for (t, name) in KNOWN_REFERENCED_TYPES {
        if types.contains(t) {
            return name;
        }
    }
    "?"
}

fn ref_type_for_set(types: &BTreeSet<u16>) -> u16 {
    for (t, _) in KNOWN_REFERENCED_TYPES {
        if types.contains(t) {
            return *t;
        }
    }
    types.iter().next().copied().unwrap_or(0)
}

fn dump_payload(label: &str, candidate: &Garc2dCandidate) {
    println!(
        "    {label} @ 0x{:06X} type_flags={} btf={} oid={} aux=[{}]",
        candidate.offset,
        candidate.type_flags,
        candidate.bytes_to_follow,
        candidate.oid,
        candidate
            .aux
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let payload = &candidate.payload;
    for chunk_start in (0..GARC2D_PAYLOAD_LEN).step_by(16) {
        let chunk_end = (chunk_start + 16).min(GARC2D_PAYLOAD_LEN);
        let raw = &payload[chunk_start..chunk_end];
        let hex: String = raw
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let ascii: String = raw
            .iter()
            .map(|b| {
                if (0x20..0x7F).contains(b) {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("      +{:03}: {:<48} | {}", chunk_start, hex, ascii);
    }
    println!(
        "      doubles: center=({:+.6},{:+.6}) +16=({:+.6},{:+.6}) +32={:+.6e} +48={:+.6e} +56={:+.6e}",
        double_at(payload, 0),
        double_at(payload, 8),
        double_at(payload, 16),
        double_at(payload, 24),
        double_at(payload, 32),
        double_at(payload, 48),
        double_at(payload, 56)
    );
    println!(
        "      raw u64: +00=0x{:016X} +08=0x{:016X} +16=0x{:016X} +24=0x{:016X} +32=0x{:016X} +40=0x{:016X} +48=0x{:016X} +56=0x{:016X}",
        u64_at(payload, 0),
        u64_at(payload, 8),
        u64_at(payload, 16),
        u64_at(payload, 24),
        u64_at(payload, 32),
        u64_at(payload, 40),
        u64_at(payload, 48),
        u64_at(payload, 56)
    );
}

fn probe_fixture(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(path)?)?;
    let sheet_paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().replace('\\', "/"))
        .filter(|p| {
            let name = p.rsplit('/').next().unwrap_or("");
            name.starts_with("Sheet")
        })
        .collect();
    if sheet_paths.is_empty() {
        return Ok(());
    }
    println!("\n=== {} ===", path.display());
    for stream_path in &sheet_paths {
        let mut stream = match cfb.open_stream(stream_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_err() {
            continue;
        }
        let candidates = probe_garc2d_candidates(&bytes);
        if candidates.is_empty() {
            continue;
        }
        println!("  -- {} (len={}) --", stream_path, bytes.len());
        let stats = collect_stream_stats(&candidates);
        println!(
            "    hits={} btf_distinct={} byte_41_47_all_zero={} byte_41_47_any_nonzero={}",
            stats.hits,
            stats.bytes_to_follow_dist.len(),
            stats.byte_41_47_all_zero_count,
            stats.byte_41_47_any_nonzero_count
        );
        print_u32_dist("bytes_to_follow", &stats.bytes_to_follow_dist, 8);
        print_byte_dist("byte_40 sweep_direction_byte", &stats.byte_40_value_dist);

        let names: &[(usize, &str)] = &[
            (0, "+00 center_x?"),
            (8, "+08 center_y?"),
            (16, "+16 axis_a_x?"),
            (24, "+24 axis_a_y or rotation?"),
            (32, "+32 axis_ratio?"),
            (48, "+48 sweep_start_angle?"),
            (56, "+56 sweep_end_angle?"),
        ];
        for (pos, name) in names {
            if let Some(field_stats) = stats.field_stats.get(pos) {
                print_field_stats(name, field_stats);
            }
        }

        println!(
            "    axis_a_y bucket: zero={} pi/2={} pi={} 3pi/2={} other_finite={}",
            stats.axis_a_y_zero_count,
            stats.axis_a_y_pi_half_count,
            stats.axis_a_y_pi_count,
            stats.axis_a_y_three_pi_half_count,
            stats.axis_a_y_other_finite_count
        );
        println!(
            "    axis_ratio bucket: in[0,1]={} denormalized={} other={}",
            stats.axis_ratio_in_unit_range_count,
            stats.axis_ratio_denormalized_count,
            stats.axis_ratio_other_count
        );

        print_packed_u16_quad("packed u16 quad @+32", &stats.packed_u16_quad_at_32, 8);
        print_packed_u32_pair("packed u32 pair  @+32", &stats.packed_u32_pair_at_32, 8);
        print_packed_u16_quad("packed u16 quad @+48", &stats.packed_u16_quad_at_48, 8);
        print_packed_u32_pair("packed u32 pair  @+48", &stats.packed_u32_pair_at_48, 8);
        print_packed_u16_quad("packed u16 quad @+56", &stats.packed_u16_quad_at_56, 8);
        print_packed_u32_pair("packed u32 pair  @+56", &stats.packed_u32_pair_at_56, 8);

        report_axis_a_x_vs_center_x(&candidates);
        report_referenced_type_buckets(&candidates);
        report_attribute_tail_sweep_angles(&candidates);
        report_field_24_by_btf_bucket(&candidates);
        report_btf_bucket_tail_signature(&candidates);
        let known_oids = build_known_oid_table(&bytes);
        report_cross_record_references(&candidates, &known_oids);

        let dump_count = candidates.len().min(4);
        for (idx, cand) in candidates.iter().take(dump_count).enumerate() {
            dump_payload(&format!("dump[{}]", idx), cand);
            if !cand.tail.is_empty() {
                println!("      tail ({} bytes after 64B payload):", cand.tail.len());
                for chunk_start in (0..cand.tail.len()).step_by(16) {
                    let chunk_end = (chunk_start + 16).min(cand.tail.len());
                    let raw = &cand.tail[chunk_start..chunk_end];
                    let hex: String = raw
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let ascii: String = raw
                        .iter()
                        .map(|b| {
                            if (0x20..0x7F).contains(b) {
                                *b as char
                            } else {
                                '.'
                            }
                        })
                        .collect();
                    println!("        tail+{:03}: {:<48} | {}", chunk_start, hex, ascii);
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "test-file/DWG-0201GP06-01.pid",
        "test-file/DWG-0202GP06-01.pid",
        "test-file/工艺管道及仪表流程-1.pid",
        "test-file/export-test/publish-data/A01/A01.pid",
    ] {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        probe_fixture(path)?;
    }
    Ok(())
}
