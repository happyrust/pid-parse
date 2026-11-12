//! What curve each member of an `igBoundary2d` re-lists, and what the record
//! says after its last member.
//!
//! Twenty fills of the `-W` border templates — the logo of `A0-W.igr` to
//! `A3-W.igr`, inside a backup package's `RefData~4~682.zip` — were refused
//! by every grammar the family had, and twenty-one fills of two other
//! templates of that archive decoded with an anchor that is not inside
//! their ring. This probe reads every live `0x0013` record on the sheet
//! chains of the documents it is given by one grammar, written out here and
//! sharing nothing with the library's decoders:
//!
//! ```text
//! oid, owner, layer, sub-type word, style            18 bytes
//! u32 loops
//!   per loop: u32 members, two bytes, the members
//! anchor (2 x f64), flag byte, u32 references
//!   per reference: u32 oid, u16 class word, u16 word  to the payload's end
//! ```
//!
//! with a member a tag byte and then the geometry of one curve as that
//! curve's own record states it after its 18-byte sub-header:
//!
//! | tag | member | record |
//! |---|---|---|
//! | `0x67` | start, end | `igLine2d` |
//! | `0x68` | `u32` count, two bytes, the points | `igLineString2d` |
//! | `0x73` | centre, major semi-axis vector, ratio, a byte | `igEllipse2d`; `igCircle2d` as the ellipse of vector `(radius, 0)` and ratio one, which is every one the corpora hold |
//! | `0x74` | two parameters, centre, major semi-axis vector, ratio, direction byte | `igEllipticalArc2d`; `igArc2d` as the ellipse of vector `(radius, 0)` and ratio one |
//! | `0x7F` | `u32 N`, poles, weight flag and weights, `u32 M`, knots, a double, four bytes | `igBspCurve2d` |
//!
//! and holds each member against the live record a reference of its
//! boundary names: a line to the line (whole, or a piece of it), an arc to
//! the circle or ellipse and to the record's two ends, a B-spline to the
//! record's curve, each either way round.
//!
//! ```text
//! cargo run --example probe_what_curve_each_boundary_member_relists
//! cargo run --example probe_what_curve_each_boundary_member_relists -- --verbose A1-W.igr test-file
//! ```
//!
//! With no directory it reads `test-file` and, where this machine has them,
//! the package and acceptance corpora (`PID_TEST_DATA_DIR`, then the two
//! `pid-test-data` copies) and the share: each directory's `.pid` and `.igr`
//! files and the `.igr` / `.pid` members of its `RefData~*.zip` and
//! `PlantData~*.zip`, each distinct file once. `--verbose <part of a name>`
//! lists the records of the documents whose name holds it, member by member.
//!
//! # What it read (2026-11-12, this machine, all four directories)
//!
//! 111 distinct documents, 1 743 live boundaries, every one read by the
//! grammar above to its payload's last byte, and — since the library reads a
//! loop of curves that names its members — every one decoded by the library
//! too (1 703 before, the other 40 being the `-W` logo's five fills in the
//! four `.igr` templates and again in the four `.pid` templates that embed
//! those frames).
//!
//! * **Members**: 5 513 lines, 13 point runs, 1 circle, 410 arcs, 32
//!   B-splines. Of the arcs 378 are a circle's — major semi-axis `(r, 0)`,
//!   ratio one, bit for bit — and 32 the `-W` logo's ellipse.
//! * **References**: 100 records name no member, 1 642 as many as they have,
//!   one (a two-loop record of `XA-A1.igr`) one more. The class word says
//!   what is named — `0x00CB` a line (5 304), `0x00D1` a circle (1),
//!   `0x00D3` an arc or an elliptical arc (48, 32), `0x00D4` a B-spline (32)
//!   — and every reference names one live record of the chain.
//! * **Each member is the curve a reference names**: all 5 416 members of
//!   the records that name theirs, 5 296 by the reference of their own
//!   place and 120 by another (in four records the references run a place
//!   or more out of step with the members). A line member is the whole line
//!   (2 257) or a piece of it between two crossings (3 046); an arc is the
//!   whole arc (48 of a circle, 32 of the ellipse); a B-spline is the
//!   record's whole curve with every interior knot raised to the degree,
//!   the same point at every parameter to 4.5e-16 m.
//! * **The reference's last word**: on a record's first loop — its only
//!   loop, on all but two — bit 2 is set on every member that runs the way
//!   its record does (3 742) and clear on every one that runs against it
//!   (1 542). On the second loop of the two records that have one, the hole
//!   of a fill, it is the other way round: clear on all 131 members, every
//!   one of which runs its record's way. Bit 0 is set on the first reference
//!   of a record and on no other; the first reference of a second loop
//!   carries bit 1 (2 records).
//! * **The anchor**: `(1e30, 1e30)` on all 100 records that name no member,
//!   and a point inside the fill — inside an odd number of its loops — on
//!   all 1 643 that do. The library's `anchor` for a record of the first
//!   kind is its ring's first vertex, which is how an anchor came to be
//!   read "outside the ring" on the 21 fills of `CPECCHBA2.igr` and
//!   `qsmcqtaz13A2.igr`.
//! * **The fill's edge**: the library flattens a curved member after the
//!   way its record is drawn. All 32 arcs of the ellipse and all 32
//!   B-splines run through the very vertices of the polyline a sheet draws
//!   their `igEllipticalArc2d` or `igBspCurve2d` as; an arc of the ellipse
//!   is then 0.0042 mm off the ellipse at most (0.0366 mm while it was cut
//!   into chords of a quarter radian, and the 14° arc of a stripe one
//!   straight line). The arc of a circle is drawn as a true arc and filled
//!   to chords: at most 0.0463 mm off its arc, on the outer edge of the
//!   `-W` crescent (radius 6.7228 mm, fifteen chords), which is drawn with
//!   a 0.13 mm pen. Of the 379 conic members that are a circle's, 49 name a
//!   record whose pen resolves, and the chords of all 49 lie within half
//!   that pen's width of the arc (0.71 of it at most).
//! * **The fill's area** against the area inside its members' own curves —
//!   a conic at 4 096 steps, a B-spline at 256 to a span — over the 127
//!   records with a conic member whose loop chains: at most 0.21% short
//!   where the record holds an arc of the ellipse (a cap of the logo,
//!   9.746 836 for 9.766 983 mm²), at most 1.71% short where its conics are
//!   all a circle's (the crescent, 40.638 130 for 41.344 461 mm²). `D04`'s
//!   oid 131 is not compared: two of its members stand 2.6 mm apart.
//! * **Symbol caches**: the boundaries of the drawings' `/JSite*/PSMcluster0`
//!   are read by another reader of the library, and are only counted here:
//!   200 live records, all 200 read by the same grammar to the last byte —
//!   191 of one loop and 9 of two; 171 circles, 89 lines, 54 arcs and 6
//!   point runs for members, every circle and every arc with the major
//!   semi-axis `(r, 0)` and the ratio one.
//! * **What it does not explain**: the two bytes after a loop's member count
//!   (`02 01` on 1 688 loops, `03 01` on 40, `00 00` on 13, `01 01` on 2,
//!   `02 00` and `04 01` on one each), the flag byte before the reference
//!   count (1 on 1 741 records, 2 on two), and bit 3 of a reference's last
//!   word (set on 1 047 members, each a whole line run its record's way).
//!   Twenty-one loops do not close to a nanometre: the file's own curves
//!   leave 1.5 µm to 2.6 mm between two members.
//!
//! It exits non-zero when a record is not read to its last byte, the
//! library refuses one, a member lies on no record its boundary names, the
//! direction bit disagrees with the way a member runs, the anchor rule
//! fails, a member whose record is drawn as a polyline is not flattened to
//! that polyline's vertices, or a conic member is flattened farther from
//! its curve than half the pen its record is drawn with.

