//! Cross-fixture ratchet for the geometry → line style link.
//!
//! [`pid_parse::style_link`] claims that a geometry record's `index` at
//! payload `+14` names a style id in the `StyleCluster` of its own document,
//! and that the record it names either is a `JStyleSimpleLine` or is a
//! `JStyleOverride` whose `+22` names one.
//!
//! The claim is corpus-grade, so it is pinned by exact counts rather than
//! trusted. Two things would break silently without this test:
//!
//! * a decoder or framing change that quietly drops resolutions — caught by
//!   the per-family totals and the "nothing unresolved" assertion;
//! * a change that resolves the same records to *different* symbology —
//!   caught by the corpus palette, which is asserted whole.
//!
//! The palette is also the readable part: nine entries over 562 records, ISO
//! 128 widths apart from the 0.100 mm point ticks. If a future change makes
//! this list longer or stranger, the framing moved. Retiring the `igLine2d`
//! `aux_hi` rule added four records and no tenth entry — they land on
//! 0.350 mm black, which is what a rectangle outline should weigh.
//!
//! Soft-skips per fixture, so a checkout without `test-file/` still passes.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{decode_iglines, decode_iglinestrings, decode_igpoints};
use pid_parse::style_link::{stylecluster_path_for_sheet, DocumentStyleTable, StyleHop};

/// What one fixture is expected to resolve.
struct Expected {
    fixture: &'static str,
    lines: usize,
    points: usize,
    linestrings: usize,
    direct: usize,
    via_override: usize,
}

const EXPECTED: [Expected; 4] = [
    Expected {
        fixture: "test-file/D06.pid",
        lines: 0,
        points: 10,
        linestrings: 6,
        direct: 16,
        via_override: 0,
    },
    Expected {
        fixture: "test-file/DWG-0201GP06-01.pid",
        lines: 24,
        points: 75,
        linestrings: 39,
        direct: 114,
        via_override: 24,
    },
    // `Sheet6615`'s four `igLine2d` — the sides of a rectangle, beside a
    // `0x0020` Rectangle Object — are in this count now. They were refused
    // for two phases over an `aux_hi` of 6996 rather than 12, and the fact
    // that all four resolve to a style this document defines is one more
    // reading that they were always records.
    Expected {
        fixture: "test-file/DWG-0202GP06-01.pid",
        lines: 46,
        points: 31,
        linestrings: 28,
        direct: 78,
        via_override: 27,
    },
    Expected {
        fixture: "test-file/工艺管道及仪表流程-1.pid",
        lines: 218,
        points: 36,
        linestrings: 49,
        direct: 297,
        via_override: 6,
    },
];

/// The whole corpus palette, as `("<width>mm #RRGGBB", record count)`.
const EXPECTED_PALETTE: [(&str, usize); 9] = [
    ("0.100mm #000000", 107),
    ("0.100mm #0000FF", 18),
    ("0.100mm #008000", 10),
    ("0.130mm #000000", 182),
    ("0.180mm #008000", 7),
    ("0.350mm #000000", 183),
    ("0.350mm #808000", 10),
    ("0.350mm #FE0060", 6),
    ("0.700mm #808000", 39),
];

#[derive(Default)]
struct Tally {
    lines: usize,
    points: usize,
    linestrings: usize,
    direct: usize,
    via_override: usize,
    unresolved: Vec<String>,
    palette: BTreeMap<String, usize>,
}

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut stream = cfb.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).ok()?;
    Some(data)
}

