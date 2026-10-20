//! Which width the reference centres a run on, and where a boxed run starts.
//!
//! Two questions the class report cannot answer on its own:
//!
//! * `along` for a centred run is half a run-width difference plus the anchor
//!   rule's own error, so the two have to be separated before either can be
//!   fixed. `predicted` is half the advance difference — what a pure width
//!   mismatch would give — and `residual` is what the anchor rule adds.
//! * A boxed `igTextBox` letters from its box's left edge. If the reference
//!   puts the first glyph's **ink** on that edge rather than its pen, every
//!   boxed run sits one left-side-bearing right of ours; `first_lsb` is that
//!   bearing in millimetres, from the face the corpus loads, so the two can be
//!   read side by side.
//!
//! ```text
//! cargo run -p pid-plot --example probe_center_anchor_width -- \
//!     "<drawing.pid>" "<reference.pdf>" "<ours.pdf>" "<corpus>"
//! ```
//!
//! Set `DETAIL=1` for one row per paired label. Read-only.

use std::collections::BTreeMap;
use std::env;
use std::path::Path;
use std::process::ExitCode;

use fontcull_skrifa::MetadataProvider;
use pid_plot::compare::{
    MaskRect, TextSource, compare_plots_with_model, template_masks, text_sources,
};
use pid_plot::model::lay_out_drawing;
use pid_plot::read::{PlotInk, read_plot};

const TEXT_MM: f64 = 1.0;
const SOURCE_MM: f64 = 20.0;

