//! Acceptance gate: every drawing in `验收数据` that ships a SmartPlant plot,
//! written by this crate's own writer and compared to that plot.
//!
//! Eleven drawings live there: the five the crate already ratcheted
//! (`plot_compare.rs` measures them out of the same directory), the two A0
//! wellsite sheets, the three A1 air-cooler sheets, and one A2 sheet. Their
//! reference plots are the plotter's own output — Amyuni PDF Converter
//! 4.5.2.9, the driver SmartPlant ships — so the page sizes and pens they
//! carry are the paper the drawings were issued on.
//!
//! What the readings say, 2026-11-02:
//!
//! * Ten of the eleven sit at coverage 0.9990–0.9997; the three A1 air
//!   coolers rose from 0.9986 once `0x0063 igEllipse2d`, the fan ovals of
//!   `Air Cooler Type 1.sym`, was decoded (`docs/plans/2026-11-02-the-a1-stem.md` §7). The extra ink on the
//!   three A0 wellsites was a join: a valve stem 0.33 mm beside its pipe is
//!   inside the fit's 0.4 mm sideways tolerance, and the merged chord then
//!   misses the stem both pages draw. The extra sampler joins at 0.20 mm, so
//!   I, J K and L M read 0.000 mm (`docs/analysis/2026-11-04-the-extra-join-was-swallowing-the-stem.md`).
//!   Every label the reference counts is paired. A same-width chord within
//!   0.5 mm, lengths within a tenth, uses the fill outline's angle, so the
//!   three valve chords on J K and L M pair at 0.34 mm instead of the thinner
//!   stroke 1 mm away. The three A1 sheets' shared 8.210 mm residue
//!   was a lone complex-dash glyph at the run's midpoint, and they now read
//!   extra 0.000 mm (`docs/plans/2026-11-02-the-a1-stem.md`).
//!   Those ten now read coverage 1.000000 and uncovered 0.000 mm: what was
//!   left on them was the tip of a signal-line dash, and it closed once that
//!   pattern (`JStyleSimpleDashType` fit mode 2) is restarted on every vertex
//!   and stretched to each straight piece the way `style.dll` lays it out
//!   (`docs/analysis/2026-11-08-the-signal-dash-is-fitted-to-every-piece.md`).
//! * The A2 sheet read 0.9819 / 210.712 mm on that date (it is pinned below at
//!   the 0.9829 / 187.091 mm it reads today) because it is the first A2 page
//!   this crate has ever had a reference for, and its furniture is a third
//!   template family (`CPECCHB-SQ-A2.igr`, the one its own link record names).
//!   `pid-plot/assets/template-masks.json` had no A2 entry before this round,
//!   so the whole title block and sign-off column were charged as drawing ink:
//!   0.9379 / 1132.334 mm. The surveyed A2 masks are what closed that gap, and
//!   what is left is one instrument cluster drawn ~16.9 mm high and ~1.9 mm
//!   right of where the reference prints it — `TICA` and `H=-15℃` are both
//!   decoded and laid out, at 500.8/256.3 and 507.8/260.8 mm. Eleven reference
//!   labels still miss: nine are that cluster, and two are pipeline strings
//!   whose text is the `.pid`'s own. The two `动火点` labels pair, and so does
//!   the mirrored valve's `D` now that a symbol top hangs from the ascender
//!   line, so the text pin is 214 of 225
//!   (`docs/analysis/2026-11-04-the-a2-five-labels-outside-the-bubble.md`,
//!   `docs/analysis/2026-11-05-a-symbol-top-is-the-ascender-line.md`).
//!   The 55.980 mm grouped as `/Sheet6` is that leader at `x 500.69`, 2.00 mm
//!   beside the reference leader, not frame ink
//!   (`docs/analysis/2026-11-04-the-a2-sheet6-extra-is-the-leader.md`).
//!   The 109.285 mm of uncovered `/Sheet6` is the same leader's reference copy,
//!   the TT balloon at `(498.70, 220.37)`, and dash tips
//!   (`docs/analysis/2026-11-04-the-a2-sheet6-uncovered-is-the-reference-leader.md`).
//!   The cluster is the same body at
//!   the same size — 12.698 mm across against the reference's 12.869, one pen
//!   apart — simply translated, and its leader's far end stays put at `y≈342.4`
//!   while the near end follows the bubble: the `.pid` records the bubble dragged
//!   along its line, so no renderer change recovers the reference's position
//!   (`docs/analysis/2026-11-02-the-a2-labels-were-already-drawn.md`). The pins
//!   below are the reading, and raising them is a code change, not a threshold
//!   change.
//!
//! The pen column is the reading of `CompareReport::pen`: how many of our
//! strokes the reference prints at the width we print them. It reached zero
//! differences on all eleven pages only after `pen_width_mm` grew the fourth
//! pen — the CCHB A2 title block declares 1.0606 / 1.1313 / 1.4141 mm rails
//! and the plot prints every one of them at 1.02 mm.
//!
//! Every stroke the pen pairing leaves unpaired is pinned under the reason it
//! has no partner (2026-11-07,
//! `docs/analysis/2026-11-07-every-unpaired-pen-stroke-has-a-reason.md`). A
//! curve the two pages cut at different vertices, or a short stroke the
//! plotter snapped to its 1/150 in grid, pairs along the reference's chain,
//! so what is left is ink the reference lacks or lays differently: the
//! `CPE-SQ-A1.igr` border on `一级闪蒸分离器A` / `低温分离器A` (286 strokes,
//! exactly their whole-page extra ink) and the A2 instrument cluster with its
//! leader. Every other page used to leave the signal line's dash tail as
//! well, which was our plot misread, not misdrawn: `DashWalk` gave up on a
//! dash after eight periods and read the rest of a long fitted piece as one
//! solid stroke, and now walks it to the end. No unpaired stroke lies in a
//! missing-symbol box.
//!
//! Each page is also compared with no template mask at all, so the sign-off
//! column and the title block are charged like any other ink (the `whole`
//! pins, 2026-11-07). The seven `CPE-SQ` pages leave no reference ink
//! uncovered there and pair every furniture label. The 0.169 mm each used to
//! read was the outer sheet frame's last device unit, which the plot draws
//! wholly outside that frame's own clip: it never printed, and the reader now
//! keeps only ink inside the clip
//! (`docs/analysis/2026-11-07-the-corner-tail-is-outside-its-clip.md`).
//! `一级闪蒸分离器A` and `低温分离器A` carry 3478.9 / 3478.7 mm of extra
//! ink: their reference plots print no border at all, while ours draws the
//! `CPE-SQ-A1.igr` both link — the same link `一级闪蒸分离器B` wears on its
//! plot. The A2 page reads 0.9881, 208.840 mm uncovered and 187.091 mm extra,
//! with 250 of 261 labels paired: every furniture label pairs, and the eleven
//! that miss are the ones the masked view misses.
//!
//! A paired label is held two ways (2026-11-10,
//! `docs/analysis/2026-11-10-every-lettering-class-is-held-and-every-label-counted.md`).
//! Every lettering class of a page — in the masked view and on the whole
//! page, whatever its size, `unknown` included — reads inside the two class
//! thresholds, with no exception. A class mean cannot see a single label, so
//! the labels whose anchor point lands more than 0.15 mm from the
//! reference's or whose run is more than 0.05 mm wider or narrower are
//! counted on the whole page and pinned: 50 over the eleven pages, each with
//! a cause on record.
//!
//! A borrowed run is held to the reference's font family as well
//! (2026-11-10). Where the face a label names cannot draw a character, the
//! plot sets it in another: fourteen paired runs on the eleven pages, twelve
//! `℃` after Arial Narrow digits and the A2 sheet's two `动火点`. The
//! reference sets all fourteen in NSimSun, and so does this crate, at the
//! reference's size — SimSun, the same family, on a machine with no NSimSun
//! file. Each page's count of borrowed runs is pinned, and its count of them
//! in the reference's family may only rise: it is fourteen of fourteen.
//!
//! Until the borrowed runs were lettered as the reference letters them
//! (2026-11-11), the twelve `℃` were FangSong at the label's own 10.17 pt
//! and the two `动火点` were squeezed to their 1.524 mm step: the pieces of
//! the `℃` labels read +0.0294 body heights wide on eight pages, pinned
//! there as the one class past the width threshold, 64 labels were past the
//! two limits, and two of the fourteen runs were in the reference's family.

