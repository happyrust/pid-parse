//! Does the vendor's plot draw anything for a `ComplexString` beyond the
//! curves it lists?
//!
//! A `0x0021 ComplexString` is an association: a member count, a scale, and
//! one entry a member. It states no coordinate, so this crate decodes it and
//! emits nothing for it — its curve members are `igLine2d` / `igArc2d`
//! records of the same chain and draw themselves. The native graphic
//! predicate takes `0x0021` for a graphic object all the same, so "it draws
//! nothing of its own" needs a witness: a vendor plot of a sheet that has
//! one.
//!
//! The acceptance sheet `轻烃分离器I` is an A2 page that wears
//! `CPECCHB-SQ-A2.igr`, and that template's sheet holds `ComplexString`
//! records: the runs of border lines of the frame and the title block. For
//! each of them this probe finds the page primitives its curve members draw
//! (by the member records' byte ranges in the template's stream) and reads
//! the two plots along each member — in the corridor 0.4 mm either side of
//! it, since a string of frame lines boxes in the whole drawing:
//!
//! * the member's own length, and the vendor's ink that runs along it
//!   (inside the corridor and parallel to the member within a degree), with
//!   the pen widths that ink is drawn in: a string drawn once more as a
//!   whole, over its members, would show as twice the length or a second
//!   width;
//! * the vendor's ink in the corridor, how much of it our ink covers, and how
//!   much of our ink the vendor's plot does not have (`compare_plots` with
//!   everything outside the corridor masked — the acceptance gate's own
//!   rule): a join the string adds at a corner, or any other stroke of its
//!   own, would be vendor ink nothing of ours covers;
//! * the vendor's fill outlines and text spans in the corridor;
//! * the strokes the plot reader leaves out of both pages' ink because they
//!   run more than nine tenths of the sheet (`sheet_edge_mm`): a frame line
//!   is such a stroke, the comparison above says nothing about it, and so
//!   the frame's own run of four lines is read here from `read_plot_pens` —
//!   the vendor's and ours, in the member's corridor, by pen width.
//!
//! What the corridors hold beyond the members is the other furniture that
//! ends on or crosses a border line, which both plots draw.
//!
//! ```text
//! cargo run -p pid-plot --example probe_what_the_reference_prints_where_a_complex_string_is
//! ```
//!
//! Reads `PID_ACCEPTANCE_CORPUS` (default `D:\work\cad\pid-test-data\验收数据`)
//! and exits non-zero when the sheet, its template or a string's members
//! cannot be found.

#[path = "support/acceptance.rs"]
mod acceptance;

use std::collections::BTreeMap;

use pid_plot::compare::{MaskRect, compare_plots};
use pid_plot::read::{SheetEdge, read_plot, read_plot_pens};

/// Millimetres added round a string's members for its box: wide enough for
/// a pen's half width and the fit between the two plots, narrow enough to
/// keep neighbouring furniture out where it can be kept out.
const MARGIN_MM: f64 = 0.4;

fn length(points: &[[f64; 2]]) -> f64 {
    points
        .windows(2)
        .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
        .sum()
}

fn inside(window: &MaskRect, point: [f64; 2]) -> bool {
    (window.min_mm[0]..=window.max_mm[0]).contains(&point[0])
        && (window.min_mm[1]..=window.max_mm[1]).contains(&point[1])
}

