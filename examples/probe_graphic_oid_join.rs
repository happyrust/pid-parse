//! Does `_Data.xml`'s `GraphicOID` name a Sheet record?
//!
//! `SmartPlant` publishes a drawing's semantic model as `<name>_Data.xml`, and
//! each `PIDRepresentation` in it carries one `IDrawingRepresentation
//! GraphicOID="<n>"`. Every Sheet record family this crate decodes carries an
//! `oid: u32`. Both are small integers. If they are the same identifier space,
//! the published model can be joined to the geometry this crate draws -- which
//! would give the first completeness measure sourced from the vendor's own
//! output rather than from this crate's self-consistency.
//!
//! **The judgement is which family an oid lands in, not how many land.** Oids
//! are a dense low range inside a document, so a hit rate proves nothing: the
//! `index` field was wrongly ruled out once on exactly that basis (format guide
//! §8.1). The families split two ways, and the split is the test:
//!
//! * **graphic** -- `GLine2d` / `igLine2d` / `igLineString2d` / `igPoint2d` /
//!   `igTextBox` / `igSymbol2d`. These emit a `PidGraphicKind`; a published
//!   representation is a *drawing representation*, so this is where its oid
//!   belongs.
//! * **non-graphic** -- `JStyleOverride` (a style), `DependencyObject` (a
//!   dependency edge), `igBoundary2d` (an association that deliberately emits
//!   nothing), `igSmartFrame2d` (the page border, not content). A published
//!   representation landing here is evidence *against* the hypothesis.
//!
//! Null hypothesis: draw as many oids at random, without replacement, from the
//! pool of every oid the Sheet streams define. The probability that all of them
//! land in graphic families is printed alongside the result.
//!
//! Read-only. No decoder changes, no emission changes.
//!
//! Two follow-up sections refine the verdict without changing it:
//!
//! * A published oid landing on `DependencyObject` is not the end of the
//!   question -- that record's tail carries OID references (payload `+22` and
//!   `+34` in the main bucket, see
//!   `docs/analysis/2026-08-04-graphicgroup-tail-property-block.md`). If the
//!   references land on graphics, the published representation names an
//!   aggregate node whose leaves draw; if they land nowhere, it is a genuine
//!   counter-example.
//! * A published oid absent from every typed decoder is either not in the
//!   file at all (the `.pid` and `_Data.xml` disagree -- a fixture-pairing
//!   problem) or present in bytes no decoder claims yet (a coverage gap, not
//!   evidence against the join). A raw little-endian `u32` scan over every
//!   stream separates the two.
//!
//! Usage: `cargo run --example probe_graphic_oid_join -- <drawing.pid> [more.pid ...]`
//!
//! The `_Data.xml` is looked up next to the `.pid` as `<stem>_Data.xml`. A
//! drawing without one is reported and skipped -- it is not a failure, most
//! fixtures have no published counterpart.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_dependency_objects, decode_igboundaries, decode_iglines, decode_iglinestrings,
    decode_igpoints, decode_igsymbols, decode_igtextboxes, decode_jstyle_overrides,
    decode_primitive_lines, decode_smartframes, SheetDependencyObjectDecoded,
};

/// Whether a record family's oids reach the canvas.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    /// Emits a `PidGraphicKind`.
    Graphic,
    /// Decoded, but audit-only or not drawing content.
    NonGraphic,
}

/// One decoded family: display name, whether it draws, and its oids.
struct FamilyOids {
    name: &'static str,
    class: Family,
    oids: BTreeSet<u32>,
}

fn cfb_streams(path: &Path, sheets_only: bool) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|path| {
            !sheets_only
                || path
                    .rsplit(['/', '\\'])
                    .next()
                    .is_some_and(|leaf| leaf.starts_with("Sheet"))
        })
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_ok() {
            out.push((name, data));
        }
    }
    out
}

fn sheet_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    cfb_streams(path, true)
}