use std::path::PathBuf;

use pid_plot::compare::{
    CompareReport, MaskRect, TextOffsetClass, UnpairedReason, UnpairedStroke,
    compare_plots_with_model, template_masks, write_overlay, write_report,
};
use pid_plot::fonts::{font_dirs, prepare_fonts, typeface_key};
use pid_plot::model::{PaperModel, PaperShape, TextAnchor, lay_out_drawing};
use pid_plot::read::{PlotInk, read_plot_with_faces};
use pid_plot::write::{PlotFont, write_plot};

/// One drawing's measured reading, as an acceptance floor/ceiling.
struct Pin {
    name: &'static str,
    /// Page the reference plot is, millimetres.
    page_mm: [f64; 2],
    min_coverage: f64,
    max_extra_mm: f64,
    /// Reference ink, millimetres, that no ink of ours covers. Ten pages read
    /// 0.000 once the signal-line dash is fitted to every straight piece; the
    /// A2 remainder is the instrument cluster the `.pid` stores elsewhere.
    max_uncovered_mm: f64,
    min_text_matched: usize,
    /// Reference labels the page counts, so a page that stops counting its
    /// reference's labels cannot pass by matching fewer of them.
    text_reference: usize,
    min_pen_agreed: usize,
    max_pen_differed: usize,
    /// Our strokes the pen pairing leaves unpaired, by why.
    unpaired: Unpaired,
    /// Symbols the corpus cannot draw. The A0 wellsites both place a
    /// `pulsation damper.sym` the reference corpus never ships.
    missing_symbols: &'static [&'static str],
    /// The same page with no template mask.
    whole: WholePage,
}

/// One page's unpaired strokes by how the reference's ink lies around them
/// (`CompareReport::pen`'s `unpaired_strokes`). Each count is a ceiling, and
/// the four together are every unpaired stroke on the page.
struct Unpaired {
    /// Template furniture with no reference ink within 0.5 mm of it: border
    /// the reference plot does not print.
    template: usize,
    /// Other ink with no reference ink within 0.5 mm of it.
    bare: usize,
    /// Ink within 0.5 mm of the reference's in places and farther in others.
    partly: usize,
    /// Ink within 0.5 mm of the reference's all along, on none of its strokes.
    beside: usize,
}

/// One drawing's reading with its furniture compared too: only the
/// missing-symbol boxes are left out.
struct WholePage {
    min_coverage: f64,
    max_uncovered_mm: f64,
    max_extra_mm: f64,
    min_text_matched: usize,
    text_reference: usize,
    /// Paired labels past [`LABEL_POSITION_MM`] or [`LABEL_WIDTH_MM`], an
    /// equality: one more is a label that moved, one fewer a pin to lower.
    text_off: usize,
    /// Paired runs of ours set in a font family their label names no face of
    /// — a borrowed `℃`, a borrowed `动火点` — an equality.
    borrowed: usize,
    /// Of [`Self::borrowed`], the runs the reference letters in the same
    /// family ([`FAMILIES`]). It may only rise, and the pin rises with it:
    /// fewer is a run that left the reference's family, more a pin to raise.
    borrowed_in_family: usize,
}

