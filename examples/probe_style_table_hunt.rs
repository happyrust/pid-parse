//! Phase 35-D, fifth iteration: where does the `igTextBox` trailer id point?
//!
//! Iteration 4 showed the trailer id is real and stable -- id 56 is every
//! Chinese string in three unrelated drawings, id 21 is the commonest
//! annotation in all of them -- but it does not match a `JStyleOverride`
//! oid. So the table it indexes is somewhere else. Two cheap places to look
//! before falling back to a heuristic:
//!
//! 1. the other u32 fields of `JStyleOverride` (maybe the oid was simply the
//!    wrong column);
//! 2. the container's own stream list, in case `SmartPlant` keeps a font or
//!    text-style table in a stream nobody has decoded yet.
//!
//! Read-only: no parser, schema, or model change.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{decode_igtextboxes, decode_jstyle_overrides};
use pid_parse::PidPackage;

// "\u{5DE5}\u{827A}..." is the gongyi fixture.
const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
];

const PSM_ENVELOPE_LEN: usize = 6;
const TRAILER_LEN: usize = 12;

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_file_root();

    for rel in FIXTURES {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let pkg = PidPackage::from_path(&path)?;
        println!("\n================ {rel} ================");

        println!("\n-- streams --");
        let mut names: Vec<(&String, usize)> =
            pkg.streams.iter().map(|(k, v)| (k, v.data.len())).collect();
        names.sort_by_key(|(_, len)| std::cmp::Reverse(*len));
        for (name, len) in names.iter().take(40) {
            println!("  {len:>9}  {name}");
        }
        println!("  ... {} streams total", pkg.streams.len());

        let mut ids: BTreeMap<u32, usize> = BTreeMap::new();
        let mut style_u32s: BTreeSet<u32> = BTreeSet::new();
        let mut style_count = 0usize;

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
                if payload.len() < TRAILER_LEN {
                    continue;
                }
                let trailer = &payload[payload.len() - TRAILER_LEN..];
                let Ok(last4) = <[u8; 4]>::try_from(&trailer[8..12]) else {
                    continue;
                };
                *ids.entry(u32::from_be_bytes(last4)).or_default() += 1;
            }
            for style in decode_jstyle_overrides(bytes) {
                style_count += 1;
                for v in [
                    style.oid,
                    style.field_a_u32,
                    style.field_b_u32,
                    style.field_c_u32,
                    style.field_d_u32,
                    style.field_e_u32,
                    style.field_f_u32,
                    style.field_g_u32,
                    u32::from(style.field_h_u16),
                    u32::from(style.field_i_u16),
                ] {
                    style_u32s.insert(v);
                }
            }
        }

        println!("\n-- trailer ids vs every u32 column of {style_count} styles --");
        let mut hits = 0usize;
        for (id, count) in &ids {
            let found = style_u32s.contains(id);
            if found {
                hits += 1;
            }
            println!(
                "  id={id:<6} x{count:<4} {}",
                if found { "present" } else { "absent" }
            );
        }
        println!(
            "  => {hits}/{} ids appear anywhere in a style record",
            ids.len()
        );
    }

    Ok(())
}
