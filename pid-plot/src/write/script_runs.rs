//! One text object for each script run of a string.
//!
//! The plotter hands GDI a string in one call, with the whole-pixel step of
//! every glyph ([`super::run_advance`]). GDI does not hand it on in one
//! piece: it cuts the string where the script changes, and the PDF driver
//! writes one text object for each piece — its own text matrix, at the
//! whole pixel the steps before it add up to, and its own letter-spacing,
//! the average its own glyphs need. This module holds that cut ([`of`]) and
//! the two figures a piece is written with ([`start_px`], [`spacing_pt`]).
//!
//! # What the reference plots show
//!
//! Of the 6416 text objects on the eleven reference plots, none holds a Han
//! character and a Latin letter together, and none holds either with a
//! full-width parenthesis. The same labels on our pages were 189 text
//! objects that did. The reference's pieces, each a text object of its own:
//!
//! ```text
//! DWG-0100 | 集05-37                 a Latin run, then a Han run with its digits
//! 苏49 | K-P5 | 注采井                 digits stay with the run before them
//! 苏49-2老井                          Han, digits, Han: one run
//! G | 井场来
//! I | 井场 | （2 | 口井 | ）           a full-width parenthesis is a run of its own
//! CADD | 号
//! 1 版                               what leads a string joins its first run
//! i≥3‰     SP:30℃                    `≥`, `‰`, `℃` and `:` change nothing
//! 工程设计证书:  | A111017147  | 综甲     工程勘察证书:  | B111017147  | 综甲
//! ```
//!
//! So three kinds of character each start a run when the run in force is of
//! another kind — a Latin letter, a Han ideograph, a full-width form — and
//! every other character (a digit, a space, ASCII punctuation, a symbol)
//! stays in the run it follows, or in the first run when nothing precedes
//! it.
//!
//! The last row is the A2 sheet's certificate line, and it fixes where a
//! piece starts and how it is spaced. Its five text objects start 0, 134,
//! 238, 457 and 561 device units after the label, which is the sum of the
//! steps the plotter's own rounding gives the glyphs before each — carried
//! across the cuts, not started again at each — and their `Tc` are −0.494,
//! −0.288, −0.453, −0.288 and −0.576, each the average of what its own
//! glyphs need.
//!
//! # What it changes on the page
//!
//! Under a plain text matrix, nothing a comparison of whole runs reads: the
//! first and the last glyph of the string are where they were, and the
//! glyphs in between move by less than a device unit, onto the places the
//! reference's stand. It matters where the matrix is not the plotter's own
//! stretch: the driver writes a stretch in whole 256ths, a little off the
//! stretch itself, so glyphs under it drift from their steps, and the drift
//! ends where the text object does.
//!
//! # What the reference plots do not show
//!
//! - any punctuation of the CJK block (`、`, `。`, U+3000): left in the run
//!   it follows;
//! - a full-width form other than the two parentheses: taken with them;
//! - kana, hangul, Greek or Cyrillic letters: left in the run they follow;
//! - a turned label. Its glyphs are each set on their own step already.

use std::ops::Range;

use super::PLOT_DPI;

/// The three kinds of character that start a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Script {
    Latin,
    Han,
    FullWidth,
}

/// The kind `ch` starts a run of, or `None` for a character that stays in
/// the run it follows.
fn script(ch: char) -> Option<Script> {
    match ch {
        'A'..='Z' | 'a'..='z' => Some(Script::Latin),
        '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{20000}'..='\u{323AF}' => Some(Script::Han),
        '\u{FF01}'..='\u{FF60}' | '\u{FFE0}'..='\u{FFE6}' => Some(Script::FullWidth),
        _ => None,
    }
}

/// The script runs of `text`, in order, each the range of its characters.
///
/// A string of one script, or of none, is one run.
pub(super) fn of(text: &str) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut in_force = None;
    let mut count = 0;
    for (at, ch) in text.chars().enumerate() {
        count = at + 1;
        let Some(kind) = script(ch) else {
            continue;
        };
        if in_force.is_some_and(|run| run != kind) {
            runs.push(start..at);
            start = at;
        }
        in_force = Some(kind);
    }
    runs.push(start..count);
    runs
}

/// One glyph of a string as the plotter sets it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Stepped {
    /// The whole device units it is stepped by.
    pub(super) step_px: i64,
    /// Its own width in the device font, in device units.
    pub(super) natural_px: f64,
}

/// How many device units after the first glyph of a string the run that
/// starts at glyph `at` starts: the steps of every glyph before it.
pub(super) fn start_px(glyphs: &[Stepped], at: usize) -> i64 {
    glyphs.iter().take(at).map(|glyph| glyph.step_px).sum()
}