/// One bridge's worth of slack above the reading, as `plot_compare.rs` holds
/// its ceilings: a symbol body leaking back in fails, a threshold crossing
/// does not.
const EXTRA_SLACK_MM: f64 = 0.5;

/// Largest mean down-shift a lettering class may show, in body heights.
///
/// Held on every class a page has — a class of one label and `unknown` with
/// the rest — in the masked view and on the whole page. No class on the
/// eleven pages reads past ±0.016 (A2's `Left/Middle`, twelve labels, and its
/// one `Left/Top` label). The valve and motor `M` (`symbol/Middle`, 14–29
/// labels on each A0 wellsite) read +0.094…+0.098 while a symbol body's
/// ascent-measured height was stepped down as if it were a cap height; this
/// is the gate that would have caught it.
const CLASS_DOWN_HEIGHTS: f64 = 0.04;

/// Largest mean width difference a lettering class may show, in body heights.
///
/// Held on every class, as [`CLASS_DOWN_HEIGHTS`] is. No class reads past
/// ±0.015, and only one past ±0.0021: the two fused valve tags left in
/// `J K`'s `unknown`, +0.0146, which is the reader joining two labels into
/// one run and not the lettering. A title block's two `Right/Bottom`
/// certificate captions read +0.0149 on seven pages while a label's `Tm`
/// carried the plotter's own stretch and not the driver's 256ths
/// (`write/text_matrix.rs`). Every
/// `Center` class read about +0.028 while an upright two-glyph run kept its
/// natural advance rather than the whole-pixel `lpDx` steps the plotter
/// spaces it by, and the A2 `Left/Ascender` class read −0.320 while its 8 pt
/// Arial line labels were lettered at the style size rather than the
/// reference's `Tf 11.16`. `Left/Ascender part` — the two pieces of a label
/// such as `L=35℃`, whose `℃` the style's Arial Narrow cannot draw — read
/// +0.0294 on the eight pages that have one while the borrowed `℃` was set
/// at the label's own 10.17 pt instead of the reference's `Tf 9.6`; it reads
/// −0.0003 now.
const CLASS_ADVANCE_HEIGHTS: f64 = 0.02;

/// How far a paired label's anchor point — the start of a left-anchored run,
/// the middle of a centred one, the end of a right-anchored one — may land
/// from the reference's before the label is counted in
/// [`WholePage::text_off`], millimetres.
///
/// A class mean cannot see one label: a class of two hundred averages a
/// label half a millimetre out down to nothing. This limit and
/// [`LABEL_WIDTH_MM`] are the text census's
/// (`probe_why_paired_labels_miss_the_reference`), which named a cause for
/// each of the 64 labels then past them
/// (`docs/analysis/2026-11-08-the-eight-text-residuals-are-the-drivers-own-placements.md`).
/// Fourteen of the 64 were the borrowed runs — twelve `℃` 0.201 mm wide and
/// the A2 sheet's two `动火点` 1.863 mm narrow — and came inside both limits
/// when they were lettered as the reference letters them. Three were the
/// air-cooler sheets' `SP:`, inside once each run of a record took its own
/// whole-pixel start. Forty-four were the title blocks' SimSun labels at
/// `800`, 0.05 to 0.43 mm wide or narrow while their `Tm` carried the
/// plotter's own stretch; they are inside now that it carries the driver's
/// 256ths (`write/text_matrix.rs`). The eleven pages come to 3: `J K`'s
/// `1-1/2"` and `FC`, which the reader joins into one run, and the A2
/// sheet's two boxed `仪表风管线`.
const LABEL_POSITION_MM: f64 = 0.15;

/// How much wider or narrower than the reference's a paired label's run may
/// be before the label is counted in [`WholePage::text_off`], millimetres.
const LABEL_WIDTH_MM: f64 = 0.05;

/// The font families a plot letters in, each with the names its faces carry
/// in a PDF.
///
/// A borrowed run is held to the reference's family rather than to its face.
/// 宋体 and 新宋体 are one design — NSimSun is SimSun with a fixed-pitch
/// Latin — so a `动火点` the reference sets in NSimSun and a machine without
/// that file sets in SimSun is the same lettering, while a `℃` set in
/// FangSong against the reference's NSimSun is not. A face is listed under
/// every name it goes by:
/// the reference's driver writes `ArialNarrow`, while this crate's writer
/// embeds a program under its own family name, `Arial Narrow`, which is also
/// what [`typeface_key`] calls the face a label names.
///
/// A name in no row fails the page that reads it, so a face new to these
/// pages is given its family on purpose.
const FAMILIES: [(&str, &[&str]); 4] = [
    ("Arial", &["Arial"]),
    ("Arial Narrow", &["Arial Narrow", "ArialNarrow"]),
    ("FangSong", &["FangSong"]),
    ("Song", &["SimSun", "NSimSun"]),
];

const PULSATION_DAMPER: &str = r"piping\specialty components\misc\pulsation damper.sym";

