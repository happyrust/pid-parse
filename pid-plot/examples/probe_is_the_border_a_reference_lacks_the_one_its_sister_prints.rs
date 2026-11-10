//! When a reference plot prints no border and we draw the one the sheet
//! links, is ours the border the vendor prints for a sheet that links the
//! same template?
//!
//! `一级闪蒸分离器A` and `低温分离器A` link `CPE-SQ-A1.igr`, and their
//! reference plots carry no frame, title block or sign-off column at all;
//! `一级闪蒸分离器B` links the same template with the same bytes, and its
//! reference plot carries all three. So the acceptance gate charges the two
//! pages about 3479 mm of extra ink each. That reading does not say whether
//! the border we draw there is *right*: only that their plots lack it.
//!
//! This takes the border alone — the furniture primitives of our page for the
//! sheet whose reference is bare — writes it as a page of its own, and
//! compares it with the *sister's* reference plot. A template is placed the
//! same on every sheet of its size, so if our border is the vendor's, the
//! sister's plot covers every stroke of it and pairs every label the template
//! letters (the labels a drawing puts in the title block are its own and are
//! not on this page).
//!
//! ```text
//! cargo run -p pid-plot --example probe_is_the_border_a_reference_lacks_the_one_its_sister_prints
//! ```
//!
//! Exit code 1 when a border stroke of ours has no ink of the sister's plot
//! within the gate's 0.5 mm, a template label fails to pair, or a paired
//! stroke prints at another width.
//!
//! Paint is read as ink here (`read::Painted::All`): the question is where
//! the strokes lie. Whether they print is
//! `probe_which_paint_leaves_no_ink`'s.
//!
//! The corpus is `PID_ACCEPTANCE_CORPUS`, default
//! `D:\work\cad\pid-test-data\验收数据`. Read-only.

#[path = "support/acceptance.rs"]
mod acceptance;

use std::process::ExitCode;

use pid_plot::compare::compare_plots_with_model;
use pid_plot::model::{PaperModel, PaperShape};
use pid_plot::read::read_plot;

use acceptance::{labels_off, pages, short, uncovered_mm, write_alone};

/// The two sheets whose reference plots are bare, and the sheet that links
/// the same template and whose plot is framed.
const BARE: [&str; 2] = ["一级闪蒸分离器A", "低温分离器A"];
const SISTER: &str = "一级闪蒸分离器B";

/// `model` cut down to what the border template contributed.
fn border_only(model: &PaperModel) -> PaperModel {
    let mut border = model.clone();
    border.primitives.retain(|primitive| primitive.furniture);
    border
}

fn main() -> ExitCode {
    let wanted: Vec<String> = BARE
        .iter()
        .chain(std::iter::once(&SISTER))
        .map(|name| (*name).to_string())
        .collect();
    let pages = match pages(&wanted) {
        Ok(pages) => pages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let Some(sister) = pages.iter().find(|page| page.name.contains(SISTER)) else {
        eprintln!("{SISTER}: not in the corpus");
        return ExitCode::from(1);
    };
    let Ok(sister_reference) = read_plot(&sister.reference) else {
        eprintln!("{}: the reference could not be read", sister.name);
        return ExitCode::from(1);
    };
    let template_of = |model: &PaperModel| {
        model.template.as_ref().map_or_else(
            || "none".to_string(),
            |template| {
                template
                    .path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or_default()
                    .to_string()
            },
        )
    };
    println!(
        "sister: {} — template {}, reference {:.0} × {:.0} mm",
        short(sister.name),
        template_of(&sister.model),
        sister_reference.page_mm[0],
        sister_reference.page_mm[1]
    );
    let mut clean = true;
    for page in pages.iter().filter(|page| !page.name.contains(SISTER)) {
        let border = border_only(&page.model);
        let strokes = border
            .primitives
            .iter()
            .filter(|primitive| !matches!(primitive.shape, PaperShape::Text { .. }))
            .count();
        let labels = border.primitives.len() - strokes;
        let bytes = match write_alone(&border) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{}: {error}", page.name);
                return ExitCode::from(1);
            }
        };
        let (Ok(ours), Ok(own_reference)) = (read_plot(&bytes), read_plot(&page.reference)) else {
            eprintln!("{}: a plot could not be read back", page.name);
            return ExitCode::from(1);
        };
        println!(
            "== {} — template {}; its border alone: {} stroke primitives, {} labels",
            short(page.name),
            template_of(&page.model),
            strokes,
            labels
        );
        // Against the sheet's own reference: none of it is there.
        let own = compare_plots_with_model(&own_reference, &ours, &[], &[], &border);
        println!(
            "   against its own reference:    extra {:>9.3} mm, labels of ours paired {}/{}, pen \
             {} paired / {} unpaired",
            own.extra_length_mm, own.text_matched, own.text_ours, own.pen.paired, own.pen.unpaired
        );
        // Against the sister's: every stroke and every template label is.
        let other = compare_plots_with_model(&sister_reference, &ours, &[], &[], &border);
        let off = labels_off(&other, &sister_reference);
        println!(
            "   against the sister's reference: extra {:>9.3} mm, labels of ours paired {}/{}, pen \
             {} paired ({} agree, {} differ) / {} unpaired; {} label(s) past the label limits",
            other.extra_length_mm,
            other.text_matched,
            other.text_ours,
            other.pen.paired,
            other.pen.agreed,
            other.pen.differed,
            other.pen.unpaired,
            off.len()
        );
        println!(
            "      (the sister's plot also inks its own drawing, so {:.3} mm of it is uncovered by a \
             border alone; fit scale {:.7}, translation ({:+.4}, {:+.4}) mm, within threshold {})",
            uncovered_mm(&other),
            other.fit.scale,
            other.fit.translation_mm[0],
            other.fit.translation_mm[1],
            other.fit.within_threshold
        );
        for text in &other.unmatched_our_text {
            println!(
                "      ours only: {:?} at ({:.2}, {:.2})",
                text.text, text.origin_mm[0], text.origin_mm[1]
            );
        }
        for line in &off {
            println!("      past limits: {line}");
        }
        let same = other.extra_length_mm < 5e-4
            && other.text_matched == other.text_ours
            && other.pen.unpaired == 0
            && other.pen.differed == 0;
        println!(
            "   → {}",
            if same {
                "the border we draw is the one the vendor prints for the sister sheet"
            } else {
                "the border we draw is NOT the sister's"
            }
        );
        clean &= same;
    }
    if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