/// Every oid the Sheet streams define, grouped by decoded family.
///
/// Only typed decoders contribute. Reading a u32 out of an untyped family to
/// pad the pool would be exactly the kind of unfounded assumption the format
/// guide's §0 forbids, and it would bias the null hypothesis in the
/// hypothesis's favour.
fn sheet_oids(path: &Path) -> Vec<FamilyOids> {
    let mut families: Vec<FamilyOids> = [
        ("GLine2d 0x3FE6", Family::Graphic),
        ("igLine2d 0x0018", Family::Graphic),
        ("igLineString2d 0x0084", Family::Graphic),
        ("igPoint2d 0x005E", Family::Graphic),
        ("igTextBox 0x004D", Family::Graphic),
        ("igSymbol2d 0x00CE", Family::Graphic),
        ("JStyleOverride 0x0030", Family::NonGraphic),
        ("DependencyObject 0x00FA", Family::NonGraphic),
        ("igBoundary2d 0x0013", Family::NonGraphic),
        ("igSmartFrame2d 0x003D", Family::NonGraphic),
    ]
    .into_iter()
    .map(|(name, class)| FamilyOids {
        name,
        class,
        oids: BTreeSet::new(),
    })
    .collect();

    for (_, data) in sheet_streams(path) {
        let per_family: [Vec<u32>; 10] = [
            decode_primitive_lines(&data)
                .iter()
                .map(|r| r.oid)
                .collect(),
            decode_iglines(&data).iter().map(|r| r.oid).collect(),
            decode_iglinestrings(&data).iter().map(|r| r.oid).collect(),
            decode_igpoints(&data).iter().map(|r| r.oid).collect(),
            decode_igtextboxes(&data).iter().map(|r| r.oid).collect(),
            decode_igsymbols(&data).iter().map(|r| r.oid).collect(),
            decode_jstyle_overrides(&data)
                .iter()
                .map(|r| r.oid)
                .collect(),
            decode_dependency_objects(&data)
                .iter()
                .map(|r| r.oid)
                .collect(),
            decode_igboundaries(&data).iter().map(|r| r.oid).collect(),
            decode_smartframes(&data).iter().map(|r| r.oid).collect(),
        ];
        for (slot, oids) in per_family.into_iter().enumerate() {
            families[slot].oids.extend(oids);
        }
    }
    families
}

/// `GraphicOID`s from a published `_Data.xml`, in document order.
///
/// Deliberately a plain scan rather than an XML parse: the file is
/// vendor-generated with one attribute per element and this probe must not add
/// a dependency to answer a yes/no question.
fn published_graphic_oids(xml: &str) -> Vec<u32> {
    const NEEDLE: &str = "GraphicOID=\"";
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(at) = rest.find(NEEDLE) {
        rest = &rest[at + NEEDLE.len()..];
        let Some(end) = rest.find('"') else {
            break;
        };
        if let Ok(oid) = rest[..end].parse::<u32>() {
            out.push(oid);
        }
        rest = &rest[end..];
    }
    out
}

/// `P(all k draws land in the favoured subset)` under sampling without
/// replacement from a pool of `total`, of which `favoured` are in the subset.
fn all_draws_favoured(favoured: usize, total: usize, draws: usize) -> f64 {
    if draws > favoured || total == 0 {
        return 0.0;
    }
    let mut p = 1.0f64;
    for step in 0..draws {
        p *= (favoured - step) as f64 / (total - step) as f64;
    }
    p
}

/// `P(every one of a `size`-member family is inside a random `draws`-subset of
/// `total`)` — `C(total-size, draws-size) / C(total, draws)`, computed as a
/// product to stay in range.
///
/// This is the statistic for "the published set contains *all* of family X",
/// which is a far stronger observation than any hit rate.
fn family_fully_covered(size: usize, total: usize, draws: usize) -> f64 {
    if size > draws || draws > total {
        return 0.0;
    }
    let mut p = 1.0f64;
    for step in 0..size {
        p *= (draws - step) as f64 / (total - step) as f64;
    }
    p
}

/// `P(k independent integers drawn uniformly from `span` distinct values all
/// land inside a `hits`-member set)` — the test for "these two identifier
/// spaces are the same space at all".
fn same_space(hits: usize, span: usize, draws: usize) -> f64 {
    if span == 0 {
        return 0.0;
    }
    (hits as f64 / span as f64).powi(draws as i32)
}