const PINS: [Pin; 11] = [
    Pin {
        name: "I井场 注采阀组工艺及自控流程图",
        page_mm: [1189.0, 841.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 546,
        text_reference: 546,
        min_pen_agreed: 9370,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[PULSATION_DAMPER],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 0.0 + EXTRA_SLACK_MM,
            min_text_matched: 583,
            text_reference: 583,
            // The six were the title block's: their `Tm` now states the
            // stretch the way the driver does (`write/text_matrix.rs`).
            text_off: 0,
            borrowed: 2,
            borrowed_in_family: 2,
        },
    },
    Pin {
        name: "J K井场 注采阀组工艺及自控流程图",
        page_mm: [1189.0, 841.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 775,
        text_reference: 775,
        min_pen_agreed: 14709,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[PULSATION_DAMPER],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 0.0 + EXTRA_SLACK_MM,
            min_text_matched: 813,
            text_reference: 813,
            // Six were the title block's, as on the I sheet. The one left is
            // `1-1/2"` beside `FC`: two labels 1.185 mm apart in height that
            // the reader joins into one run because our page shows them one
            // after the other, and whose joined run it measures along the
            // slant, 0.058 mm long. The ink is where the reference's is.
            text_off: 1,
            borrowed: 0,
            borrowed_in_family: 0,
        },
    },
    Pin {
        name: "L M井场 注采阀组工艺及自控流程图",
        page_mm: [1189.0, 841.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 921,
        text_reference: 921,
        min_pen_agreed: 17866,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[PULSATION_DAMPER],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 0.0 + EXTRA_SLACK_MM,
            min_text_matched: 959,
            text_reference: 959,
            // The six were the title block's, as on the I sheet.
            text_off: 0,
            borrowed: 2,
            borrowed_in_family: 2,
        },
    },
    Pin {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 151,
        text_reference: 151,
        min_pen_agreed: 1278,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 286,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 3478.898 + EXTRA_SLACK_MM,
            min_text_matched: 155,
            text_reference: 155,
            text_off: 0,
            borrowed: 0,
            borrowed_in_family: 0,
        },
    },
    Pin {
        name: "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 123,
        text_reference: 123,
        min_pen_agreed: 1425,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 0.0 + EXTRA_SLACK_MM,
            min_text_matched: 161,
            text_reference: 161,
            // The six were the title block's, as on the I sheet.
            text_off: 0,
            borrowed: 0,
            borrowed_in_family: 0,
        },
    },
    Pin {
        name: "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 167,
        text_reference: 167,
        min_pen_agreed: 1695,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 286,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 3478.694 + EXTRA_SLACK_MM,
            min_text_matched: 171,
            text_reference: 171,
            text_off: 0,
            borrowed: 2,
            borrowed_in_family: 2,
        },
    },
    Pin {
        name: "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 249,
        text_reference: 249,
        min_pen_agreed: 3196,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 0.0 + EXTRA_SLACK_MM,
            min_text_matched: 253,
            text_reference: 253,
            text_off: 0,
            borrowed: 2,
            borrowed_in_family: 2,
        },
    },
    Pin {
        // The riser's stamped glyph centres itself now, so this sheet keeps
        // no extra ink at all: see `docs/plans/2026-11-02-the-a1-stem.md`.
        // The air cooler's two fan ovals are `0x0063 igEllipse2d` records in
        // both the `.sym` and the cached body; drawing them took uncovered
        // from 23.031 to 5.757 mm (plan §7), and fitting the signal dash to
        // every straight piece took the rest to 0.000.
        name: "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 288,
        text_reference: 288,
        min_pen_agreed: 3404,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 0.0 + EXTRA_SLACK_MM,
            min_text_matched: 326,
            text_reference: 326,
            // `SP:` was the seventh: its colon is 21 units after the label
            // here, as the reference's is, since each run of a record starts
            // on its own whole pixel (`write::record_run_start_px`). The six
            // were the title block's, as on the I sheet.
            text_off: 0,
            borrowed: 1,
            borrowed_in_family: 1,
        },
    },
    Pin {
        // Same glyph, same fix as the A sheet: no extra ink left. The
        // fan ovals draw as on the A sheet (plan §7).
        name: "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 290,
        text_reference: 290,
        min_pen_agreed: 3411,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 0.0 + EXTRA_SLACK_MM,
            min_text_matched: 328,
            text_reference: 328,
            // `SP:` was the seventh and the title block's the six, as on the
            // A sheet.
            text_off: 0,
            borrowed: 1,
            borrowed_in_family: 1,
        },
    },
    Pin {
        // Same glyph, same fix as the A sheet: no extra ink left. The
        // fan ovals draw as on the A sheet (plan §7).
        name: "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
        page_mm: [841.0, 594.0],
        min_coverage: 0.9999,
        max_extra_mm: 0.0 + EXTRA_SLACK_MM,
        max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
        min_text_matched: 283,
        text_reference: 283,
        min_pen_agreed: 3278,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 0,
            partly: 0,
            beside: 0,
        },
        missing_symbols: &[],
        whole: WholePage {
            min_coverage: 0.9999,
            max_uncovered_mm: 0.0 + EXTRA_SLACK_MM,
            max_extra_mm: 0.0 + EXTRA_SLACK_MM,
            min_text_matched: 321,
            text_reference: 321,
            // `SP:` was the seventh and the title block's the six, as on the
            // A sheet.
            text_off: 0,
            borrowed: 1,
            borrowed_in_family: 1,
        },
    },
    Pin {
        // The two `动火点` labels are the only whole-string font fallback on
        // these sheets, both 2.5 mm and centred. Stepping that run 9 device
        // units (1.524 mm) a glyph, each glyph at the reference's `Tf 9.6`,
        // brings both inside 0.07 mm. The mirrored valve's `D` pairs
        // once it letters from the descender line. The other eleven stay out.
        // The template sits where the border frame places it, (0.388, 0.566)
        // mm off the origin, which pairs two more of its strokes.
        // The logo's boundary arcs turn the way their direction byte says.
        // The near-circles drawn before lent eight chords to the scallop
        // strokes beside them; the short arcs pair four of their own, so
        // the pen tally is four below the rings' 2794.
        // Read whole, the arcs take the rings' 39.824 mm of extra ink off
        // the furniture; CPECC pairs at the height its style states and the
        // company name on the line its paragraph sets.
        name: "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
        page_mm: [594.0, 420.0],
        min_coverage: 0.9829,
        max_extra_mm: 187.091 + EXTRA_SLACK_MM,
        max_uncovered_mm: 208.671 + EXTRA_SLACK_MM,
        min_text_matched: 214,
        text_reference: 225,
        min_pen_agreed: 2806,
        max_pen_differed: 0,
        unpaired: Unpaired {
            template: 0,
            bare: 70,
            partly: 26,
            beside: 6,
        },
        missing_symbols: &[],
        whole: WholePage {
            min_coverage: 0.9881,
            max_uncovered_mm: 208.840 + EXTRA_SLACK_MM,
            max_extra_mm: 187.091 + EXTRA_SLACK_MM,
            min_text_matched: 250,
            text_reference: 261,
            // Two of the four were the company name's two lines, stated in
            // the driver's 256ths now. The two left are the boxed
            // `仪表风管线`, whose line hangs from the top of its box on the
            // reference page: 0.212 and 0.201 mm off.
            text_off: 2,
            borrowed: 3,
            borrowed_in_family: 3,
        },
    },
];

