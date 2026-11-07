//! Settle whether IGDS `0x0021` (`ComplexString Object`, `imagdex.dex`
//! CLSID `3750D460-90EB-11CE-976E-08003601E26D`, see
//! `docs/analysis/2026-08-04-psm-type-code-registry.md`) is an
//! association record like `0x0013 igBoundary2d`, or a record whose
//! own geometry would be lost by staying undecoded.
//!
//! `D04.pid` (`backup-plant-data/test/U01/D04.pid`) carries the only
//! `0x0021` record in the whole `pid-test-data` corpus: `/Sheet6 +2478`,
//! `oid=62`, `bytes_to_follow=191`. This probe re-derives its member
//! list and checks every member oid against the sheet's own decoded
//! `igLine2d` / `igArc2d` records.
//!
//! Payload grammar (18-byte IGDS sub-header, then):
//!
//! - `+0` `u8 0x01`, `+1` `u16 0x0006`, `+3` `u32 member_count`,
//!   `+7` `f64 1.0` (always seen as exactly `1.0` across every fixture
//!   record, D04's included — a fixed scale/weight field, not data);
//! - `+15` onward: `member_count` groups of 14 bytes each —
//!   `u32 member_oid` + 4×`i16` (a local bounding box, never checked
//!   here) + `u16 tag`. `tag & 0x8000 == 0` marks a member whose oid
//!   resolves to a real `igLine2d`/`igArc2d` record in the same
//!   stream; `tag & 0x8000 != 0` marks a companion reference that
//!   resolves to neither (confirmed on both `D04.pid` and the
//!   `CPECCHB-SQ-A2.igr` border template, which carries three more
//!   `0x0021` records with the same split);
//! - a 4-byte trailer `00 02 00 00` closes every record seen so far.
//!
//! D04's record lists 11 members: 6 with `tag & 0x8000 == 0` (oids
//! 27, 337, 361, 380, 275, 175 — 4 `igArc2d` + 2 `igLine2d`, matching
//! the finding written up for this record) and 5 with the `0x8000`
//! bit set (oids 196, 300, 314, 456, 468) that this probe confirms
//! are *not* `igLine2d`/`igArc2d`/`igPoint2d`/`igCircle2d` records.
//!
//! All 6 geometry members already reach
//! `build_normalized_geometry` as independent `Decoded` `Arc` / `Line`
//! entities before `0x0021` is ever read — the association carries no
//! ink of its own, the same policy `0x0013 igBoundary2d` uses. This
//! probe is the audit trail for leaving `0x0021` undecoded.
//!
//! Read-only probe; run with
//! `cargo run --quiet --example probe_0021_complexstring_members`.

use pid_parse::build_normalized_geometry;
use pid_parse::parsers::sheet_records::{
    decode_igarcs, decode_igcircles, decode_iglines, decode_igpoints,
};
use pid_parse::PidParser;
use std::path::Path;

/// PSM 14-bit type code for the IGDS `ComplexString Object` class tag,
/// used directly as a PSM type code the same way `igLine2d` /
/// `igArc2d` are (Slice J discovery).
const PSM_TYPE_CODE_COMPLEXSTRING: u16 = 0x0021;

/// One member reference decoded from a `0x0021` payload.
struct Member {
    oid: u32,
    tag: u16,
}

/// Decode every `0x0021` record's member list in `data`, returning
/// `(record_offset, members)` pairs. Mirrors the shared PSM envelope
/// walk (`type_word & 0x3FFF`, `bytes_to_follow` as a `u32`) without
/// reaching for a dedicated decoder — this type stays audit-only.
fn scan_complexstrings(data: &[u8]) -> Vec<(usize, Vec<Member>)> {
    let mut out = Vec::new();
    let mut at = 8usize;
    while at + 6 <= data.len() {
        let type_word = u16::from_le_bytes([data[at], data[at + 1]]);
        if type_word & 0x3FFF == 0 {
            break;
        }
        let btf =
            u32::from_le_bytes([data[at + 2], data[at + 3], data[at + 4], data[at + 5]]) as usize;
        let body = at + 6;
        let Some(end) = body.checked_add(btf) else {
            break;
        };
        if end > data.len() {
            break;
        }
        if type_word & 0x3FFF == PSM_TYPE_CODE_COMPLEXSTRING {
            if let Some(members) = decode_members(&data[body..end]) {
                out.push((at, members));
            }
        }
        at = end;
    }
    out
}