fn data_xml_beside(pid: &Path) -> Option<PathBuf> {
    let stem = pid.file_stem()?.to_string_lossy().into_owned();
    let candidate = pid.with_file_name(format!("{stem}_Data.xml"));
    candidate.is_file().then_some(candidate)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: probe_graphic_oid_join <drawing.pid> [more.pid ...]");
        return;
    }

    for arg in args {
        let pid = PathBuf::from(&arg);
        println!("{}", pid.display());

        let Some(xml_path) = data_xml_beside(&pid) else {
            println!("  no <stem>_Data.xml beside it -- skipped\n");
            continue;
        };
        let Ok(xml) = std::fs::read_to_string(&xml_path) else {
            println!("  {} unreadable -- skipped\n", xml_path.display());
            continue;
        };

        let published = published_graphic_oids(&xml);
        let distinct: BTreeSet<u32> = published.iter().copied().collect();
        println!(
            "  published: {} GraphicOID ({} distinct, {}..{})",
            published.len(),
            distinct.len(),
            distinct.first().copied().unwrap_or(0),
            distinct.last().copied().unwrap_or(0)
        );

        let families = sheet_oids(&pid);
        let pool: BTreeSet<u32> = families
            .iter()
            .flat_map(|f| f.oids.iter().copied())
            .collect();
        let graphic_pool: BTreeSet<u32> = families
            .iter()
            .filter(|f| f.class == Family::Graphic)
            .flat_map(|f| f.oids.iter().copied())
            .collect();
        println!(
            "  sheet pool: {} distinct oid ({}..{}), {} graphic / {} non-graphic-only",
            pool.len(),
            pool.first().copied().unwrap_or(0),
            pool.last().copied().unwrap_or(0),
            graphic_pool.len(),
            pool.len() - graphic_pool.len()
        );
        for family in &families {
            if !family.oids.is_empty() {
                println!(
                    "    {:<24} {:>4} oid  [{}]",
                    family.name,
                    family.oids.len(),
                    match family.class {
                        Family::Graphic => "graphic",
                        Family::NonGraphic => "non-graphic",
                    }
                );
            }
        }

        // Where each published oid lands. A published oid can legitimately name
        // records in more than one family (a symbol placement plus its label),
        // so the verdict is per-oid: graphic-only, mixed, non-graphic-only, or
        // absent.
        let mut verdicts: BTreeMap<&'static str, Vec<u32>> = BTreeMap::new();
        let mut landing: BTreeMap<String, usize> = BTreeMap::new();
        for &oid in &distinct {
            let hits: Vec<&FamilyOids> = families
                .iter()
                .filter(|family| family.oids.contains(&oid))
                .collect();
            let verdict = if hits.is_empty() {
                "absent"
            } else if hits.iter().all(|f| f.class == Family::Graphic) {
                "graphic-only"
            } else if hits.iter().all(|f| f.class == Family::NonGraphic) {
                "non-graphic-only"
            } else {
                "mixed"
            };
            verdicts.entry(verdict).or_default().push(oid);
            if !hits.is_empty() {
                let key = hits.iter().map(|f| f.name).collect::<Vec<_>>().join(" + ");
                *landing.entry(key).or_default() += 1;
            }
        }

        println!("  verdict:");
        for verdict in ["graphic-only", "mixed", "non-graphic-only", "absent"] {
            let oids = verdicts.get(verdict).map(Vec::as_slice).unwrap_or(&[]);
            if oids.is_empty() {
                continue;
            }
            let shown: Vec<String> = oids.iter().take(12).map(u32::to_string).collect();
            let tail = if oids.len() > 12 { " ..." } else { "" };
            println!(
                "    {:<17} {:>3}   {}{}",
                verdict,
                oids.len(),
                shown.join(","),
                tail
            );
        }

        println!("  landed on:");
        let mut by_count: Vec<(&String, &usize)> = landing.iter().collect();
        by_count.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (key, count) in by_count {
            println!("    {count:>3} x {key}");
        }

        let present = distinct.len() - verdicts.get("absent").map_or(0, Vec::len);
        let graphic_only = verdicts.get("graphic-only").map_or(0, Vec::len);

        // How much of each family the published set names. A family that is
        // *entirely* published is the strongest signal available here, and it
        // is not a hit rate: it is a containment.
        println!("  family coverage (published / decoded):");
        for family in families.iter().filter(|f| !f.oids.is_empty()) {
            let named = family.oids.iter().filter(|o| distinct.contains(o)).count();
            let note = if named == family.oids.len() {
                let p = family_fully_covered(family.oids.len(), pool.len(), distinct.len());
                format!("  <- whole family, P={p:.3e}")
            } else {
                String::new()
            };
            println!(
                "    {:<24} {:>3} / {:<3}{note}",
                family.name,
                named,
                family.oids.len()
            );
        }

        // Is this even the same identifier space? Only meaningful against the
        // window the published values actually occupy.
        let (lo, hi) = (
            distinct.first().copied().unwrap_or(0),
            distinct.last().copied().unwrap_or(0),
        );
        let span = (hi - lo + 1) as usize;
        let pool_in_span = pool.range(lo..=hi).count();
        println!(
            "  same-space test: {pool_in_span} of the pool sit in the published window \
             {lo}..{hi} ({span} values); P({present}/{} random hits) = {:.3e}",
            distinct.len(),
            same_space(pool_in_span, span, distinct.len())
        );

        let p_graphic = all_draws_favoured(graphic_pool.len(), pool.len(), present);
        println!(
            "  graphic-class test: P(all {present} present oids land graphic) = {p_graphic:.3e}  \
             (pool {} graphic / {} total)",
            graphic_pool.len(),
            pool.len()
        );
        println!(
            "  coverage: {present}/{} published oids exist in the sheet decode, \
             {graphic_only} of them graphic-only",
            distinct.len()
        );

        resolve_dependency_edges(&pid, &distinct, &families, &pool);
        scan_absent_oids_raw(
            &pid,
            verdicts.get("absent").map(Vec::as_slice).unwrap_or(&[]),
        );
        println!();
    }
}