struct Row {
    text: String,
    raw: String,
    first_lsb: f64,
    along: f64,
    predicted: f64,
    height: f64,
    our_height: f64,
    our_advance: f64,
    ref_advance: f64,
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn read(path: &str) -> Option<PlotInk> {
    match std::fs::read(path)
        .map_err(|error| error.to_string())
        .and_then(|bytes| read_plot(&bytes).map_err(|error| error.to_string()))
    {
        Ok(ink) => Some(ink),
        Err(error) => {
            eprintln!("{path}: {error}");
            None
        }
    }
}

fn source<'a>(
    origin: [f64; 2],
    text: &str,
    sources: &'a [TextSource],
    into_reference: impl Fn([f64; 2]) -> [f64; 2],
) -> Option<&'a TextSource> {
    let wanted = normalize(text);
    let mut best: Option<(f64, &TextSource)> = None;
    for candidate in sources {
        if normalize(&candidate.text) != wanted {
            continue;
        }
        let point = into_reference(candidate.origin_mm);
        let distance = (point[0] - origin[0]).hypot(point[1] - origin[1]);
        if distance <= SOURCE_MM && best.is_none_or(|(best, _)| distance < best) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate)
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(pid), Some(reference), Some(ours), Some(corpus)) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        eprintln!(
            "usage: probe_center_anchor_width <drawing.pid> <reference.pdf> <ours.pdf> <corpus>"
        );
        return ExitCode::from(2);
    };
    let model = match lay_out_drawing(Path::new(&pid), Path::new(&corpus)) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    let (Some(reference), Some(ours)) = (read(&reference), read(&ours)) else {
        return ExitCode::from(1);
    };
    let template = template_masks(reference.page_mm);
    let missing: Vec<MaskRect> = model
        .missing_boxes
        .iter()
        .map(|box_| MaskRect {
            label: box_.path.clone(),
            min_mm: box_.min_mm,
            max_mm: box_.max_mm,
        })
        .collect();
    let report = compare_plots_with_model(&reference, &ours, &template, &missing, &model);
    let fit = &report.fit;
    let into_reference = |point: [f64; 2]| {
        [
            (point[0] - fit.translation_mm[0]) / fit.scale,
            (point[1] - fit.translation_mm[1]) / fit.scale,
        ]
    };
    let narrow_bytes = std::fs::read(Path::new(&corpus).join("fonts/ARIALN.TTF")).ok();
    let narrow = narrow_bytes
        .as_deref()
        .and_then(|bytes| fontcull_skrifa::FontRef::new(bytes).ok());
    let sources = text_sources(&model);
    let mut used = vec![false; ours.spans.len()];
    let mut classes: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for span in &reference.spans {
        let wanted = normalize(&span.text);
        if wanted.is_empty() {
            continue;
        }
        let found = ours.spans.iter().enumerate().find(|(slot, other)| {
            if used[*slot] || normalize(&other.text) != wanted {
                return false;
            }
            let point = into_reference(other.origin_mm);
            (point[0] - span.origin_mm[0]).hypot(point[1] - span.origin_mm[1]) <= TEXT_MM
        });
        let Some((slot, mine)) = found else {
            continue;
        };
        used[slot] = true;
        let point = into_reference(mine.origin_mm);
        let delta = [point[0] - span.origin_mm[0], point[1] - span.origin_mm[1]];
        let direction = span.direction;
        let along = delta[0] * direction[0] + delta[1] * direction[1];
        let Some(source) = source(span.origin_mm, &span.text, &sources, into_reference) else {
            continue;
        };
        let class = match source.symbol {
            Some(_) => format!("symbol/{:?}", source.line),
            None => format!("{:?}/{:?}", source.anchor, source.line),
        };
        let first_lsb = narrow
            .as_ref()
            .and_then(|font| {
                use fontcull_skrifa::raw::TableProvider;
                let upem = f64::from(font.head().map(|head| head.units_per_em()).unwrap_or(0));
                let first = mine.text.trim_start().chars().next()?;
                let gid = font.charmap().map(first)?;
                let metrics = font.glyph_metrics(
                    fontcull_skrifa::instance::Size::unscaled(),
                    fontcull_skrifa::instance::LocationRef::default(),
                );
                let bounds = metrics.bounds(gid)?;
                Some(f64::from(bounds.x_min) / upem * mine.height_mm)
            })
            .unwrap_or(f64::NAN);
        classes.entry(class).or_default().push(Row {
            raw: format!("{:?}", mine.text),
            text: wanted,
            first_lsb,
            along,
            // A run that steps back by half its own advance lands half the
            // width difference away from ours; this is that prediction.
            predicted: 0.5 * (span.advance_mm - mine.advance_mm),
            height: span.height_mm,
            our_height: mine.height_mm,
            our_advance: mine.advance_mm,
            ref_advance: span.advance_mm,
        });
    }
    if env::var_os("DETAIL").is_some() {
        println!(
            "class\ttext\talong_mm\tpredicted_mm\theight_mm\tour_height\tour_advance\tref_advance\tfirst_lsb\trun"
        );
        for (class, rows) in &classes {
            for row in rows {
                println!(
                    "{class}\t{}\t{:+.3}\t{:+.3}\t{:.2}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}",
                    row.text,
                    row.along,
                    row.predicted,
                    row.height,
                    row.our_height,
                    row.our_advance,
                    row.ref_advance,
                    row.first_lsb,
                    row.raw
                );
            }
        }
    }
    println!(
        "class\tn\talong_mm\tpredicted_mm\tresidual_mm\talong/heights\tresidual/heights\tfirst_lsb\talong+lsb"
    );
    for (class, rows) in &classes {
        let n = rows.len();
        let mean = |pick: fn(&Row) -> f64| rows.iter().map(pick).sum::<f64>() / n as f64;
        let along = mean(|row| row.along);
        let predicted = mean(|row| row.predicted);
        let height = mean(|row| row.height);
        let lsb = mean(|row| row.first_lsb);
        println!(
            "{class}\t{n}\t{along:+.3}\t{predicted:+.3}\t{:+.3}\t{:+.4}\t{:+.4}\t{lsb:+.3}\t{:+.3}",
            along - predicted,
            along / height,
            (along - predicted) / height,
            along + lsb
        );
    }
    ExitCode::SUCCESS
}