/// The three surveyed sheets, as `template_masks` answers them.
const MASK_SHEETS: [([f64; 2], [&str; 2]); 3] = [
    ([1189.0, 841.0], ["左侧会签栏与左上角框", "右下标题栏"]),
    ([841.0, 594.0], ["左侧会签栏与左上角框", "右下标题栏"]),
    // The CCHB A2 furniture: a 35.75 mm sign-off column down the left edge and
    // a 274 × 115 mm title block, both surveyed from the A2 reference plot.
    ([594.0, 420.0], ["左侧会签栏", "右下标题栏"]),
];

/// Font directories for this corpus, plus the self-check corpus's own set.
///
/// `验收数据` ships no fonts; the drawing corpus beside it does, and a run that
/// states `PID_PLOT_FONTS` still wins because `font_dirs` puts that list first.
fn font_search_dirs(corpus: &std::path::Path) -> Vec<PathBuf> {
    let mut dirs = font_dirs(Some(corpus));
    let beside = PathBuf::from("D:/work/cad/PID自研软件验证/fonts");
    if beside.is_dir() {
        dirs.push(beside);
    }
    dirs
}

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

/// Where the reports and overlays go: `PID_ACCEPTANCE_OUT`, else
/// `plot/acceptance` in the corpus. Two checkouts running this gate at once
/// must each name their own, or they write the same files.
fn report_dir(corpus: &std::path::Path) -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_OUT")
        .map_or_else(|| corpus.join("plot/acceptance"), PathBuf::from)
}

