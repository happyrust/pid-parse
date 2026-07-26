//! Phase 35-D, third iteration: sweep every undecoded byte of `igTextBox`.
//!
//! Iterations 1-2 chased a link *out* of the record -- u32 cross-references
//! (`probe_igtextbox_style_link.rs`), then positional pairing against
//! `JStyleOverride` (`probe_igtextbox_style_pairing.rs`) -- and neither is
//! deterministic. Phase 35-C's breakthrough went the other way: the answer
//! sat inside the record, four bytes ahead of a fixed tag. So this probe
//! stops guessing at links and sweeps `igTextBox` itself.
//!
//! The decoder keeps oid/parent/index/text plus 3 trailing f64s, and drops:
//!
//! ```text
//!   payload[8..12]   `remaining_header`  -- parsed, never stored
//!   payload[18..30]  12 bytes the layout comment calls "sub-fields"
//!   last 12 bytes    trailer, after the 3 known f64s
//! ```
//!
//! Both regions are addressable independent of text length -- the head from
//! the front, the tail from the back -- so the sweep reads f64 and f32 at
//! every alignment in each and reports which offsets land in the text-height
//! domain on *every* record. A field that really is the height is plausible
//! everywhere; a coincidence is not.
//!
//! Read-only: no parser, schema, or model change.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::decode_igtextboxes;
use pid_parse::PidPackage;

// "\u{5DE5}\u{827A}..." is the gongyi fixture (Chinese file name kept as
// escapes so this source stays pure ASCII).
const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
];

const PSM_ENVELOPE_LEN: usize = 6;
const HEAD_LEN: usize = 32;
const TAIL_LEN: usize = 36;

// Source unit is the metre. ISO 3098 body text on a P&ID runs 1.8-7mm and a
// title block can reach ~10mm; 0.5..20mm brackets that with room to spare.
const HEIGHT_MIN: f64 = 0.0005;
const HEIGHT_MAX: f64 = 0.02;

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn plausible_height(v: f64) -> bool {
    v.is_finite() && (HEIGHT_MIN..=HEIGHT_MAX).contains(&v)
}

/// Angles a drafter actually uses: axis-aligned, or a quadrant step.
fn plausible_rotation(v: f64) -> bool {
    if !v.is_finite() || v.abs() > std::f64::consts::TAU + 1e-6 {
        return false;
    }
    let steps = v / std::f64::consts::FRAC_PI_2;
    (steps - steps.round()).abs() < 1e-6
}

fn read_f64(bytes: &[u8], at: usize) -> Option<f64> {
    let chunk = bytes.get(at..at + 8)?;
    Some(f64::from_le_bytes(chunk.try_into().ok()?))
}

fn read_f32(bytes: &[u8], at: usize) -> Option<f32> {
    let chunk = bytes.get(at..at + 4)?;
    Some(f32::from_le_bytes(chunk.try_into().ok()?))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Default)]
struct Tally {
    seen: usize,
    height: usize,
    rotation: usize,
    nonzero: usize,
    distinct: BTreeMap<String, usize>,
}

impl Tally {
    fn record(&mut self, value: f64) {
        self.seen += 1;
        if plausible_height(value) {
            self.height += 1;
        }
        if plausible_rotation(value) && value != 0.0 {
            self.rotation += 1;
        }
        if value != 0.0 {
            self.nonzero += 1;
        }
        if self.distinct.len() < 64 {
            *self.distinct.entry(format!("{value:.6}")).or_default() += 1;
        }
    }

    fn top(&self, n: usize) -> String {
        let mut pairs: Vec<_> = self.distinct.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1));
        pairs
            .into_iter()
            .take(n)
            .map(|(v, c)| format!("{v}x{c}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

type SweepKey = (&'static str, &'static str, usize);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_file_root();
    let mut sweep: BTreeMap<SweepKey, Tally> = BTreeMap::new();
    let mut total = 0usize;
    let mut dumped = 0usize;

    for rel in FIXTURES {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let pkg = PidPackage::from_path(&path)?;

        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            for rec in decode_igtextboxes(bytes) {
                let start = rec.byte_range.start + PSM_ENVELOPE_LEN;
                let Some(payload) = bytes.get(start..rec.byte_range.end) else {
                    continue;
                };
                if payload.len() < HEAD_LEN + TAIL_LEN {
                    continue;
                }
                total += 1;

                let head = &payload[..HEAD_LEN];
                let tail = &payload[payload.len() - TAIL_LEN..];

                // A few raw dumps first: a histogram can only confirm a
                // structure somebody already guessed at, and nobody has
                // eyeballed these bytes side by side with their text yet.
                if dumped < 8 {
                    dumped += 1;
                    let shown: String = rec.text.chars().take(24).collect();
                    println!(
                        "\n--- {rel} [{dumped}] text={shown:?} len={}",
                        rec.text_length
                    );
                    println!("    head[0..32]  {}", hex(head));
                    println!("    tail[0..24]  {}", hex(&tail[..24]));
                    println!("    trailer[24..36] {}", hex(&tail[24..]));
                }

                for at in (0..=HEAD_LEN - 8).step_by(2) {
                    if let Some(v) = read_f64(head, at) {
                        sweep.entry(("head", "f64", at)).or_default().record(v);
                    }
                }
                for at in (0..=HEAD_LEN - 4).step_by(2) {
                    if let Some(v) = read_f32(head, at) {
                        sweep
                            .entry(("head", "f32", at))
                            .or_default()
                            .record(f64::from(v));
                    }
                }
                for at in (0..=TAIL_LEN - 8).step_by(2) {
                    if let Some(v) = read_f64(tail, at) {
                        sweep.entry(("tail", "f64", at)).or_default().record(v);
                    }
                }
                for at in (0..=TAIL_LEN - 4).step_by(2) {
                    if let Some(v) = read_f32(tail, at) {
                        sweep
                            .entry(("tail", "f32", at))
                            .or_default()
                            .record(f64::from(v));
                    }
                }
            }
        }
    }

    println!("\n\n=== sweep over {total} igTextBox records ===");
    println!("(tail f64 @0/@8/@16 are the 3 already-decoded doubles)\n");

    println!("-- offsets whose value is a plausible text height on EVERY record --");
    let mut any_full = false;
    for ((region, width, at), tally) in &sweep {
        if tally.seen == total && tally.height == total && total > 0 {
            any_full = true;
            println!(
                "  {region:<4} {width} @{at:<2}  height=100%  nonzero={:<3}  {}",
                tally.nonzero,
                tally.top(6)
            );
        }
    }
    if !any_full {
        println!("  (none)");
    }

    println!("\n-- offsets >=80% plausible height --");
    for ((region, width, at), tally) in &sweep {
        if tally.seen == 0 {
            continue;
        }
        let rate = tally.height * 100 / tally.seen;
        if rate >= 80 && tally.height != total {
            println!(
                "  {region:<4} {width} @{at:<2}  height={rate:>3}%  nonzero={:<3}  {}",
                tally.nonzero,
                tally.top(6)
            );
        }
    }

    println!("\n-- offsets carrying a non-zero quadrant angle (rotation candidates) --");
    for ((region, width, at), tally) in &sweep {
        if tally.rotation == 0 {
            continue;
        }
        println!(
            "  {region:<4} {width} @{at:<2}  quadrant-rot={:<3}/{:<3}  {}",
            tally.rotation,
            tally.seen,
            tally.top(6)
        );
    }

    Ok(())
}