fn tally(path: &Path) -> Option<Tally> {
    let file = std::fs::File::open(path).ok()?;
    let mut cfb = CompoundFile::open(file).ok()?;
    let sheet_paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .filter(|p| p.rsplit('/').next().unwrap_or("").starts_with("Sheet"))
        .collect();

    let mut out = Tally::default();
    for sheet_path in &sheet_paths {
        let Some(sheet) = read_stream(&mut cfb, sheet_path) else {
            continue;
        };
        // The scoping rule: a sheet resolves against its own document.
        let table = read_stream(&mut cfb, &stylecluster_path_for_sheet(sheet_path))
            .map_or_else(DocumentStyleTable::default, |bytes| {
                DocumentStyleTable::from_stylecluster_bytes(&bytes)
            });

        let mut indices: Vec<(&'static str, u32)> = Vec::new();
        for record in decode_iglines(&sheet) {
            out.lines += 1;
            indices.push(("igLine2d", record.index));
        }
        for record in decode_igpoints(&sheet) {
            out.points += 1;
            indices.push(("igPoint2d", record.index));
        }
        for record in decode_iglinestrings(&sheet) {
            out.linestrings += 1;
            indices.push(("igLineString2d", record.index));
        }

        for (family, index) in indices {
            let Some(resolved) = table.resolve_line_style(index) else {
                out.unresolved
                    .push(format!("{sheet_path} {family} index={index}"));
                continue;
            };
            match resolved.hop {
                StyleHop::Direct => out.direct += 1,
                StyleHop::ViaOverride { .. } => out.via_override += 1,
            }
            let width_mm = resolved.symbology.width_mm();
            let [r, g, b] = resolved.symbology.rgb();
            *out.palette
                .entry(format!("{width_mm:.3}mm #{r:02X}{g:02X}{b:02X}"))
                .or_default() += 1;
        }
    }
    Some(out)
}

#[test]
fn every_drawable_record_reaches_a_line_width_and_colour() {
    let mut corpus_palette: BTreeMap<String, usize> = BTreeMap::new();
    let mut corpus_records = 0usize;
    let mut fixtures_seen = 0usize;

    for expected in &EXPECTED {
        let path = Path::new(expected.fixture);
        if !path.exists() {
            eprintln!("skip: {} is absent", expected.fixture);
            continue;
        }
        fixtures_seen += 1;
        let got = tally(path).expect("fixture opens as a compound file");

        assert_eq!(
            got.unresolved,
            Vec::<String>::new(),
            "{}: every index must name a style this document defines",
            expected.fixture
        );
        assert_eq!(got.lines, expected.lines, "{} igLine2d", expected.fixture);
        assert_eq!(
            got.points, expected.points,
            "{} igPoint2d",
            expected.fixture
        );
        assert_eq!(
            got.linestrings, expected.linestrings,
            "{} igLineString2d",
            expected.fixture
        );
        assert_eq!(
            got.direct, expected.direct,
            "{} resolved without an override",
            expected.fixture
        );
        assert_eq!(
            got.via_override, expected.via_override,
            "{} resolved through an override",
            expected.fixture
        );

        let resolved = got.direct + got.via_override;
        assert_eq!(
            resolved,
            expected.lines + expected.points + expected.linestrings,
            "{}: resolutions must account for every record",
            expected.fixture
        );
        corpus_records += resolved;
        for (key, count) in got.palette {
            *corpus_palette.entry(key).or_default() += count;
        }
    }

    if fixtures_seen == 0 {
        eprintln!("skip: no fixture present");
        return;
    }
    if fixtures_seen < EXPECTED.len() {
        // A partial corpus cannot be held to corpus-wide totals.
        return;
    }

    assert_eq!(corpus_records, 562, "records reaching a line style");
    let expected_palette: BTreeMap<String, usize> = EXPECTED_PALETTE
        .iter()
        .map(|(key, count)| ((*key).to_string(), *count))
        .collect();
    assert_eq!(
        corpus_palette, expected_palette,
        "the resolved palette is the load-bearing evidence; a change here means \
         the framing moved, not that the drawings did"
    );
}

/// Text reaches a real character height, through the second hop.
///
/// `igTextBox` names a `JStyleTextPara`, and the height lives on the
/// `JStyleTextChar` that paragraph style names at +38. The heights that fall
/// out are the readable evidence: ISO 3098's 1.5 / 2.5 / 3.5 mm, the imperial
/// 1/16-1/8-1/4 inch steps. A wrong hop would not land on a drafting ladder.
///
/// Not every record resolves, and that is on purpose: some reach a character
/// style whose height reads 0.254 mm, which nobody has explained. The counts
/// are pinned so that number cannot drift unnoticed in either direction.
#[test]
fn text_reaches_the_height_its_character_style_states() {
    let mut heights: BTreeMap<String, usize> = BTreeMap::new();
    let mut fixtures_seen = 0usize;
    for expected in &EXPECTED {
        let path = Path::new(expected.fixture);
        if !path.exists() {
            continue;
        }
        fixtures_seen += 1;
        let resolved =
            pid_parse::style_link::text_heights_for_file(path).expect("fixture opens for text");
        for style in resolved.values() {
            let mm = style.height_mm();
            *heights.entry(format!("{mm:.3}mm")).or_default() += 1;
        }
    }
    if fixtures_seen < EXPECTED.len() {
        return;
    }

    // These are the records `decode_igtextboxes` accepts -- now the whole
    // family, all 260, since the native sub-type layout landed. Reading
    // sub-types 1 and 3 brought 21 more records to a character style and put
    // two sizes on the table that no drawing had shown before: 6.350mm is a
    // quarter inch, the heading size, and 1.524mm is 0.06 inch. See
    // `docs/analysis/2026-08-13-igtextbox-has-three-shapes.md`.
    let expected_heights: BTreeMap<String, usize> = [
        ("1.500mm", 10),
        ("1.524mm", 1),
        ("2.032mm", 1),
        ("2.464mm", 7),
        ("2.500mm", 27),
        ("2.540mm", 2),
        ("3.175mm", 98),
        ("3.500mm", 7),
        ("6.350mm", 2),
    ]
    .iter()
    .map(|(key, count)| ((*key).to_string(), *count))
    .collect();
    assert_eq!(heights, expected_heights);
    // 3.175mm is 1/8 inch and the most common size by far, so the renderer's
    // old fixed 2.5mm was 27% too small for most of a drawing's lettering.
    assert_eq!(heights.values().sum::<usize>(), 155);
}

/// Lettering reaches the colour its character style states, not just its size.
///
/// Every label used to render in its layer's default because nothing read a
/// text colour. `JStyleTextChar +34` holds one, and the corpus is what
/// established that: 381 of 381 records read there with a zero high byte,
/// which an unconstrained `u32` does about once in 256. The palette below is
/// that measurement, pinned -- most lettering is black, and the few stated
/// colours include `#FE0060`, a value the same drawings use for line work.
/// See `docs/analysis/2026-08-13-text-colour-is-002c-plus-34.md`.
#[test]
fn text_reaches_the_colour_its_character_style_states() {
    let mut palette: BTreeMap<String, usize> = BTreeMap::new();
    let mut unstated = 0usize;
    let mut fixtures_seen = 0usize;
    for expected in &EXPECTED {
        let path = Path::new(expected.fixture);
        if !path.exists() {
            continue;
        }
        fixtures_seen += 1;
        let resolved =
            pid_parse::style_link::text_heights_for_file(path).expect("fixture opens for text");
        for style in resolved.values() {
            match style.rgb() {
                Some([r, g, b]) => *palette.entry(format!("#{r:02X}{g:02X}{b:02X}")).or_default() += 1,
                None => unstated += 1,
            }
        }
    }
    if fixtures_seen < EXPECTED.len() {
        return;
    }

    assert_eq!(
        unstated, 0,
        "every character style the corpus reaches states a colour, got palette {palette:?}"
    );
    // The resolved index is narrower than a raw scan of `0x002C`: it holds
    // only the styles a text record actually reaches whose height is usable,
    // so 155 of the corpus's 381 character styles. Nearly all of a P&ID
    // letters in black -- which the renderer flips to white on a dark
    // background, the same as the drawing's black line work -- and two
    // labels are stated red.
    let expected_palette: BTreeMap<String, usize> = [("#000000", 153), ("#FF0000", 2)]
        .iter()
        .map(|(key, count)| ((*key).to_string(), *count))
        .collect();
    assert_eq!(palette, expected_palette);
    assert_eq!(palette.values().sum::<usize>(), 155, "one colour per height");
}

/// A P&ID does not letter everything from the left, and until now every
/// consumer had to assume it did.
///
/// `JStyleTextPara +35` states the side, with the values Intergraph's own
/// `Interop.RAD2D.dll` declares (`igHorizontalTextLeft/Center/Right = 0/1/2`).
/// The counts below are the reason this is worth reading: **76 of the 155
/// styles the corpus reaches are not left** -- 49% -- so a consumer placing
/// them all at the insertion point puts half its labels off by half a label.
/// (Across all 376 paragraph records the share is 38%; the styles text
/// actually reaches lean further off-left than the file as a whole.) See
/// `docs/analysis/2026-08-13-text-para-layout-and-justification.md`.
#[test]
fn text_reaches_the_side_its_paragraph_letters_from() {
    use pid_parse::style_link::TextAlignment;

    let mut sides: BTreeMap<String, usize> = BTreeMap::new();
    let mut fixtures_seen = 0usize;
    for expected in &EXPECTED {
        let path = Path::new(expected.fixture);
        if !path.exists() {
            continue;
        }
        fixtures_seen += 1;
        let resolved =
            pid_parse::style_link::text_heights_for_file(path).expect("fixture opens for text");
        for style in resolved.values() {
            let key = match style.alignment {
                Some(TextAlignment::Left) => "left",
                Some(TextAlignment::Center) => "center",
                Some(TextAlignment::Right) => "right",
                None => "unstated",
            };
            *sides.entry(key.to_string()).or_default() += 1;
        }
    }
    if fixtures_seen < EXPECTED.len() {
        return;
    }

    // Every style a text record reaches arrives through a paragraph, so none
    // of them is `unstated`; the one-hop shape that would produce that does
    // not occur in this corpus.
    let expected_sides: BTreeMap<String, usize> =
        [("center", 60), ("left", 79), ("right", 16)]
            .iter()
            .map(|(key, count)| ((*key).to_string(), *count))
            .collect();
    assert_eq!(sides, expected_sides);
    assert_eq!(sides.values().sum::<usize>(), 155, "one side per height");
}

/// Lettering reaches the typeface its character style names.
///
/// The name is the last field of the record, `+68` u16 count and `+70` UTF-16
/// body, and the length formula `payload == 70 + 2*count` holds for all 381
/// character styles in the corpus. The distribution below is the subset text
/// actually reaches: 7 typefaces across 155 styles.
///
/// It is also the answer to a question the format guide left open. Twelve of
/// the 381 names are damaged by the vendor before encoding -- `ËÎÌå` is
/// `宋体`'s GB2312 bytes widened one per code unit, and four more read
/// `匪_GB2312` -- and the decision was to carry them verbatim rather than
/// reconstruct. **No text record reaches any of them.** They live in the style
/// table unreferenced, alongside the single `Intergraph ANSI` style, so the
/// decision costs this corpus nothing on screen. A fixture that did reach one
/// would break this assertion, which is the point of pinning it. See
/// `docs/analysis/2026-08-13-text-colour-is-002c-plus-34.md` §4-3.
#[test]
fn text_reaches_the_typeface_its_character_style_names() {
    let mut fonts: BTreeMap<String, usize> = BTreeMap::new();
    let mut unstated = 0usize;
    let mut fixtures_seen = 0usize;
    for expected in &EXPECTED {
        let path = Path::new(expected.fixture);
        if !path.exists() {
            continue;
        }
        fixtures_seen += 1;
        let resolved =
            pid_parse::style_link::text_heights_for_file(path).expect("fixture opens for text");
        for style in resolved.values() {
            match style.font_name.as_deref() {
                Some(name) => *fonts.entry(name.to_string()).or_default() += 1,
                None => unstated += 1,
            }
        }
    }
    if fixtures_seen < EXPECTED.len() {
        return;
    }

    assert_eq!(
        unstated, 0,
        "every character style the corpus reaches names a typeface, got {fonts:?}"
    );
    let expected_fonts: BTreeMap<String, usize> = [
        ("Arial", 68),
        ("Arial Narrow", 28),
        ("Braggadocio", 11),
        ("SimSun-ExtB", 4),
        ("仿宋", 5),
        ("仿宋_GB2312", 3),
        ("宋体", 36),
    ]
    .iter()
    .map(|(key, count)| ((*key).to_string(), *count))
    .collect();
    assert_eq!(fonts, expected_fonts);
    assert_eq!(fonts.values().sum::<usize>(), 155, "one typeface per height");
}

/// The join a renderer performs: entity -> line style, by stream path and oid.
///
/// [`pid_parse::style_link::line_styles_for_file`] keys on
/// `(stream_path, oid)`, and a normalized entity carries both. If those two
/// ever stop agreeing the lookup silently returns nothing and everything
/// quietly renders at the default width, which is the failure mode this test
/// exists to make loud.
#[test]
fn every_normalized_line_entity_finds_its_style_by_stream_and_oid() {
    let mut joined = 0usize;
    for expected in &EXPECTED {
        let path = Path::new(expected.fixture);
        if !path.exists() {
            continue;
        }
        let styles =
            pid_parse::style_link::line_styles_for_file(path).expect("fixture opens for styling");
        let parsed = pid_parse::PidParser::new()
            .parse_file(path)
            .expect("fixture parses");
        let geometry = pid_parse::build_normalized_geometry(&parsed);

        let mut missed = Vec::new();
        for entity in &geometry.entities {
            let is_indexed_family = ["igline2d:", "igpoint2d:", "iglinestring2d:"]
                .iter()
                .any(|marker| entity.id.contains(marker));
            if !is_indexed_family {
                continue;
            }
            let (Some(stream), Some(oid)) = (entity.source.stream_path.clone(), entity.graphic_oid)
            else {
                missed.push(format!("{} has no stream/oid", entity.id));
                continue;
            };
            if styles.contains_key(&(stream, oid)) {
                joined += 1;
            } else {
                missed.push(entity.id.clone());
            }
        }
        assert_eq!(
            missed,
            Vec::<String>::new(),
            "{}: every indexed-family entity must join",
            expected.fixture
        );
    }
    if joined > 0 {
        assert_eq!(joined, 562, "entities joined to a line style");
    }
}

#[test]
fn a_sheet_never_resolves_against_another_documents_style_table() {
    // Pooling ids across documents is what hid this link for two rounds. The
    // guard is structural, so it is worth a test of its own.
    assert_eq!(stylecluster_path_for_sheet("/Sheet6"), "/StyleCluster");
    assert_eq!(
        stylecluster_path_for_sheet("/JSite329/Sheet6"),
        "/JSite329/StyleCluster"
    );
}