use pid_parse::parsers::sheet_records::{
    decode_igarc_at, decode_igboundary_at, decode_igbspcurve_at, decode_igcircle_at,
    decode_igellipse_at, decode_igellipticalarc_at, decode_igline_at, decode_iglinestring_at,
    parse_psm_header, sheet_record_starts,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn u16_at(p: &[u8], at: usize) -> Option<u16> {
    p.get(at..at.checked_add(2)?)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn u32_at(p: &[u8], at: usize) -> Option<u32> {
    p.get(at..at.checked_add(4)?)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn f64_at(p: &[u8], at: usize) -> Option<f64> {
    p.get(at..at.checked_add(8)?)
        .map(|b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

type P = (f64, f64);

/// One member of a loop, as the grammar above reads it.
#[derive(Debug, Clone)]
enum Member {
    Line {
        start: P,
        end: P,
    },
    Run {
        points: Vec<P>,
    },
    Ellipse {
        center: P,
        major: P,
        ratio: f64,
        flag: u8,
    },
    Arc {
        start: f64,
        end: f64,
        center: P,
        major: P,
        ratio: f64,
        flag: u8,
    },
    Spline {
        poles: Vec<P>,
        weights: Vec<f64>,
        knots: Vec<f64>,
    },
}

impl Member {
    fn tag(&self) -> u8 {
        match self {
            Member::Line { .. } => 0x67,
            Member::Run { .. } => 0x68,
            Member::Ellipse { .. } => 0x73,
            Member::Arc { .. } => 0x74,
            Member::Spline { .. } => 0x7F,
        }
    }

    /// Points along the member, from its start to its end.
    fn points(&self) -> Vec<P> {
        self.curve(64)
    }

    /// The member's own curve, from its start to its end: a conic at
    /// `steps` equal steps of its parameter, a B-spline sampled at a
    /// sixteenth as many to a span (sixteen for [`Member::points`]), a line
    /// or a run as it is.
    fn curve(&self, steps: u32) -> Vec<P> {
        match self {
            Member::Line { start, end } => vec![*start, *end],
            Member::Run { points } => points.clone(),
            Member::Ellipse {
                center,
                major,
                ratio,
                flag,
            } => {
                let turn = if *flag == 0 {
                    -std::f64::consts::TAU
                } else {
                    std::f64::consts::TAU
                };
                (0..=steps)
                    .map(|i| {
                        let t = turn * f64::from(i) / f64::from(steps);
                        ellipse_at(*center, *major, *ratio, t)
                    })
                    .collect()
            }
            Member::Arc {
                start,
                end,
                center,
                major,
                ratio,
                flag,
            } => {
                let sweep = arc_sweep(*start, *end, *flag);
                (0..=steps)
                    .map(|i| {
                        let t = start + sweep * f64::from(i) / f64::from(steps);
                        ellipse_at(*center, *major, *ratio, t)
                    })
                    .collect()
            }
            Member::Spline {
                poles,
                weights,
                knots,
            } => pid_parse::bspline::sample(poles, weights, knots, (steps / 16).max(16) as usize),
        }
    }
}

/// What a conic member's major semi-axis vector and ratio say it is.
fn conic_shape(major: P, ratio: f64) -> &'static str {
    if major.1 == 0.0 && ratio == 1.0 {
        "major semi-axis (r, 0), ratio 1"
    } else if (ratio - 1.0).abs() < 1e-12 {
        "a circle whose major semi-axis is off +X"
    } else {
        "an ellipse"
    }
}

/// The area a fill paints: its first loop's, less that of every later one.
fn fill_area(loops: &[Vec<P>]) -> f64 {
    loops
        .iter()
        .enumerate()
        .map(|(index, ring)| {
            let area = signed_area(ring).abs();
            if index == 0 {
                area
            } else {
                -area
            }
        })
        .sum()
}

/// How far `point` stands from the nearest segment of `polyline`.
fn distance_to_polyline(point: P, polyline: &[P]) -> f64 {
    polyline
        .windows(2)
        .map(|pair| {
            let (a, b) = (pair[0], pair[1]);
            let d = (b.0 - a.0, b.1 - a.1);
            let length_squared = d.0 * d.0 + d.1 * d.1;
            let along = if length_squared > 0.0 {
                (((point.0 - a.0) * d.0 + (point.1 - a.1) * d.1) / length_squared).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (point.0 - (a.0 + along * d.0)).hypot(point.1 - (a.1 + along * d.1))
        })
        .fold(f64::INFINITY, f64::min)
}

/// `C + cos t · major + sin t · minor`, `minor` the major semi-axis turned a
/// quarter and scaled by the ratio.
fn ellipse_at(center: P, major: P, ratio: f64, t: f64) -> P {
    let minor = (-major.1 * ratio, major.0 * ratio);
    (
        center.0 + t.cos() * major.0 + t.sin() * minor.0,
        center.1 + t.cos() * major.1 + t.sin() * minor.1,
    )
}

/// The signed parameter an arc turns through: down for direction byte `0`,
/// up otherwise; two angles a whole turn apart are a whole turn.
fn arc_sweep(start: f64, end: f64, flag: u8) -> f64 {
    use std::f64::consts::TAU;
    let whole = (end - start).abs() >= TAU - 1e-9;
    match (flag == 0, whole) {
        (true, true) => -TAU,
        (false, true) => TAU,
        (true, false) => -(start - end).rem_euclid(TAU),
        (false, false) => (end - start).rem_euclid(TAU),
    }
}

struct Loop {
    /// The two bytes after the loop's member count.
    tail: [u8; 2],
    /// Each member with the payload offset of its tag.
    members: Vec<(usize, Member)>,
}

struct Read {
    owner: u32,
    layer: u32,
    sub: u16,
    style: u32,
    loops: Vec<Loop>,
    anchor: P,
    flag: u8,
    /// `(oid, class word, last word)`.
    refs: Vec<(u32, u16, u16)>,
}

/// One member at `pos`: the member and the offset after it.
fn read_member(p: &[u8], pos: usize) -> Result<(Member, usize), String> {
    let tag = *p.get(pos).ok_or_else(|| format!("no tag byte at +{pos}"))?;
    let f = |at: usize| f64_at(p, at).ok_or_else(|| format!("no double at +{at}"));
    match tag {
        0x67 => Ok((
            Member::Line {
                start: (f(pos + 1)?, f(pos + 9)?),
                end: (f(pos + 17)?, f(pos + 25)?),
            },
            pos + 33,
        )),
        0x68 => {
            let count = u32_at(p, pos + 1).ok_or("no run count")? as usize;
            if count > p.len() / 16 {
                return Err(format!("a run of {count} points at +{pos}"));
            }
            let mut points = Vec::with_capacity(count);
            for i in 0..count {
                points.push((f(pos + 7 + i * 16)?, f(pos + 15 + i * 16)?));
            }
            Ok((Member::Run { points }, pos + 7 + count * 16))
        }
        0x73 => Ok((
            Member::Ellipse {
                center: (f(pos + 1)?, f(pos + 9)?),
                major: (f(pos + 17)?, f(pos + 25)?),
                ratio: f(pos + 33)?,
                flag: *p.get(pos + 41).ok_or("no ellipse byte")?,
            },
            pos + 42,
        )),
        0x74 => Ok((
            Member::Arc {
                start: f(pos + 1)?,
                end: f(pos + 9)?,
                center: (f(pos + 17)?, f(pos + 25)?),
                major: (f(pos + 33)?, f(pos + 41)?),
                ratio: f(pos + 49)?,
                flag: *p.get(pos + 57).ok_or("no direction byte")?,
            },
            pos + 58,
        )),
        0x7F => {
            let mut at = pos + 1;
            let n = u32_at(p, at).ok_or("no pole count")? as usize;
            if n > p.len() / 16 {
                return Err(format!("{n} poles at +{pos}"));
            }
            at += 4;
            let mut poles = Vec::with_capacity(n);
            for _ in 0..n {
                poles.push((f(at)?, f(at + 8)?));
                at += 16;
            }
            let rational = u32_at(p, at).ok_or("no weight flag")?;
            at += 4;
            let mut weights = Vec::new();
            if rational != 0 {
                for _ in 0..n {
                    weights.push(f(at)?);
                    at += 8;
                }
            }
            let m = u32_at(p, at).ok_or("no knot count")? as usize;
            if m > p.len() / 8 {
                return Err(format!("{m} knots at +{pos}"));
            }
            at += 4;
            let mut knots = Vec::with_capacity(m);
            for _ in 0..m {
                knots.push(f(at)?);
                at += 8;
            }
            // A double (-1.0 on every one) and four bytes (04 01 01 00).
            f(at)?;
            p.get(at + 8..at + 12).ok_or("no closing bytes")?;
            Ok((
                Member::Spline {
                    poles,
                    weights,
                    knots,
                },
                at + 12,
            ))
        }
        other => Err(format!("member tag 0x{other:02X} at +{pos}")),
    }
}

/// The whole payload by the grammar in the module comment, or where it
/// stops being that grammar.
fn read_boundary(p: &[u8]) -> Result<Read, String> {
    let owner = u32_at(p, 4).ok_or("short")?;
    let layer = u32_at(p, 8).ok_or("short")?;
    let sub = u16_at(p, 12).ok_or("short")?;
    let style = u32_at(p, 14).ok_or("short")?;
    let loop_count = u32_at(p, 18).ok_or("short")? as usize;
    if loop_count == 0 || loop_count > p.len() {
        return Err(format!("{loop_count} loops"));
    }
    let mut pos = 22usize;
    let mut loops = Vec::new();
    for index in 0..loop_count {
        let count =
            u32_at(p, pos).ok_or_else(|| format!("loop {index}: no member count"))? as usize;
        if count == 0 || count > p.len() {
            return Err(format!("loop {index}: {count} members at +{pos}"));
        }
        let tail = [
            *p.get(pos + 4).ok_or("no loop bytes")?,
            *p.get(pos + 5).ok_or("no loop bytes")?,
        ];
        pos += 6;
        let mut members = Vec::with_capacity(count);
        for m in 0..count {
            let (member, next) =
                read_member(p, pos).map_err(|why| format!("loop {index} member {m}: {why}"))?;
            members.push((pos, member));
            pos = next;
        }
        loops.push(Loop { tail, members });
    }
    let anchor = (
        f64_at(p, pos).ok_or_else(|| format!("no anchor at +{pos}"))?,
        f64_at(p, pos + 8).ok_or("no anchor")?,
    );
    let flag = *p.get(pos + 16).ok_or("no flag byte")?;
    let count = u32_at(p, pos + 17).ok_or("no reference count")? as usize;
    let refs_at = pos + 21;
    if count > p.len() || refs_at + count * 8 != p.len() {
        return Err(format!(
            "{count} reference(s) from +{refs_at} do not end where the payload does, +{}",
            p.len()
        ));
    }
    let refs = (0..count)
        .map(|i| {
            let at = refs_at + i * 8;
            (
                u32_at(p, at).unwrap_or(0),
                u16_at(p, at + 4).unwrap_or(0),
                u16_at(p, at + 6).unwrap_or(0),
            )
        })
        .collect();
    Ok(Read {
        owner,
        layer,
        sub,
        style,
        loops,
        anchor,
        flag,
        refs,
    })
}

fn near(a: P, b: P, tolerance: f64) -> bool {
    (a.0 - b.0).hypot(a.1 - b.1) <= tolerance
}

/// Whether `point` is inside the closed ring by the even-odd rule.
fn contains(ring: &[P], point: P) -> bool {
    let mut inside = false;
    for (i, &(x0, y0)) in ring.iter().enumerate() {
        let (x1, y1) = ring[(i + 1) % ring.len()];
        if (y0 > point.1) != (y1 > point.1) && point.0 < x0 + (point.1 - y0) * (x1 - x0) / (y1 - y0)
        {
            inside = !inside;
        }
    }
    inside
}

fn signed_area(ring: &[P]) -> f64 {
    ring.iter()
        .enumerate()
        .map(|(i, &(x0, y0))| {
            let (x1, y1) = ring[(i + 1) % ring.len()];
            x0 * y1 - x1 * y0
        })
        .sum::<f64>()
        / 2.0
}

/// How a member lies on a record: what it is of the record's curve, and
/// whether it runs the record's own way. `None` when it does not lie on it.
struct Held {
    /// `the whole line`, `a piece of the line`, `the whole arc`, …
    what: &'static str,
    /// `Some(true)` the record's way, `Some(false)` against it, `None` for a
    /// closed curve, which has no way to run.
    forward: Option<bool>,
    detail: String,
}

/// Hold `member` against the live record at `at`, of type `code`.
fn hold(data: &[u8], member: &Member, code: u16, at: usize) -> Option<Held> {
    const TOLERANCE: f64 = 1e-9;
    match (member, code) {
        (Member::Line { start, end }, 0x0018) => {
            let line = decode_igline_at(data, at)?;
            if near(*start, line.start, TOLERANCE) && near(*end, line.end, TOLERANCE) {
                return Some(Held {
                    what: "the whole line",
                    forward: Some(true),
                    detail: format!("bit-exact {}", *start == line.start && *end == line.end),
                });
            }
            if near(*start, line.end, TOLERANCE) && near(*end, line.start, TOLERANCE) {
                return Some(Held {
                    what: "the whole line",
                    forward: Some(false),
                    detail: format!("bit-exact {}", *start == line.end && *end == line.start),
                });
            }
            // A piece of it: both ends on the line, between its ends.
            let d = (line.end.0 - line.start.0, line.end.1 - line.start.1);
            let length = d.0.hypot(d.1);
            let along = |p: P| {
                ((p.0 - line.start.0) * d.0 + (p.1 - line.start.1) * d.1) / (length * length)
            };
            let off =
                |p: P| ((p.0 - line.start.0) * d.1 - (p.1 - line.start.1) * d.0).abs() / length;
            let (from, to) = (along(*start), along(*end));
            let within = |t: f64| (-1e-6..=1.0 + 1e-6).contains(&t);
            (off(*start) <= 1e-8 && off(*end) <= 1e-8 && within(from) && within(to) && from != to)
                .then(|| Held {
                    what: "a piece of the line",
                    forward: Some(to > from),
                    detail: format!("from {from:.6} to {to:.6} of its length"),
                })
        }
        (
            Member::Arc {
                start,
                end,
                center,
                major,
                ratio,
                flag,
            },
            0x0061,
        ) => {
            let arc = decode_igarc_at(data, at)?;
            let same_circle = near(*center, arc.center, TOLERANCE)
                && (major.0.hypot(major.1) - arc.radius).abs() <= TOLERANCE
                && (*ratio - 1.0).abs() <= 1e-12;
            if !same_circle {
                return None;
            }
            let on = |angle: f64| {
                (
                    arc.center.0 + arc.radius * angle.cos(),
                    arc.center.1 + arc.radius * angle.sin(),
                )
            };
            let own = (
                ellipse_at(*center, *major, *ratio, *start),
                ellipse_at(*center, *major, *ratio, *end),
            );
            let theirs = (on(arc.start_angle), on(arc.end_angle));
            let (turned, their_turn) = (
                arc_sweep(*start, *end, *flag),
                arc_sweep(arc.start_angle, arc.end_angle, arc.flag),
            );
            let detail = format!(
                "{start:.6} -> {end:.6} direction {flag}, major ({:.6e}, {:.6e}); the record \
                 {:.6} -> {:.6} direction {}, radius {:.6e}",
                major.0, major.1, arc.start_angle, arc.end_angle, arc.flag, arc.radius
            );
            if near(own.0, theirs.0, 1e-8)
                && near(own.1, theirs.1, 1e-8)
                && (turned - their_turn).abs() < 1e-6
            {
                Some(Held {
                    what: "the whole arc",
                    forward: Some(true),
                    detail,
                })
            } else if near(own.0, theirs.1, 1e-8)
                && near(own.1, theirs.0, 1e-8)
                && (turned + their_turn).abs() < 1e-6
            {
                Some(Held {
                    what: "the whole arc",
                    forward: Some(false),
                    detail,
                })
            } else {
                None
            }
        }
        (
            Member::Arc {
                start,
                end,
                center,
                major,
                ratio,
                flag,
            },
            0x007E,
        ) => {
            let arc = decode_igellipticalarc_at(data, at)?;
            let same_ellipse = near(*center, arc.center, TOLERANCE)
                && near(*major, arc.major_axis, TOLERANCE)
                && (*ratio - arc.ratio).abs() <= 1e-12;
            if !same_ellipse {
                return None;
            }
            let on = |t: f64| ellipse_at(arc.center, arc.major_axis, arc.ratio, t);
            let own = (on(*start), on(*end));
            let theirs = (on(arc.sweep_start), on(arc.sweep_end));
            let (turned, their_turn) = (
                arc_sweep(*start, *end, *flag),
                arc_sweep(arc.sweep_start, arc.sweep_end, arc.flag),
            );
            let detail = format!(
                "{start:.6} -> {end:.6} direction {flag}; the record {:.6} -> {:.6} direction \
                 {}, major ({:.6e}, {:.6e}), ratio {}",
                arc.sweep_start,
                arc.sweep_end,
                arc.flag,
                arc.major_axis.0,
                arc.major_axis.1,
                arc.ratio
            );
            if near(own.0, theirs.0, 1e-8)
                && near(own.1, theirs.1, 1e-8)
                && (turned - their_turn).abs() < 1e-6
            {
                Some(Held {
                    what: "the whole elliptical arc",
                    forward: Some(true),
                    detail,
                })
            } else if near(own.0, theirs.1, 1e-8)
                && near(own.1, theirs.0, 1e-8)
                && (turned + their_turn).abs() < 1e-6
            {
                Some(Held {
                    what: "the whole elliptical arc",
                    forward: Some(false),
                    detail,
                })
            } else {
                None
            }
        }
        (
            Member::Spline {
                poles,
                weights,
                knots,
            },
            0x005D,
        ) => {
            let curve = decode_igbspcurve_at(data, at)?;
            // The breaks of a knot vector: its distinct values.
            let breaks = |knots: &[f64]| {
                let mut out: Vec<f64> = Vec::new();
                for knot in knots {
                    if out.last().is_none_or(|last| (knot - last).abs() > 1e-12) {
                        out.push(*knot);
                    }
                }
                out
            };
            let (own_breaks, their_breaks) = (breaks(knots), breaks(&curve.knots));
            let mirrored: Vec<f64> = {
                let (low, high) = (
                    their_breaks.first().copied().unwrap_or(0.0),
                    their_breaks.last().copied().unwrap_or(0.0),
                );
                their_breaks.iter().rev().map(|b| low + high - b).collect()
            };
            let same = |a: &[f64], b: &[f64]| {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-9)
            };
            // With the same breaks, `sample` evaluates both at the same
            // parameters: the same curve is the same points.
            let own = pid_parse::bspline::sample(poles, weights, knots, 16);
            let mut theirs =
                pid_parse::bspline::sample(&curve.poles, &curve.weights, &curve.knots, 16);
            let forward = if same(&own_breaks, &their_breaks) {
                true
            } else if same(&own_breaks, &mirrored) {
                theirs.reverse();
                false
            } else {
                return None;
            };
            if own.len() != theirs.len() {
                return None;
            }
            let worst = own
                .iter()
                .zip(&theirs)
                .map(|(a, b)| (a.0 - b.0).hypot(a.1 - b.1))
                .fold(0.0f64, f64::max);
            (worst <= 1e-9).then(|| Held {
                what: "the whole B-spline",
                forward: Some(forward),
                detail: format!(
                    "{} poles and {} knots for the record's {} and {}; at most {worst:.3e} m \
                     from it at equal parameters",
                    poles.len(),
                    knots.len(),
                    curve.poles.len(),
                    curve.knots.len()
                ),
            })
        }
        (
            Member::Ellipse {
                center,
                major,
                ratio,
                flag,
            },
            0x0059,
        ) => {
            let circle = decode_igcircle_at(data, at)?;
            (near(*center, circle.center, TOLERANCE)
                && (major.0.hypot(major.1) - circle.radius).abs() <= TOLERANCE
                && (*ratio - 1.0).abs() <= 1e-12)
                .then(|| Held {
                    what: "the whole circle",
                    forward: None,
                    detail: format!("the member's byte {flag}, the record's {}", circle.flag),
                })
        }
        (
            Member::Ellipse {
                center,
                major,
                ratio,
                ..
            },
            0x0063,
        ) => {
            let ellipse = decode_igellipse_at(data, at)?;
            (near(*center, ellipse.center, TOLERANCE)
                && near(*major, ellipse.major_axis, TOLERANCE)
                && (*ratio - ellipse.ratio).abs() <= 1e-12)
                .then(|| Held {
                    what: "the whole ellipse",
                    forward: None,
                    detail: String::new(),
                })
        }
        (Member::Run { points }, 0x0084) => {
            let run = decode_iglinestring_at(data, at)?;
            let same = |theirs: &mut dyn Iterator<Item = &P>| {
                points.len() == run.vertices.len()
                    && points
                        .iter()
                        .zip(theirs)
                        .all(|(a, b)| near(*a, *b, TOLERANCE))
            };
            if same(&mut run.vertices.iter()) {
                Some(Held {
                    what: "the whole line string",
                    forward: Some(true),
                    detail: String::new(),
                })
            } else if same(&mut run.vertices.iter().rev()) {
                Some(Held {
                    what: "the whole line string",
                    forward: Some(false),
                    detail: String::new(),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Default)]
struct Totals {
    documents: usize,
    boundaries: usize,
    library_decodes: usize,
    read: usize,
    sub_types: BTreeMap<u16, usize>,
    flags: BTreeMap<u8, usize>,
    tags: BTreeMap<u8, usize>,
    conics: BTreeMap<String, usize>,
    loop_bytes: BTreeMap<String, usize>,
    references: BTreeMap<&'static str, usize>,
    classes: BTreeMap<(u16, u16), usize>,
    held: BTreeMap<String, usize>,
    last_words: BTreeMap<(u16, String), usize>,
    anchors: BTreeMap<String, usize>,
    open_loops: Vec<String>,
    other_flags: Vec<String>,
    worst_spline: f64,
    /// The fill's edge against the member's own curve, by kind of conic: the
    /// farthest a point of the curve stands from the segments the library
    /// flattened the member to, the major semi-axis of that member (the
    /// radius, when it is a circle's), where, and the width in metres of
    /// the pen the record it names is stroked with.
    chord_gaps: BTreeMap<&'static str, (f64, f64, String, Option<f64>)>,
    /// The same gap against the pen, over the conic members whose record
    /// resolves a pen: how many, how many lie within half the pen's width
    /// of their curve, and the widest gap as a share of that half width.
    pens: BTreeMap<&'static str, (usize, usize, f64)>,
    /// Members that name a record a sheet draws as a polyline, by that
    /// record's family (`igEllipticalArc2d`, `igBspCurve2d`): how many, how
    /// many the library flattened to the very vertices of that polyline,
    /// and the farthest one of its vertices stands from the fill's edge.
    stroked: BTreeMap<&'static str, (usize, usize, f64)>,
    /// The area a record fills against the area inside its members' own
    /// curves, by the kind of conic the record holds: how many records, the
    /// widest difference as a share of the latter (negative when the fill
    /// is short), both areas in m², and where.
    fill_short: BTreeMap<&'static str, (usize, f64, f64, f64, String)>,
    /// Records with a conic member whose loop does not chain, so that there
    /// is no area to compare.
    fill_not_compared: Vec<String>,
    /// Live boundaries in symbol caches, and how many the grammar reads.
    cached: usize,
    cached_read: usize,
    cached_tags: BTreeMap<u8, usize>,
    cached_conics: BTreeMap<String, usize>,
    cached_loops: BTreeMap<usize, usize>,
    cached_unread: Vec<String>,
    /// What fails the probe.
    faults: Vec<String>,
}

fn walk_document(
    label: &str,
    package: &pid_parse::PidPackage,
    verbose: Option<&str>,
    t: &mut Totals,
) {
    t.documents += 1;
    let name = Path::new(label)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let show = verbose.is_some_and(|part| name.contains(part));
    // The pens of the document's records, resolved when a member asks.
    let mut styles: Option<pid_parse::style_link::LineStyleIndex> = None;
    for (stream, raw) in &package.streams {
        let leaf = stream.rsplit('/').next().unwrap_or_default();
        if leaf == "PSMcluster0" && stream.starts_with("/JSite") {
            // A symbol cache's boundaries are read by another reader of the
            // library (`decode_igsym_boundary_payload`); here only whether
            // the same grammar closes their payloads.
            let data = raw.data.as_slice();
            for at in sheet_record_starts(data) {
                let Some(header) = parse_psm_header(data, at) else {
                    continue;
                };
                if header.type_code != 0x0013
                    || data.get(at + 1).is_some_and(|high| high & 0x80 != 0)
                {
                    continue;
                }
                let end = header.body_start + header.bytes_to_follow as usize;
                let Some(payload) = data.get(header.body_start..end) else {
                    continue;
                };
                t.cached += 1;
                match read_boundary(payload) {
                    Ok(read) => {
                        t.cached_read += 1;
                        for lp in &read.loops {
                            for (_, member) in &lp.members {
                                *t.cached_tags.entry(member.tag()).or_default() += 1;
                                if let Member::Arc { major, ratio, .. }
                                | Member::Ellipse { major, ratio, .. } = member
                                {
                                    *t.cached_conics
                                        .entry(format!(
                                            "0x{:02X} {}",
                                            member.tag(),
                                            conic_shape(*major, *ratio)
                                        ))
                                        .or_default() += 1;
                                }
                            }
                        }
                        *t.cached_loops.entry(read.loops.len()).or_default() += 1;
                    }
                    Err(why) => {
                        if t.cached_unread.len() < 12 {
                            t.cached_unread
                                .push(format!("{name} {stream} +{at}: {why}"));
                        }
                    }
                }
            }
            continue;
        }
        if !leaf.starts_with("Sheet") {
            continue;
        }
        let data = raw.data.as_slice();
        let starts = sheet_record_starts(data);
        // The live records of the chain, by oid.
        let mut live: BTreeMap<u32, Vec<(u16, usize)>> = BTreeMap::new();
        for &at in &starts {
            let Some(header) = parse_psm_header(data, at) else {
                continue;
            };
            if data.get(at + 1).is_some_and(|high| high & 0x80 != 0) {
                continue;
            }
            let end = header.body_start + header.bytes_to_follow as usize;
            if let Some(oid) = data.get(header.body_start..end).and_then(|p| u32_at(p, 0)) {
                live.entry(oid).or_default().push((header.type_code, at));
            }
        }
        for &at in &starts {
            let Some(header) = parse_psm_header(data, at) else {
                continue;
            };
            if header.type_code != 0x0013 || data.get(at + 1).is_some_and(|high| high & 0x80 != 0) {
                continue;
            }
            let end = header.body_start + header.bytes_to_follow as usize;
            let Some(payload) = data.get(header.body_start..end) else {
                continue;
            };
            t.boundaries += 1;
            let site = format!(
                "{name} {stream} +{at} oid {}",
                u32_at(payload, 0).unwrap_or(0)
            );
            let library = decode_igboundary_at(data, at);
            if library.is_some() {
                t.library_decodes += 1;
            } else {
                t.faults.push(format!("{site}: the library refuses it"));
            }
            let read = match read_boundary(payload) {
                Ok(read) => read,
                Err(why) => {
                    t.faults
                        .push(format!("{site}: {} bytes, not read: {why}", payload.len()));
                    continue;
                }
            };
            t.read += 1;
            *t.sub_types.entry(read.sub).or_default() += 1;
            *t.flags.entry(read.flag).or_default() += 1;
            if read.flag != 1 {
                t.other_flags
                    .push(format!("{site}: flag byte {}", read.flag));
            }
            let members: Vec<&(usize, Member)> =
                read.loops.iter().flat_map(|l| l.members.iter()).collect();
            let relation = match read.refs.len().cmp(&members.len()) {
                _ if read.refs.is_empty() => "no reference",
                std::cmp::Ordering::Equal => "as many references as members",
                std::cmp::Ordering::Less => "fewer references than members",
                std::cmp::Ordering::Greater => "more references than members",
            };
            *t.references.entry(relation).or_default() += 1;

            // Each loop as a ring; how far its members are from running on
            // from one another.
            let mut rings: Vec<Vec<P>> = Vec::new();
            let mut open = Vec::new();
            let mut widest_gap = 0.0f64;
            for (index, lp) in read.loops.iter().enumerate() {
                let mut ring: Vec<P> = Vec::new();
                let mut gap = 0.0f64;
                let mut previous: Option<P> = None;
                for (_, member) in &lp.members {
                    let points = member.points();
                    if let (Some(previous), Some(first)) = (previous, points.first()) {
                        gap = gap.max((previous.0 - first.0).hypot(previous.1 - first.1));
                    }
                    previous = points.last().copied();
                    ring.extend(points.iter().take(points.len().saturating_sub(1)).copied());
                    *t.tags.entry(member.tag()).or_default() += 1;
                    if let Member::Arc { major, ratio, .. } | Member::Ellipse { major, ratio, .. } =
                        member
                    {
                        *t.conics
                            .entry(format!(
                                "0x{:02X} {}",
                                member.tag(),
                                conic_shape(*major, *ratio)
                            ))
                            .or_default() += 1;
                    }
                }
                if let (Some(last), Some(first)) = (previous, ring.first()) {
                    gap = gap.max((last.0 - first.0).hypot(last.1 - first.1));
                }
                if gap > 1e-9 {
                    open.push(format!("loop {index} by {gap:.3e} m"));
                }
                widest_gap = widest_gap.max(gap);
                let turn = if signed_area(&ring) > 0.0 {
                    "counter-clockwise"
                } else {
                    "clockwise"
                };
                *t.loop_bytes
                    .entry(format!(
                        "{:02X} {:02X}  {} loop, {}",
                        lp.tail[0],
                        lp.tail[1],
                        if index == 0 { "first" } else { "later" },
                        if lp.members.len() == 1 {
                            format!("one 0x{:02X} member", lp.members[0].1.tag())
                        } else {
                            turn.to_string()
                        }
                    ))
                    .or_default() += 1;
                rings.push(ring);
            }
            if !open.is_empty() {
                t.open_loops.push(format!("{site}: {}", open.join(", ")));
            }

            // What the record fills — the ring the library flattens it to —
            // against the area inside its members' own curves. Only where
            // the members run on from one another to within a tenth of a
            // millimetre: a loop that does not chain encloses no area to
            // compare (`D04`'s one such record leaves 2.6 mm between two
            // members).
            let conics: Vec<bool> = members
                .iter()
                .filter_map(|(_, member)| match member {
                    Member::Arc { major, ratio, .. } | Member::Ellipse { major, ratio, .. } => {
                        Some(major.1 == 0.0 && *ratio == 1.0)
                    }
                    _ => None,
                })
                .collect();
            let mut fills_line = None;
            if !conics.is_empty() && widest_gap > 1e-4 {
                t.fill_not_compared.push(format!(
                    "{site}: its members leave {:.3} mm between two of them",
                    widest_gap * 1e3
                ));
            } else if let (false, Some(decoded)) = (conics.is_empty(), library.as_ref()) {
                let own: Vec<Vec<P>> = read
                    .loops
                    .iter()
                    .map(|lp| {
                        lp.members
                            .iter()
                            .flat_map(|(_, member)| {
                                let mut points = member.curve(4096);
                                points.pop();
                                points
                            })
                            .collect()
                    })
                    .collect();
                let filled: Vec<Vec<P>> = decoded
                    .loops()
                    .map(|segments| segments.iter().map(|segment| segment.start).collect())
                    .collect();
                let (own, filled) = (fill_area(&own), fill_area(&filled));
                let kind = if conics.iter().all(|circle| *circle) {
                    "a record whose conic members are all a circle's"
                } else {
                    "a record that holds the arc of an ellipse"
                };
                let share = (filled - own) / own;
                let entry = t
                    .fill_short
                    .entry(kind)
                    .or_insert((0, 0.0, 0.0, 0.0, String::new()));
                entry.0 += 1;
                if share.abs() > entry.1.abs() {
                    (entry.1, entry.2, entry.3, entry.4) = (share, filled, own, site.clone());
                }
                fills_line = Some(format!(
                    "fills {:.6} mm², {:+.4}% against the {:.6} mm² inside its members' own \
                     curves",
                    filled * 1e6,
                    share * 1e2,
                    own * 1e6
                ));
            }

            // The anchor: the placeholder on a record that names no member,
            // a point inside the fill on one that does.
            let placeholder = read.anchor == (1e30, 1e30);
            let around = rings
                .iter()
                .filter(|ring| contains(ring, read.anchor))
                .count();
            let anchor = if placeholder {
                "(1e30, 1e30)".to_string()
            } else if around % 2 == 1 {
                "inside the fill".to_string()
            } else {
                format!("OUTSIDE the fill ({around} loop(s) around it)")
            };
            *t.anchors
                .entry(format!("{relation}: {anchor}"))
                .or_default() += 1;
            if read.refs.is_empty() != placeholder || (!placeholder && around % 2 != 1) {
                t.faults.push(format!(
                    "{site}: {relation}, and the anchor ({:.6}, {:.6}) mm is {anchor}",
                    read.anchor.0 * 1e3,
                    read.anchor.1 * 1e3
                ));
            }

            if show {
                println!(
                    "  {site}: {} bytes, owner {}, layer {}, sub-type 0x{:04X}, style {}, {} \
                     loop(s), {} member(s), {} reference(s), flag {}, anchor ({:.6}, {:.6}) mm \
                     {anchor}; the library {}",
                    payload.len(),
                    read.owner,
                    read.layer,
                    read.sub,
                    read.style,
                    read.loops.len(),
                    members.len(),
                    read.refs.len(),
                    read.flag,
                    read.anchor.0 * 1e3,
                    read.anchor.1 * 1e3,
                    library.as_ref().map_or_else(
                        || "REFUSES it".to_string(),
                        |decoded| format!(
                            "decodes it: {} segment(s), anchor ({:.6}, {:.6}) mm",
                            decoded.segments.len(),
                            decoded.anchor.0 * 1e3,
                            decoded.anchor.1 * 1e3
                        )
                    ),
                );
                if !open.is_empty() {
                    println!("      does not close: {}", open.join(", "));
                }
                if let Some(line) = &fills_line {
                    println!("      {line}");
                }
            }

            // Every reference names one live record; the class word says of
            // what family.
            let mut named: Vec<Option<(u16, usize)>> = Vec::with_capacity(read.refs.len());
            for &(oid, class, _) in &read.refs {
                match live.get(&oid).map(Vec::as_slice) {
                    Some([(code, record_at)]) => {
                        *t.classes.entry((class, *code)).or_default() += 1;
                        named.push(Some((*code, *record_at)));
                    }
                    other => {
                        t.faults.push(format!(
                            "{site}: reference oid {oid} names {} live record(s)",
                            other.map_or(0, <[_]>::len)
                        ));
                        named.push(None);
                    }
                }
            }
            // Each member against the reference of its own place, then
            // against the others.
            for (index, (tag_at, member)) in members.iter().enumerate() {
                let try_reference = |slot: usize| {
                    named
                        .get(slot)
                        .copied()
                        .flatten()
                        .and_then(|(code, record_at)| hold(data, member, code, record_at))
                        .map(|held| (slot, held))
                };
                let found = try_reference(index).or_else(|| {
                    (0..read.refs.len())
                        .filter(|slot| *slot != index)
                        .find_map(try_reference)
                });
                // What the library made of the member: the segments of the
                // decoded record that carry the member's tag offset.
                let flattened: Vec<P> = library.as_ref().map_or_else(Vec::new, |decoded| {
                    let mut vertices = Vec::new();
                    for segment in decoded
                        .segments
                        .iter()
                        .filter(|segment| segment.tag_offset == *tag_at)
                    {
                        if vertices.is_empty() {
                            vertices.push(segment.start);
                        }
                        vertices.push(segment.end);
                    }
                    vertices
                });
                if let (
                    true,
                    Member::Arc { major, ratio, .. } | Member::Ellipse { major, ratio, .. },
                ) = (flattened.len() >= 2, member)
                {
                    // The fill's edge against the curve itself.
                    let gap = member
                        .curve(4096)
                        .iter()
                        .map(|point| distance_to_polyline(*point, &flattened))
                        .fold(0.0f64, f64::max);
                    let circle = major.1 == 0.0 && *ratio == 1.0;
                    let kind = if circle {
                        "the arc of a circle"
                    } else {
                        "the arc of an ellipse"
                    };
                    // The pen the record this member names is stroked with.
                    let pen = found.as_ref().and_then(|(slot, _)| {
                        styles
                            .get_or_insert_with(|| {
                                pid_parse::style_link::line_styles_for_document(&package.parsed)
                            })
                            .get(&(stream.clone(), read.refs[*slot].0))
                            .map(|style| style.symbology.width_m)
                    });
                    let entry = t
                        .chord_gaps
                        .entry(kind)
                        .or_insert((0.0, 0.0, String::new(), None));
                    if gap > entry.0 {
                        *entry = (
                            gap,
                            major.0.hypot(major.1),
                            format!("{site} member {index}"),
                            pen,
                        );
                    }
                    if let Some(pen) = pen.filter(|pen| *pen > 0.0) {
                        let entry = t.pens.entry(kind).or_insert((0, 0, 0.0));
                        entry.0 += 1;
                        if gap <= pen / 2.0 {
                            entry.1 += 1;
                        } else {
                            t.faults.push(format!(
                                "{site}: member {index} is flattened {:.4} mm off its curve, \
                                 past half the {:.4} mm pen its record is drawn with",
                                gap * 1e3,
                                pen * 1e3
                            ));
                        }
                        entry.2 = entry.2.max(gap / (pen / 2.0));
                    }
                }
                if let Some((slot, held)) = &found {
                    // A member whose record a sheet draws as a polyline — an
                    // arc of an ellipse, a B-spline — against that polyline:
                    // the curve as `geometry.rs` samples it.
                    let drawn = named.get(*slot).copied().flatten().and_then(
                        |(code, record_at)| match code {
                            0x007E => decode_igellipticalarc_at(data, record_at)
                                .and_then(|arc| {
                                    pid_parse::bspline::elliptical_arc_directed(
                                        arc.center,
                                        arc.major_axis,
                                        arc.ratio,
                                        arc.sweep_start,
                                        arc.sweep_end,
                                        arc.flag == 0,
                                    )
                                })
                                .map(|curve| ("igEllipticalArc2d", curve)),
                            0x005D => decode_igbspcurve_at(data, record_at).map(|curve| {
                                ("igBspCurve2d", (curve.poles, curve.weights, curve.knots))
                            }),
                            _ => None,
                        },
                    );
                    if let Some((family, (poles, weights, knots))) = drawn {
                        let mut stroke = pid_parse::bspline::sample(
                            &poles,
                            &weights,
                            &knots,
                            pid_parse::bspline::SEGMENTS_PER_SPAN,
                        );
                        if held.forward == Some(false) {
                            stroke.reverse();
                        }
                        let entry = t.stroked.entry(family).or_insert((0, 0, 0.0));
                        entry.0 += 1;
                        if stroke.len() == flattened.len()
                            && stroke
                                .iter()
                                .zip(&flattened)
                                .all(|(a, b)| near(*a, *b, 1e-12))
                        {
                            entry.1 += 1;
                        } else {
                            t.faults.push(format!(
                                "{site}: member {index} is not flattened to the vertices its \
                                 {family} is drawn as"
                            ));
                        }
                        if flattened.len() >= 2 {
                            let off = stroke
                                .iter()
                                .map(|point| distance_to_polyline(*point, &flattened))
                                .fold(0.0f64, f64::max);
                            entry.2 = entry.2.max(off);
                        }
                    }
                }
                let line = match &found {
                    None if read.refs.is_empty() => "names no member".to_string(),
                    None => {
                        t.faults.push(format!(
                            "{site}: member {index} (0x{:02X}) lies on no record the boundary \
                             names",
                            member.tag()
                        ));
                        "ON NO RECORD THE BOUNDARY NAMES".to_string()
                    }
                    Some((slot, held)) => {
                        let (oid, class, last_word) = read.refs[*slot];
                        let turn = if *slot == index {
                            "the reference of its own place"
                        } else {
                            "another reference"
                        };
                        *t.held
                            .entry(format!("{}, by {turn}", held.what))
                            .or_default() += 1;
                        if let Member::Spline { .. } = member {
                            if let Some(worst) = held
                                .detail
                                .split("at most ")
                                .nth(1)
                                .and_then(|rest| rest.split(' ').next())
                                .and_then(|value| value.parse::<f64>().ok())
                            {
                                t.worst_spline = t.worst_spline.max(worst);
                            }
                        }
                        let way = match held.forward {
                            Some(true) => "the record's way",
                            Some(false) => "against the record",
                            None => "a closed curve",
                        };
                        *t.last_words
                            .entry((
                                last_word,
                                format!(
                                    "{} reference, a member of {} loop, {way}",
                                    if *slot == 0 { "first" } else { "later" },
                                    if index >= read.loops[0].members.len() {
                                        "a later"
                                    } else {
                                        "the first"
                                    }
                                ),
                            ))
                            .or_default() += 1;
                        // Bit 2: on a record's first loop, the member runs the
                        // way its record does; on a later loop — the hole of
                        // a fill — the other way round. Bit 0: the first
                        // reference of the record.
                        let later_loop = index >= read.loops[0].members.len();
                        if held
                            .forward
                            .is_some_and(|forward| (forward != later_loop) != (last_word & 4 != 0))
                        {
                            t.faults.push(format!(
                                "{site}: member {index} runs {way} under the word {last_word}"
                            ));
                        }
                        if (last_word & 1 != 0) != (*slot == 0) {
                            t.faults.push(format!(
                                "{site}: reference {slot} carries the word {last_word}"
                            ));
                        }
                        format!(
                            "{} of oid {oid} (class 0x{class:04X}, word {last_word}), {way}, by \
                             {turn}; {}",
                            held.what, held.detail
                        )
                    }
                };
                if show {
                    let points = member.points();
                    let (first, last) = (points[0], points[points.len() - 1]);
                    println!(
                        "      [{index:>3}] +{tag_at:<5} 0x{:02X} ({:.6}, {:.6}) -> ({:.6}, \
                         {:.6}) mm, {} segment(s) of the fill's ring: {line}",
                        member.tag(),
                        first.0 * 1e3,
                        first.1 * 1e3,
                        last.0 * 1e3,
                        last.1 * 1e3,
                        flattened.len().saturating_sub(1),
                    );
                }
            }
        }
    }
}

/// The `.pid` / `.igr` files under `dir` and the archives whose members are
/// read beside them.
fn documents_under(dir: &Path, files: &mut Vec<PathBuf>, archives: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            // A machine-local symbol library a checkout may link in.
            if path.file_name().is_some_and(|name| name == "symbols-full") {
                continue;
            }
            documents_under(&path, files, archives);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pid") || e.eq_ignore_ascii_case("igr"))
        {
            files.push(path);
        } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            (n.starts_with("RefData~") || n.starts_with("PlantData~")) && n.ends_with(".zip")
        }) {
            archives.push(path);
        }
    }
}

/// A file's length and a hash of its bytes: what tells two copies of one
/// document from two documents.
fn fingerprint(bytes: &[u8]) -> (usize, u64) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    (bytes.len(), hasher.finish())
}

fn main() {
    let mut verbose: Option<String> = None;
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--verbose" {
            verbose = args.next();
        } else {
            dirs.push(PathBuf::from(arg));
        }
    }
    if dirs.is_empty() {
        dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file"));
        if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
            dirs.push(PathBuf::from(dir));
        }
        dirs.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
        dirs.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
        dirs.push(PathBuf::from(r"D:\work\cad\PID自研软件验证"));
    }

    let mut t = Totals::default();
    let mut seen: BTreeSet<(usize, u64)> = BTreeSet::new();
    for dir in &dirs {
        if !dir.is_dir() {
            println!("{} is absent; skipped", dir.display());
            continue;
        }
        let mut files = Vec::new();
        let mut archives = Vec::new();
        documents_under(dir, &mut files, &mut archives);
        let before = t.documents;
        for path in &files {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            if !seen.insert(fingerprint(&bytes)) {
                continue;
            }
            match pid_parse::PidPackage::from_bytes(&bytes) {
                Ok(package) => walk_document(
                    &path.display().to_string(),
                    &package,
                    verbose.as_deref(),
                    &mut t,
                ),
                Err(error) => println!("  {} does not parse: {error}", path.display()),
            }
        }
        for archive in &archives {
            for extension in ["igr", "pid"] {
                let Ok(members) = pid_parse::refdata_archive::archive_members(archive, extension)
                else {
                    continue;
                };
                for member in members {
                    let label = member.label().display().to_string();
                    let Ok(bytes) = pid_parse::refdata_archive::read_archive_file(&member) else {
                        continue;
                    };
                    if !seen.insert(fingerprint(&bytes)) {
                        continue;
                    }
                    match pid_parse::PidPackage::from_bytes(&bytes) {
                        Ok(package) => walk_document(&label, &package, verbose.as_deref(), &mut t),
                        Err(error) => println!("  {label} does not parse: {error}"),
                    }
                }
            }
        }
        println!(
            "{}: {} file(s), {} archive(s), {} document(s) not read before",
            dir.display(),
            files.len(),
            archives.len(),
            t.documents - before
        );
    }

    println!(
        "\n{} distinct document(s); {} live igBoundary2d on their sheet chains; the grammar \
         reads {} to the last byte; the library decodes {}",
        t.documents, t.boundaries, t.read, t.library_decodes
    );
    println!(
        "sub-type words: {}",
        t.sub_types
            .iter()
            .map(|(word, count)| format!("0x{word:04X} x {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("flag bytes: {:?}", t.flags);
    for line in &t.other_flags {
        println!("  {line}");
    }
    println!(
        "members: {}",
        t.tags
            .iter()
            .map(|(tag, count)| format!("0x{tag:02X} x {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("conic members by shape:");
    for (shape, count) in &t.conics {
        println!("  {count:>6}  {shape}");
    }
    println!("the edge the library flattens a conic member to, against the member's own curve:");
    for (kind, (gap, semi_axis, site, pen)) in &t.chord_gaps {
        println!(
            "  {kind}: at most {:.4} mm off its curve, on one whose major semi-axis is {:.4} mm \
             ({site}; {})",
            gap * 1e3,
            semi_axis * 1e3,
            pen.map_or_else(
                || "it names no record whose pen resolves".to_string(),
                |pen| format!("its record is drawn with a {:.4} mm pen", pen * 1e3)
            )
        );
    }
    for (kind, (count, within, share)) in &t.pens {
        println!(
            "  {kind}, where its record resolves a pen ({count}): {within} within half the \
             pen's width of the curve, at most {share:.3} of that half width"
        );
    }
    println!("members whose record a sheet draws as a polyline:");
    for (family, (count, same, off)) in &t.stroked {
        println!(
            "  {count:>6}  name an {family}; flattened to the very vertices of that polyline: \
             {same}; one of its vertices stands at most {:.4} mm from the fill's edge",
            off * 1e3
        );
    }
    println!("what a record fills, against the area inside its members' own curves:");
    for (kind, (count, share, filled, own, site)) in &t.fill_short {
        println!(
            "  {kind} ({count}): at most {:+.4}% ({:.6} mm² for {:.6} mm², {site})",
            share * 1e2,
            filled * 1e6,
            own * 1e6
        );
    }
    for line in &t.fill_not_compared {
        println!("  not compared: {line}");
    }
    println!("the two bytes after a loop's member count:");
    for (bytes, count) in &t.loop_bytes {
        println!("  {count:>6}  {bytes}");
    }
    println!("references against members:");
    for (relation, count) in &t.references {
        println!("  {count:>6}  {relation}");
    }
    println!("a reference's class word, by the type code of the record it names:");
    for ((class, code), count) in &t.classes {
        println!("  {count:>6}  0x{class:04X} names a 0x{code:04X}");
    }
    println!("each member on a record its boundary names:");
    for (what, count) in &t.held {
        println!("  {count:>6}  {what}");
    }
    println!(
        "  a B-spline member is at most {:.3e} m from its record's curve at equal parameters",
        t.worst_spline
    );
    println!("the reference's last word:");
    for ((word, what), count) in &t.last_words {
        println!("  {count:>6}  {word:>3} (0b{word:04b})  {what}");
    }
    println!("anchors:");
    for (what, count) in &t.anchors {
        println!("  {count:>6}  {what}");
    }
    println!(
        "loops whose members do not run on from one another within 1e-9 m: {}",
        t.open_loops.len()
    );
    for line in &t.open_loops {
        println!("  {line}");
    }
    println!(
        "\nsymbol caches (/JSite*/PSMcluster0), which the library reads another way: {} live \
         igBoundary2d, of which the grammar reads {} to the last byte; loops a record {:?}; \
         members {}",
        t.cached,
        t.cached_read,
        t.cached_loops,
        t.cached_tags
            .iter()
            .map(|(tag, count)| format!("0x{tag:02X} x {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for (shape, count) in &t.cached_conics {
        println!("  {count:>6}  {shape}");
    }
    for line in &t.cached_unread {
        println!("  not read: {line}");
    }
    if t.faults.is_empty() {
        println!(
            "\nnothing fails: every record is read, decoded, its members are the curves it \
             names, and each curved member is flattened as its record is drawn or within half \
             its pen"
        );
    } else {
        println!("\n{} thing(s) fail:", t.faults.len());
        for fault in &t.faults {
            println!("  {fault}");
        }
        std::process::exit(1);
    }
}