fn main() -> Result<(), String> {
    let pages = acceptance::pages(&["轻烃分离器I".to_string()])?;
    let page = pages
        .first()
        .ok_or("the A2 acceptance sheet is not in the corpus")?;
    let template = page
        .model
        .template
        .as_ref()
        .ok_or("the sheet wears no template")?;
    println!("{}: template {}", page.name, template.path);
    // The template is a file of a template directory, or a member of a
    // backup package's `RefData~*.zip`, named `<archive>\<member>`.
    let path = std::path::Path::new(&template.path);
    let bytes = match path.parent().filter(|parent| parent.is_file()) {
        Some(archive) => {
            let member = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("the template's member has no name")?;
            pid_parse::refdata_archive::read_archive_member(archive, member)
                .map_err(|error| format!("{}: {error}", template.path))?
        }
        None => std::fs::read(path).map_err(|error| format!("{}: {error}", template.path))?,
    };
    let document = pid_parse::PidParser::new()
        .parse_reader(std::io::Cursor::new(bytes))
        .map_err(|error| format!("{}: {error}", template.path))?;

    // Member oid -> (stream, byte range) of the line or arc record.
    let mut strings = Vec::new();
    for sheet in &document.sheet_streams {
        let Some(geometry) = &sheet.geometry else {
            continue;
        };
        let mut curves: BTreeMap<u32, [usize; 2]> = BTreeMap::new();
        for line in &geometry.decoded_iglines {
            curves.insert(line.oid, [line.byte_start, line.byte_end]);
        }
        for arc in &geometry.decoded_igarcs {
            curves.insert(arc.oid, [arc.byte_start, arc.byte_end]);
        }
        for string in &geometry.decoded_igcomplexstrings {
            let members: Vec<(u32, Option<[usize; 2]>)> = string
                .curve_oids()
                .map(|oid| (oid, curves.get(&oid).copied()))
                .collect();
            strings.push((
                sheet.path.clone(),
                string.oid,
                string.members.len(),
                members,
            ));
        }
    }
    if strings.is_empty() {
        return Err(format!("{} holds no ComplexString", template.path));
    }

    let reference = read_plot(&page.reference).map_err(|error| format!("reference: {error}"))?;
    let ours = read_plot(&page.ours).map_err(|error| format!("ours: {error}"))?;
    let reference_edges = read_plot_pens(&page.reference)
        .map_err(|error| format!("reference: {error}"))?
        .sheet_edges;
    let our_edges = read_plot_pens(&page.ours)
        .map_err(|error| format!("ours: {error}"))?
        .sheet_edges;
    // Inked sheet edges inside `window`, as millimetres by pen width.
    let edges_in =
        |edges: &[SheetEdge], window: &MaskRect, tally: &mut BTreeMap<String, (usize, f64)>| {
            for edge in edges {
                if edge.paper || !(inside(window, edge.start_mm) && inside(window, edge.end_mm)) {
                    continue;
                }
                let entry = tally.entry(format!("{:.3}", edge.width_mm)).or_default();
                entry.0 += 1;
                entry.1 += length(&[edge.start_mm, edge.end_mm]);
            }
        };
    println!(
        "page {:.2} x {:.2} mm; {} ComplexString record(s) on the template's sheets",
        ours.page_mm[0],
        ours.page_mm[1],
        strings.len()
    );

    let mut unfound = 0usize;
    for (stream, oid, entries, members) in &strings {
        // What the corridors of this string's members add up to.
        let mut members_mm = 0.0f64;
        let mut vendor_along_mm = 0.0f64;
        let mut vendor_mm = 0.0f64;
        let mut covered_mm = 0.0f64;
        let mut extra_mm = 0.0f64;
        let mut fills = 0usize;
        let mut spans = 0usize;
        let mut drawn = 0usize;
        let mut widths: BTreeMap<String, f64> = BTreeMap::new();
        let mut our_widths: BTreeMap<String, usize> = BTreeMap::new();
        let mut vendor_edges: BTreeMap<String, (usize, f64)> = BTreeMap::new();
        let mut our_sheet_edges: BTreeMap<String, (usize, f64)> = BTreeMap::new();
        for (member, range) in members {
            let Some(range) = range else {
                println!("  oid {oid}: member {member} is not a line or an arc record");
                unfound += 1;
                continue;
            };
            let primitives: Vec<_> = page
                .model
                .primitives
                .iter()
                .filter(|primitive| {
                    primitive.furniture
                        && primitive.stream_path.as_deref() == Some(stream.as_str())
                        && primitive.byte_range == Some(*range)
                })
                .collect();
            if primitives.is_empty() {
                println!("  oid {oid}: member {member} draws no primitive on the page");
                unfound += 1;
            }
            for primitive in primitives {
                drawn += 1;
                if let Some(stroke) = &primitive.stroke {
                    *our_widths
                        .entry(format!("{:.3}", stroke.width_mm))
                        .or_default() += 1;
                }
                for stroke in acceptance::strokes_of(&primitive.shape) {
                    let (Some(first), Some(last)) = (stroke.first(), stroke.last()) else {
                        continue;
                    };
                    members_mm += length(&stroke);
                    let xs = stroke.iter().map(|point| point[0]);
                    let ys = stroke.iter().map(|point| point[1]);
                    let window = MaskRect {
                        label: format!("member {member} of ComplexString {oid}"),
                        min_mm: [
                            xs.clone().fold(f64::INFINITY, f64::min) - MARGIN_MM,
                            ys.clone().fold(f64::INFINITY, f64::min) - MARGIN_MM,
                        ],
                        max_mm: [
                            xs.fold(f64::NEG_INFINITY, f64::max) + MARGIN_MM,
                            ys.fold(f64::NEG_INFINITY, f64::max) + MARGIN_MM,
                        ],
                    };
                    // The member's direction, for "runs along it".
                    let along = [last[0] - first[0], last[1] - first[1]];
                    let along_len = along[0].hypot(along[1]).max(f64::MIN_POSITIVE);
                    for segment in &reference.segments {
                        if !(inside(&window, segment.start_mm) && inside(&window, segment.end_mm)) {
                            continue;
                        }
                        if segment.width_mm == 0.0 {
                            fills += 1;
                            continue;
                        }
                        let run = [
                            segment.end_mm[0] - segment.start_mm[0],
                            segment.end_mm[1] - segment.start_mm[1],
                        ];
                        let run_len = run[0].hypot(run[1]);
                        let cross = (run[0] * along[1] - run[1] * along[0]).abs()
                            / (run_len.max(f64::MIN_POSITIVE) * along_len);
                        if cross < 0.0175 {
                            vendor_along_mm += run_len;
                            *widths
                                .entry(format!("{:.3}", segment.width_mm))
                                .or_default() += run_len;
                        }
                    }
                    spans += reference
                        .spans
                        .iter()
                        .filter(|span| inside(&window, span.origin_mm))
                        .count();
                    edges_in(&reference_edges, &window, &mut vendor_edges);
                    edges_in(&our_edges, &window, &mut our_sheet_edges);
                    let report = compare_plots(&reference, &ours, &acceptance::outside(&window));
                    vendor_mm += report.reference_length_mm;
                    covered_mm += report.covered_length_mm;
                    extra_mm += report.extra_length_mm;
                }
            }
        }
        if drawn == 0 {
            continue;
        }
        println!(
            "  {stream} oid {oid}: {entries} member entries, {} curve(s), {drawn} primitive(s) on the page, pens {our_widths:?}",
            members.len()
        );
        println!(
            "    members' strokes {members_mm:.3} mm; vendor ink along them {vendor_along_mm:.3} mm, by pen width {}",
            widths
                .iter()
                .map(|(width, mm)| format!("{width} mm: {mm:.3}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "    in the corridors: vendor ink {vendor_mm:.3} mm, covered by ours {covered_mm:.3} mm, uncovered {:.3} mm; ours the vendor lacks {extra_mm:.3} mm",
            vendor_mm - covered_mm
        );
        println!(
            "    in the corridors: {fills} vendor fill outline segment(s), {spans} vendor text span(s)"
        );
        let edges = |tally: &BTreeMap<String, (usize, f64)>| {
            if tally.is_empty() {
                "none".to_string()
            } else {
                tally
                    .iter()
                    .map(|(width, (count, mm))| format!("{count} of {width} mm, {mm:.3} mm"))
                    .collect::<Vec<_>>()
                    .join("; ")
            }
        };
        println!(
            "    strokes left out as sheet edges, in the corridors: vendor {}; ours {}",
            edges(&vendor_edges),
            edges(&our_sheet_edges)
        );
    }
    if unfound != 0 {
        return Err(format!(
            "{unfound} member(s) could not be found on the page"
        ));
    }
    Ok(())
}