/// Run `git` and return its trimmed stdout, or `None` if it cannot run or exits
/// non-zero. Only stamps `BUILD.txt`; a missing git never fails the gate.
fn git_line(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Write the just-gated page bytes as the delivery under `<corpus>/plot`, plus a
/// `BUILD.txt` naming the commit and whether `pid-plot/src` was clean. Every page
/// has passed its pins by the time this runs, so the delivery is exactly the
/// bytes the gate verified, never a mix of old and new. `PID_ACCEPTANCE_DELIVER=0`
/// skips it, e.g. a second checkout running the gate against the same corpus.
fn deliver_plots(corpus: &std::path::Path, delivered: &[(&str, Vec<u8>)]) {
    if std::env::var("PID_ACCEPTANCE_DELIVER").ok().as_deref() == Some("0") {
        return;
    }
    let plot_dir = corpus.join("plot");
    std::fs::create_dir_all(&plot_dir).expect("create delivery dir");
    for (name, bytes) in delivered {
        std::fs::write(plot_dir.join(format!("{name}.pdf")), bytes).expect("write delivery pdf");
    }
    let head = git_line(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    let tree = match git_line(&["rev-parse", "--show-toplevel"]) {
        Some(root) => match git_line(&["-C", &root, "status", "--porcelain", "--", "pid-plot/src"])
        {
            Some(status) if status.is_empty() => "clean",
            Some(_) => "dirty",
            None => "unknown",
        },
        None => "unknown",
    };
    let written_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let build = format!(
        "HEAD: {head}\nplot_src: {tree}\nwritten_unix: {written_unix}\npages: {}\n",
        delivered.len()
    );
    std::fs::write(plot_dir.join("BUILD.txt"), build).expect("write BUILD.txt");
}

/// A page that matches no surveyed sheet gets no masks at all.
#[test]
fn only_the_surveyed_sheets_carry_furniture_masks() {
    for (page_mm, labels) in MASK_SHEETS {
        let masks = template_masks(page_mm);
        let found: Vec<&str> = masks.iter().map(|mask| mask.label.as_str()).collect();
        assert_eq!(found, labels, "{page_mm:?}");
    }
    assert!(template_masks([300.0, 200.0]).is_empty());
    // The A2 survey must not swallow the drawing's own left-edge continuation
    // ink: its sign-off column stops at 35.75 mm, well inside the 42 mm double
    // rails the reference inks at x 50.969..92.964.
    let a2 = template_masks([594.0, 420.0]);
    assert!(a2.iter().all(|mask| mask.max_mm[0] <= 594.0));
    assert!(
        a2.iter()
            .any(|mask| mask.min_mm[0] <= 0.0 && mask.max_mm[0] <= 36.0),
        "the A2 sign-off column must stay inside the drawing's first 36 mm"
    );
}

#[test]
fn eleven_reference_plots_hold_their_readings() {
    let corpus = corpus_dir();
    let first_reference = corpus.join(format!("{}.pdf", PINS[0].name));
    if !first_reference.is_file() {
        eprintln!(
            "skipping the acceptance gate; {} is absent",
            first_reference.display()
        );
        return;
    }
    let out_dir = report_dir(&corpus);
    let whole_dir = out_dir.join("whole-page");
    std::fs::create_dir_all(&whole_dir).expect("mkdir acceptance");

    let models: Vec<PaperModel> = PINS
        .iter()
        .map(|pin| {
            let path = corpus.join(format!("{}.pid", pin.name));
            lay_out_drawing(&path, &corpus).unwrap_or_else(|error| {
                panic!("{}: layout: {error}", pin.name);
            })
        })
        .collect();
    let dirs = font_search_dirs(&corpus);
    let prepared = prepare_fonts(&models.iter().collect::<Vec<_>>(), &dirs).expect("fonts");
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();

    // D1/S1: collect each page's gated bytes, then write them as the delivery
    // only after all eleven pass, so the delivery can never drift from what the
    // gate verified (see `deliver_plots`).
    let mut delivered: Vec<(&str, Vec<u8>)> = Vec::new();
    let mut borrowed_on_all_pages = 0usize;
    let mut in_family_on_all_pages = 0usize;

    for (pin, model) in PINS.iter().zip(&models) {
        let name = pin.name;
        let reference_bytes = std::fs::read(corpus.join(format!("{name}.pdf"))).expect("reference");
        let (reference, reference_faces) =
            read_plot_with_faces(&reference_bytes).expect("read reference");
        for axis in 0..2 {
            assert!(
                (reference.page_mm[axis] - pin.page_mm[axis]).abs() < 1.0,
                "{name} reference page {:?} is not {:?}",
                reference.page_mm,
                pin.page_mm
            );
        }
        let page = model.resolved_page_mm().expect("page");
        assert!(
            (page[0] - pin.page_mm[0]).abs() < 1.0 && (page[1] - pin.page_mm[1]).abs() < 1.0,
            "{name} page {page:?} is not the reference's {:?}",
            pin.page_mm
        );

        let our_bytes = write_plot(model, &fonts).expect("write ours");
        delivered.push((name, our_bytes.clone()));
        let (ours, our_faces) = read_plot_with_faces(&our_bytes).expect("read ours");
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
        let report = compare_plots_with_model(&reference, &ours, &template, &missing, model);
        write_report(&out_dir.join(format!("{name}.json")), &report).expect("json");
        write_overlay(
            &out_dir.join(format!("{name}.png")),
            &reference,
            &ours,
            &report.fit,
        )
        .expect("overlay");

        assert!(
            report.fit.within_threshold,
            "{name} fit outside threshold: {:?}",
            report.fit
        );
        assert!(
            report.coverage >= pin.min_coverage,
            "{name} coverage {:.4} < {:.4}",
            report.coverage,
            pin.min_coverage
        );
        assert!(
            report.extra_length_mm <= pin.max_extra_mm,
            "{name} extra {:.3} mm > {:.3} mm",
            report.extra_length_mm,
            pin.max_extra_mm
        );
        let uncovered_mm = report.reference_length_mm - report.covered_length_mm;
        assert!(
            uncovered_mm <= pin.max_uncovered_mm,
            "{name} uncovered {uncovered_mm:.3} mm > {:.3} mm",
            pin.max_uncovered_mm
        );
        assert_eq!(
            report.text_reference, pin.text_reference,
            "{name} reference label count moved"
        );
        assert!(
            report.text_matched >= pin.min_text_matched,
            "{name} text_matched {} < {}",
            report.text_matched,
            pin.min_text_matched
        );
        let pen = &report.pen;
        assert_eq!(
            pen.paired,
            pen.agreed + pen.differed,
            "{name} pen pairs {}/{}/{}",
            pen.paired,
            pen.agreed,
            pen.differed
        );
        assert!(
            pen.agreed >= pin.min_pen_agreed,
            "{name} pen agreed {} < {}",
            pen.agreed,
            pin.min_pen_agreed
        );
        assert!(
            pen.differed <= pin.max_pen_differed,
            "{name} pen differed {} > {}",
            pen.differed,
            pin.max_pen_differed
        );
        assert_eq!(
            pen.unpaired_strokes.len(),
            pen.unpaired,
            "{name} every unpaired stroke carries a reason"
        );
        let unpaired = |keep: &dyn Fn(&UnpairedStroke) -> bool| {
            pen.unpaired_strokes
                .iter()
                .filter(|stroke| keep(stroke))
                .count()
        };
        for (label, count, ceiling) in [
            (
                "template",
                unpaired(&|stroke| stroke.reason == UnpairedReason::Bare && stroke.furniture),
                pin.unpaired.template,
            ),
            (
                "bare",
                unpaired(&|stroke| stroke.reason == UnpairedReason::Bare && !stroke.furniture),
                pin.unpaired.bare,
            ),
            (
                "partly bare",
                unpaired(&|stroke| stroke.reason == UnpairedReason::PartlyBare),
                pin.unpaired.partly,
            ),
            (
                "beside",
                unpaired(&|stroke| stroke.reason == UnpairedReason::Beside),
                pin.unpaired.beside,
            ),
        ] {
            assert!(
                count <= ceiling,
                "{name} unpaired {label} strokes {count} > {ceiling}"
            );
        }
        assert_eq!(
            unpaired(&|stroke| stroke.in_missing_box),
            0,
            "{name} an unpaired stroke lies in a missing-symbol box"
        );
        assert_classes_hold(name, "masked", &report.text_offsets);
        let missing_now: Vec<&str> = model.missing_symbols.iter().map(String::as_str).collect();
        assert_eq!(
            missing_now, pin.missing_symbols,
            "{name} missing symbol list moved"
        );

        let whole = compare_plots_with_model(&reference, &ours, &[], &missing, model);
        write_report(&whole_dir.join(format!("{name}.json")), &whole).expect("json");
        let floor = &pin.whole;
        assert!(
            whole.coverage >= floor.min_coverage,
            "{name} whole-page coverage {:.6} < {:.4}",
            whole.coverage,
            floor.min_coverage
        );
        let uncovered_mm = whole.reference_length_mm - whole.covered_length_mm;
        assert!(
            uncovered_mm <= floor.max_uncovered_mm,
            "{name} whole-page uncovered {uncovered_mm:.3} mm > {:.3} mm",
            floor.max_uncovered_mm
        );
        assert!(
            whole.extra_length_mm <= floor.max_extra_mm,
            "{name} whole-page extra {:.3} mm > {:.3} mm",
            whole.extra_length_mm,
            floor.max_extra_mm
        );
        assert_eq!(
            whole.text_reference, floor.text_reference,
            "{name} whole-page reference label count moved"
        );
        assert!(
            whole.text_matched >= floor.min_text_matched,
            "{name} whole-page text_matched {} < {}",
            whole.text_matched,
            floor.min_text_matched
        );
        // The furniture's lettering is in no class of the masked view at all,
        // so the same two thresholds are held again here, where it is.
        assert_classes_hold(name, "whole-page", &whole.text_offsets);
        let off = labels_off(&whole, &reference);
        assert!(
            off.len() <= floor.text_off,
            "{name} whole-page: {} paired labels land more than {LABEL_POSITION_MM} mm from \
             the reference's or are more than {LABEL_WIDTH_MM} mm wider or narrower, {} are \
             pinned; one of these is new:\n{}",
            off.len(),
            floor.text_off,
            off.join("\n")
        );
        assert!(
            off.len() >= floor.text_off,
            "{name} whole-page: {} paired labels are past the label limits where {} are \
             pinned; lower `text_off` to what is left:\n{}",
            off.len(),
            floor.text_off,
            off.join("\n")
        );
        let borrowed = borrowed_runs(
            name,
            &whole,
            model,
            (&reference, &reference_faces),
            (&ours, &our_faces),
        );
        let listed = borrowed
            .iter()
            .map(|run| run.line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            borrowed.len(),
            floor.borrowed,
            "{name} whole-page: the paired runs set in a family their label does not name \
             are not the {} pinned:\n{listed}",
            floor.borrowed
        );
        let in_family = borrowed.iter().filter(|run| run.in_family).count();
        assert!(
            in_family >= floor.borrowed_in_family,
            "{name} whole-page: {in_family} borrowed runs are in the reference's font family \
             where {} are pinned; one has left it:\n{listed}",
            floor.borrowed_in_family
        );
        assert!(
            in_family <= floor.borrowed_in_family,
            "{name} whole-page: {in_family} borrowed runs are in the reference's font family \
             and {} are pinned; raise `borrowed_in_family` to what it reads:\n{listed}",
            floor.borrowed_in_family
        );
        borrowed_on_all_pages += borrowed.len();
        in_family_on_all_pages += in_family;
    }
    eprintln!(
        "borrowed runs paired on the {} pages: {borrowed_on_all_pages}, of which \
         {in_family_on_all_pages} are in the reference's font family",
        PINS.len()
    );

    deliver_plots(&corpus, &delivered);
}

/// Hold every lettering class of one comparison to the two class thresholds.
///
/// No class is left out for being small or for being `unknown`: a class of
/// one label is that label's own reading, and the thresholds are wide of
/// every one of them.
fn assert_classes_hold(name: &str, view: &str, classes: &[TextOffsetClass]) {
    for class in classes {
        assert!(
            class.mean_down_heights.abs() <= CLASS_DOWN_HEIGHTS,
            "{name} {view} {} down {:+.4} body heights over {} labels, past ±{CLASS_DOWN_HEIGHTS}",
            class.class,
            class.mean_down_heights,
            class.count
        );
        assert!(
            class.mean_advance_heights.abs() <= CLASS_ADVANCE_HEIGHTS,
            "{name} {view} {} advance {:+.4} body heights over {} labels, past \
             ±{CLASS_ADVANCE_HEIGHTS}",
            class.class,
            class.mean_advance_heights,
            class.count
        );
    }
}

/// The paired labels of `report` past [`LABEL_POSITION_MM`] or
/// [`LABEL_WIDTH_MM`], one line each: its class, the reference's runs, how
/// far its anchor point is from the reference's and how much wider it is.
///
/// A label's anchor point is where its paper-table source says it hangs from,
/// so a centred run that is wider than the reference's by some amount and
/// starts half that amount early has not moved. A pair with no source is
/// measured from its start.
fn labels_off(report: &CompareReport, reference: &PlotInk) -> Vec<String> {
    report
        .text_pairs
        .iter()
        .filter_map(|pair| {
            let fraction = match pair.source.as_ref().map(|source| source.anchor) {
                Some(TextAnchor::Center) => 0.5,
                Some(TextAnchor::Right) => 1.0,
                Some(TextAnchor::Left) | None => 0.0,
            };
            let position_mm = (pair.along_mm + fraction * pair.advance_mm).hypot(pair.down_mm);
            if position_mm <= LABEL_POSITION_MM && pair.advance_mm.abs() <= LABEL_WIDTH_MM {
                return None;
            }
            let runs: Vec<&str> = pair
                .reference_runs
                .iter()
                .map(|index| reference.spans[*index].text.as_str())
                .collect();
            Some(format!(
                "  {} {runs:?}: {position_mm:.4} mm off, {:+.4} mm wide",
                pair.class, pair.advance_mm
            ))
        })
        .collect()
}

/// One paired run of ours set in a font family its label names no face of.
struct BorrowedRun {
    /// The label, what we set outside its named faces and what the reference
    /// sets there, for a failure to list.
    line: String,
    /// Whether the reference letters the label outside its named faces in
    /// the same families we do.
    in_family: bool,
}

/// The family a font name belongs to, by [`FAMILIES`].
fn family_of(face: &str) -> Option<&'static str> {
    FAMILIES
        .iter()
        .find(|(_, faces)| faces.contains(&face))
        .map(|(family, _)| *family)
}

/// The family of a face one of `name`'s pages reads, or a failure that says
/// whose face it is and asks for a row.
fn family(name: &str, whose: &str, face: &str) -> &'static str {
    family_of(face).unwrap_or_else(|| {
        panic!("{name}: {whose} face {face:?} is in no row of FAMILIES; give it a family")
    })
}