/// Parse the member list out of one `0x0021` payload (past the
/// 18-byte IGDS sub-header). Returns `None` if the fixed prefix or
/// the member count don't fit the payload length.
fn decode_members(payload: &[u8]) -> Option<Vec<Member>> {
    let rest = payload.get(18..)?;
    if rest.len() < 15 {
        return None;
    }
    let count = u32::from_le_bytes(rest.get(3..7)?.try_into().ok()?) as usize;
    let members_start = 15usize;
    let needed = members_start.checked_add(count.checked_mul(14)?)?;
    if rest.len() < needed {
        return None;
    }
    let mut members = Vec::with_capacity(count);
    let mut at = members_start;
    for _ in 0..count {
        let oid = u32::from_le_bytes(rest[at..at + 4].try_into().ok()?);
        let tag = u16::from_le_bytes(rest[at + 12..at + 14].try_into().ok()?);
        members.push(Member { oid, tag });
        at += 14;
    }
    Some(members)
}

fn main() {
    let path = Path::new(r"backup-plant-data/test/U01/D04.pid");
    let candidates = [
        path.to_path_buf(),
        Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
        std::env::var("PID_TEST_DATA_DIR")
            .map(|dir| Path::new(&dir).join(path))
            .unwrap_or_default(),
        Path::new(r"D:\work\plant-code\cad\pid-test-data").join(path),
    ];
    let Some(found) = candidates.iter().find(|p| p.exists()) else {
        println!("D04.pid not found under any candidate path; soft-skip.");
        return;
    };

    let package = PidParser::new()
        .parse_package(found)
        .expect("D04.pid parses as a compound file");
    let normalized = build_normalized_geometry(&package.parsed);

    for (stream_path, raw) in &package.streams {
        if !stream_path.starts_with("/Sheet") {
            continue;
        }
        let hits = scan_complexstrings(&raw.data);
        if hits.is_empty() {
            continue;
        }
        let lines = decode_iglines(&raw.data);
        let arcs = decode_igarcs(&raw.data);
        let points = decode_igpoints(&raw.data);
        let circles = decode_igcircles(&raw.data);

        for (offset, members) in hits {
            println!("{stream_path} +{offset}: {} members", members.len());
            for member in &members {
                let geometry = lines
                    .iter()
                    .find(|l| l.oid == member.oid)
                    .map(|l| (l.byte_range.clone(), "igLine2d"))
                    .or_else(|| {
                        arcs.iter()
                            .find(|a| a.oid == member.oid)
                            .map(|a| (a.byte_range.clone(), "igArc2d"))
                    });
                let other_hit = points.iter().any(|p| p.oid == member.oid)
                    || circles.iter().any(|c| c.oid == member.oid);

                match &geometry {
                    Some((range, kind)) => {
                        let emitted = normalized.entities.iter().any(|e| {
                            e.source.stream_path.as_deref() == Some(stream_path.as_str())
                                && e.source.byte_range.as_ref().is_some_and(|br| {
                                    br.start == range.start && br.end == range.end
                                })
                        });
                        println!(
                            "  oid={:<5} tag=0x{:04X} -> {kind} range={range:?} emitted_independently={emitted}",
                            member.oid, member.tag
                        );
                    }
                    None if other_hit => {
                        println!(
                            "  oid={:<5} tag=0x{:04X} -> igPoint2d/igCircle2d (unexpected for a 0x8000 member)",
                            member.oid, member.tag
                        );
                    }
                    None => {
                        println!(
                            "  oid={:<5} tag=0x{:04X} -> no igLine2d/igArc2d/igPoint2d/igCircle2d match",
                            member.oid, member.tag
                        );
                    }
                }
            }
        }
    }
}