/// The letter-spacing of a run whose glyphs are `glyphs`, in points of a
/// text space scaled by `sx` along the baseline.
///
/// It is what [`super::stepped_advance`] gives a whole string: the average,
/// over every glyph but the last, of what its step is short of or past its
/// own width. A run of one glyph has no gap to space.
pub(super) fn spacing_pt(glyphs: &[Stepped], sx: f64) -> f64 {
    let Some((_, leading)) = glyphs.split_last() else {
        return 0.0;
    };
    if leading.is_empty() || !(sx.is_finite() && sx > 0.0) {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let stepped: f64 = leading.iter().map(|glyph| glyph.step_px as f64).sum();
    let natural: f64 = leading.iter().map(|glyph| glyph.natural_px).sum();
    #[allow(clippy::cast_precision_loss)]
    let gaps = leading.len() as f64;
    (stepped - natural) / gaps * (72.0 / PLOT_DPI) / sx
}

#[cfg(test)]
mod tests {
    use super::{Stepped, of, spacing_pt, start_px};

    /// `text` cut into its script runs.
    fn cut(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        of(text)
            .into_iter()
            .map(|run| chars[run].iter().collect())
            .collect()
    }

    /// The labels the reference plots write as more than one text object,
    /// cut where the reference cuts them.
    #[test]
    fn a_string_is_cut_where_the_reference_plots_cut_it() {
        for (text, pieces) in [
            ("DWG-0100集05-37", vec!["DWG-0100", "集05-37"]),
            ("苏49K-P5注采井", vec!["苏49", "K-P5", "注采井"]),
            ("G井场来", vec!["G", "井场来"]),
            ("采气生产管线去G井场", vec!["采气生产管线去", "G", "井场"]),
            ("I井场（2口井）", vec!["I", "井场", "（2", "口井", "）"]),
            ("老井井场（1口井）", vec!["老井井场", "（1", "口井", "）"]),
            ("CADD号", vec!["CADD", "号"]),
            (
                "天然气自压缩机X-01141出口来",
                vec!["天然气自压缩机", "X-01141", "出口来"],
            ),
            (
                "工程设计证书: A111017147 综甲     工程勘察证书: B111017147 综甲",
                vec![
                    "工程设计证书: ",
                    "A111017147 ",
                    "综甲     工程勘察证书: ",
                    "B111017147 ",
                    "综甲",
                ],
            ),
        ] {
            assert_eq!(cut(text), pieces, "{text}");
        }
    }

    /// The labels the reference plots write as one text object stay one:
    /// digits, spaces, punctuation and symbols belong to the run they
    /// follow, and what leads a string to its first run.
    #[test]
    fn a_string_of_one_script_is_one_run() {
        for text in [
            "苏49-2老井",
            "1 版",
            "i≥3‰",
            "SP:30℃",
            "SP=-8℃",
            "25-CI-1001901-B2-EH40",
            "H=11.8MPa",
            " 1001104A",
            "010065-kj",
            "华北油田苏桥储气库群",
            "集 05-35",
            "1-1/2\"",
            "",
            "  ",
        ] {
            assert_eq!(cut(text), vec![text.to_string()], "{text:?}");
        }
    }

    /// The certificate line of the A2 sheet: SimSun on a cell of 24 device
    /// units with a stated width of 10, so a full-width glyph is 20 units
    /// wide and a half-width one 10, and the plotter steps them 19 and 10 or
    /// 9. The reference's second text object starts 134 units after the
    /// first and its third 238, and the first two are spaced `Tc −0.494`
    /// and `Tc −0.288` under a matrix of 213/256.
    #[test]
    fn a_run_starts_on_the_steps_before_it_and_is_spaced_by_its_own_glyphs() {
        let glyph = |step_px: i64, natural_px: f64| Stepped {
            step_px,
            natural_px,
        };
        // 工程设计证书:␠ then A111017147␠, stepped the way the plotter's
        // rounding carries across the two.
        let mut glyphs = vec![glyph(19, 20.0); 6];
        glyphs.extend([glyph(10, 10.0), glyph(10, 10.0)]);
        for step in [9, 10, 9, 10, 9, 10, 9, 10, 9, 10, 9] {
            glyphs.push(glyph(step, 10.0));
        }
        assert_eq!(start_px(&glyphs, 0), 0);
        assert_eq!(start_px(&glyphs, 8), 134);
        assert_eq!(start_px(&glyphs, 19), 238);
        let written = 213.0 / 256.0;
        let cut_to_three = |tc: f64| (tc * 1000.0).trunc() / 1000.0;
        assert!((cut_to_three(spacing_pt(&glyphs[..8], written)) + 0.494).abs() < 1e-9);
        assert!((cut_to_three(spacing_pt(&glyphs[8..19], written)) + 0.288).abs() < 1e-9);
        // One glyph has no gap, and neither has none.
        assert_eq!(spacing_pt(&glyphs[..1], written), 0.0);
        assert_eq!(spacing_pt(&[], written), 0.0);
    }
}