/// One run set outside its label's named families: the family, the face's
/// name on the page and what the run spells.
type Outside<'a> = (&'static str, &'a str, &'a str);

/// The runs of one side of a pair that are set in a family `named` does not
/// hold. A run of spaces inks nothing and is not counted.
fn outside_named<'a>(
    name: &str,
    whose: &str,
    runs: &[usize],
    (ink, faces): (&'a PlotInk, &'a [String]),
    named: &[&'static str],
) -> Vec<Outside<'a>> {
    runs.iter()
        .filter(|index| !ink.spans[**index].text.trim().is_empty())
        .map(|index| {
            (
                family(name, whose, &faces[*index]),
                faces[*index].as_str(),
                ink.spans[*index].text.as_str(),
            )
        })
        .filter(|(family, ..)| !named.contains(family))
        .collect()
}

/// The families `runs` are set in, each once, in order.
fn families(runs: &[Outside<'_>]) -> Vec<&'static str> {
    let mut families: Vec<&'static str> = runs.iter().map(|(family, ..)| *family).collect();
    families.sort_unstable();
    families.dedup();
    families
}

/// `runs` as a failure lists them: what each spells and the face it is in.
fn listed(runs: &[Outside<'_>]) -> String {
    if runs.is_empty() {
        return "nothing outside the named faces".to_string();
    }
    runs.iter()
        .map(|(_, face, text)| format!("{text:?} in {face}"))
        .collect::<Vec<_>>()
        .join(" + ")
}

/// The paired runs of `report` the page borrows a face for, each with
/// whether the reference letters it in the same family.
///
/// A label's style names a face, and a run inside it may name another;
/// [`typeface_key`] reads both. A run of ours set in a family none of those
/// faces belongs to is lettered in a borrowed face, the named one being
/// unable to draw it. That is read off the written page — the face each of
/// its spans is set in — so the writer's rule for choosing a lender is not
/// repeated here. The reference's side of the pair is read the same way, and
/// the two agree when the families they set outside the label's named faces
/// are the same: for a `℃` paired with a `℃`, ours is in the family of the
/// reference's.
///
/// Only a paired run can be held against the reference, and only a pair
/// whose label the comparison found says which faces are named. A borrowed
/// run that pairs with nothing is therefore not in this reading: the A2
/// sheet's `L=-5℃` and `H=-15℃` sit in the instrument cluster the `.pid`
/// stores 16.9 mm from where the reference prints it.
fn borrowed_runs(
    name: &str,
    report: &CompareReport,
    model: &PaperModel,
    (reference, reference_faces): (&PlotInk, &[String]),
    (ours, our_faces): (&PlotInk, &[String]),
) -> Vec<BorrowedRun> {
    let mut borrowed = Vec::new();
    for pair in &report.text_pairs {
        let Some(source) = &pair.source else {
            continue;
        };
        // `text_sources` copies a label's text and insertion point out of the
        // model, so the label is the one that holds exactly those.
        let (value, font, spans) = model
            .primitives
            .iter()
            .find_map(|primitive| match &primitive.shape {
                PaperShape::Text {
                    value,
                    origin_mm,
                    font,
                    spans,
                    ..
                } if *value == source.text && *origin_mm == source.origin_mm => {
                    Some((value, font, spans))
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!(
                    "{name}: no label of the model is the source {:?}",
                    source.text
                )
            });
        let mut named = vec![family(
            name,
            "a label's",
            typeface_key(font.as_deref(), value),
        )];
        for span in spans {
            if let Some(run_font) = span.font.as_deref() {
                named.push(family(
                    name,
                    "a label run's",
                    typeface_key(Some(run_font), value),
                ));
            }
        }
        let ours_outside = outside_named(name, "our", &pair.our_runs, (ours, our_faces), &named);
        if ours_outside.is_empty() {
            continue;
        }
        let reference_outside = outside_named(
            name,
            "the reference's",
            &pair.reference_runs,
            (reference, reference_faces),
            &named,
        );
        borrowed.push(BorrowedRun {
            line: format!(
                "  {value:?}: ours {}; the reference {}",
                listed(&ours_outside),
                listed(&reference_outside)
            ),
            in_family: families(&ours_outside) == families(&reference_outside),
        });
    }
    borrowed
}