/// Follow-up 1: a published oid landing on `DependencyObject` names a
/// dependency record, and that record's tail carries OID references (payload
/// `+22` / `+34` in the main bucket). Resolving those references tells whether
/// the representation points at an aggregate whose leaves are graphics, or at
/// a genuine dead end.
fn resolve_dependency_edges(
    pid: &Path,
    published: &BTreeSet<u32>,
    families: &[FamilyOids],
    pool: &BTreeSet<u32>,
) {
    let dep_family = families
        .iter()
        .find(|f| f.name.starts_with("DependencyObject"));
    let Some(dep_family) = dep_family else {
        return;
    };
    let landed: Vec<u32> = published
        .iter()
        .copied()
        .filter(|oid| dep_family.oids.contains(oid))
        .collect();
    if landed.is_empty() {
        return;
    }

    let records: Vec<SheetDependencyObjectDecoded> = sheet_streams(pid)
        .iter()
        .flat_map(|(_, data)| decode_dependency_objects(data))
        .collect();

    println!(
        "  dependency-edge resolution ({} published oids name a DependencyObject):",
        landed.len()
    );
    let mut oids_with_graphic_edge = 0usize;
    for &oid in &landed {
        let mut edges: Vec<String> = Vec::new();
        let mut shapes: Vec<String> = Vec::new();
        let mut any_graphic = false;
        for rec in records.iter().filter(|r| r.oid == oid) {
            shapes.push(format!(
                "btf={} kind={}",
                rec.bytes_to_follow, rec.group_kind_word
            ));
            let tail = &rec.raw_reference_payload;
            for at in (0..tail.len().saturating_sub(3)).step_by(4) {
                let value =
                    u32::from_le_bytes([tail[at], tail[at + 1], tail[at + 2], tail[at + 3]]);
                if value == 0 || value == oid || !pool.contains(&value) {
                    continue;
                }
                let names: Vec<&str> = families
                    .iter()
                    .filter(|f| f.oids.contains(&value))
                    .map(|f| f.name)
                    .collect();
                any_graphic |= families
                    .iter()
                    .any(|f| f.class == Family::Graphic && f.oids.contains(&value));
                edges.push(format!("+{}->{value} [{}]", 18 + at, names.join("+")));
            }
        }
        if any_graphic {
            oids_with_graphic_edge += 1;
        }
        println!(
            "    oid {oid:<5} ({})  {}",
            shapes.join(", "),
            if edges.is_empty() {
                "no pool references in tail".to_string()
            } else {
                edges.join("  ")
            }
        );
    }
    println!(
        "    => {oids_with_graphic_edge}/{} of these oids depend on at least one graphic oid",
        landed.len()
    );
}

/// Follow-up 2: an absent published oid is either not in the file at all (the
/// `.pid` / `_Data.xml` pairing is broken) or sits in bytes no typed decoder
/// claims yet (a coverage gap). A raw little-endian `u32` scan over **every**
/// stream separates the two readings.
fn scan_absent_oids_raw(pid: &Path, absent: &[u32]) {
    if absent.is_empty() {
        return;
    }
    let streams = cfb_streams(pid, false);
    let total_bytes: usize = streams.iter().map(|(_, data)| data.len()).sum();
    println!(
        "  raw-byte scan for absent oids ({} streams, {} bytes):",
        streams.len(),
        total_bytes
    );
    for &oid in absent {
        let needle = oid.to_le_bytes();
        let mut hits: Vec<String> = Vec::new();
        for (name, data) in &streams {
            let mut count = 0usize;
            let mut first = None;
            for at in 0..data.len().saturating_sub(3) {
                if data[at..at + 4] == needle {
                    count += 1;
                    if first.is_none() {
                        first = Some(at);
                    }
                }
            }
            if let Some(first) = first {
                hits.push(format!("{name} x{count} first@0x{first:X}"));
            }
        }
        if hits.is_empty() {
            println!(
                "    oid {oid:<6} nowhere in the file -- the .pid does not contain this identifier"
            );
        } else {
            println!("    oid {oid:<6} {}", hits.join(", "));
        }
    }
}
