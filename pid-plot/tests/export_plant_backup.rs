//! Every `.pid` a plant backup holds exports a page.
//!
//! The five corpus drawings and the `test-file/` fixtures are checked
//! elsewhere. This gate covers the wider set the vendor's own
//! `PlantData~2~711.zip` carries — 53 drawings from one plant, shelled out of
//! the backup into `<corpus>/backup-plant-data` with their GBK names restored.
//!
//! What it pins, in the order a regression would show up:
//!
//! - every drawing lays out at all (a decoder that stops seeing a record
//!   family shows up as a page of the wrong size, or none);
//! - no stroke is refused for not fitting the sheet (`off_sheet_primitives`);
//! - no label reaches the writer with a line break in it — the writer has no
//!   face for `U+000D`, and one such label took the whole page down;
//! - no placement falls back to a placeholder box: a symbol the `.sym`
//!   libraries lack is still drawn from the body the `.pid` carries itself
//!   (`missing_boxes` is empty on all 53 — the 18 library gaps are drawn);
//! - every library gap is one of the known ones below, and no gap grows;
//! - every sheet wearing a border template wears the `.igr` it links at the size
//!   and stroke count that file states, its strokes start at the page origin,
//!   and each symbol its title block places sits at the point the `.igr` states
//!   — on every sheet wearing it, since a placement is the template's;
//! - every label that template letters is the same characters, at the same
//!   insertion point and the same body height, cell by cell — 225 cells over
//!   the seven templates, pinned per template and required to agree sheet by
//!   sheet, so a re-authored title block that drifts a cell or respells the
//!   company name fails even though its stroke count, extent and placements
//!   are untouched;
//! - the written page reads back at the size the model resolved — the declared
//!   sheet, or the standard sheet measured for the four drawings that state
//!   none — and weighs what a subset-embedded page weighs: after the `EBDT`
//!   fix each page is 175–846 KiB (an empty sheet is 722 B), before it
//!   6.6–7.1 MiB;
//!
//! Soft-skips when the extracted backup is absent, which is the case on CI.
//! It does not soft-skip once the backup is there and no face directory is:
//! the written-page pins cannot be read without a face, and reporting them
//! unread is not the same as reading them — see [`require_faces`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{
    PaperModel, PaperShape, STANDARD_SHEETS_MM, SymbolBodySource, lay_out_drawing,
};
use pid_plot::read::read_plot;
use pid_plot::write::{PlotFont, WIDEST_PEN_MM, write_plot};

/// The reference share this machine keeps the licensed faces in.
fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// Where the shelled-out plant backup sits, if this machine has one.
///
/// The extraction is a local artifact, not a repository fixture, so the gate
/// looks beside the corpus and then in the backup package itself. `None`
/// soft-skips the test, which is what CI sees.
fn backup_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(corpus_dir());
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots
        .into_iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|candidate| candidate.is_dir())
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

/// Symbol files the plant's own reference share does not carry.
///
/// `Pulsation Damper.sym`, a bare `Wellhead Christmas Tree.sym` (the share has
/// the `01`/`02` variants only) and one renamed temperature transmitter are
/// absent from both the corpus `Ref` and the bundled `test-file/symbols-full`,
/// and from the backup archives themselves. Pinned so a *new* gap fails rather
/// than quietly drawing less.
const KNOWN_LIBRARY_GAPS: [&str; 3] = [
    r"piping\specialty components\misc\pulsation damper.sym",
    r"equipment\other equipment\wellhead christmas tree.sym",
    r"instrumentation\off-line\without implied components\temperature\clone of tt-temperature transmitter(1).sym",
];

/// `PlantData~2~711.zip` holds 53 sheets. A ratchet floor, not an equality:
/// a fuller extraction supersedes this one rather than failing the gate.
const BACKUP_SHEETS: usize = 53;

/// Every page is a few hundred KiB once the faces are subset. The bitmap
/// strike that used to ride along pushed each one past 6 MiB.
const PAGE_CEILING: usize = 2 * 1024 * 1024;

/// Sheets whose `.pid` states no page at all.
///
/// `D井场`, `Q井场`, `换热器CD` and `凝析油一级闪蒸分离器B气相出口改造` carry
/// neither an `igSmartFrame2d` nor a template name, so
/// [`PaperModel::resolved_page_mm`] measures one instead: A0, A1, A1 and A2
/// respectively. Pinned as an equality, both ways round — a new drawing with no
/// sheet fails here rather than quietly taking the ink's own box, and a sheet
/// that starts stating its page lowers this number deliberately.
const DERIVED_PAGES: usize = 4;

/// Sheets that carry a border template. A floor for the same reason
/// [`BACKUP_SHEETS`] is one: a fuller extraction only adds sheets — the 16 that
/// sat on none until the backup's own `RefData~4~682.zip`, whose templates the
/// corpus now carries, gave them their frames.
const TEMPLATED_SHEETS: usize = 47;

/// One symbol a pinned template places, as the `.igr` states it: the file name
/// the placement stores and the point it sits at, in paper millimetres.
type PinnedPlacement = (&'static str, [f64; 2]);

/// One label a pinned template letters, as the `.igr` states it: the
/// characters, the insertion point in paper millimetres, and the body height in
/// millimetres.
///
/// `f64::NAN` stands for a record that states no height; none of the seven
/// templates here does, and a `NAN` matches only another `NAN`.
type PinnedLabel = (&'static str, [f64; 2], f64);

/// One pinned template: the file name, the extent and stroke count its `.igr`
/// states, what that `.igr` places, and what it letters.
type PinnedTemplate = (
    &'static str,
    [f64; 2],
    usize,
    &'static [PinnedPlacement],
    &'static [PinnedLabel],
);

/// The 34 cells `CPE-SQ-A0.igr` letters, sorted by text then position.
///
/// Twenty of them run down the left edge's sign-off column and fourteen sit
/// in the title block. The `CPE-SQ` family is one template re-authored at
/// three sizes, and the two blocks hang from different edges, which is what
/// the three lists show: the column's `x` is 24 mm from the left edge on all
/// three of them and its `y` counts down from the *top* edge, so A0's column
/// sits 247.000 mm below A1's (the difference between the two page heights)
/// and A1's 174.000 mm below A2's; the title block's `y` is the same on all
/// three — it counts up from the *bottom* edge — and its `x` counts in from
/// the right, so A0's title block is 348.000 mm right of A1's (the difference
/// between the two page widths) and A1's 247.000 mm right of A2's.
static CPE_SQ_A0_LABELS: [PinnedLabel; 34] = [
    ("010065-kj", [1137.931554, 66.349430], 3.500000),
    ("1 版", [1172.607467, 12.694865], 3.000000),
    ("2012,05,15", [1100.447381, 12.813829], 3.200000),
    ("A111001641", [1091.960694, 66.560638], 3.500000),
    ("CADD号", [1120.587282, 25.124944], 3.200000),
    ("CPENC-DD11008", [1144.435787, 12.694865], 3.200000),
    (
        "中国石油集团工程设计有限责任公司华北分公司",
        [1063.413661, 74.145144],
        4.500000,
    ),
    ("会 签 专 业", [23.999970, 693.500016], 3.000000),
    ("会 签 专 业", [23.999970, 768.500016], 3.000000),
    ("修改", [24.225908, 643.746055], 3.000000),
    ("修改号", [24.225908, 598.289181], 3.000000),
    ("制  图", [1046.499970, 61.194323], 3.200000),
    ("勘察证书号", [1136.391170, 66.014058], 3.500000),
    ("华北油田苏桥储气库群", [1125.246205, 57.730800], 4.000000),
    ("审  定", [1046.499970, 37.143917], 3.200000),
    ("审  核", [1046.499970, 43.160719], 3.200000),
    ("审核", [24.225908, 658.773122], 3.000000),
    ("批  准", [1046.499970, 31.143917], 3.200000),
    ("文件号", [1120.587282, 19.311584], 3.200000),
    ("施工图", [1099.732930, 24.762086], 3.200000),
    ("日    期", [23.999970, 743.500016], 3.000000),
    ("日    期", [23.999970, 818.500016], 3.000000),
    ("日期", [1081.127194, 13.006008], 3.200000),
    ("日期", [24.225908, 628.906828], 3.000000),
    ("校  对", [1046.499970, 48.923321], 3.200000),
    ("次数", [24.225908, 585.891852], 3.000000),
    ("比例", [1077.328358, 17.550016], 3.200000),
    ("签     署", [23.999970, 718.500016], 3.000000),
    ("签     署", [23.999970, 793.500016], 3.000000),
    ("设  计", [1046.499970, 55.194323], 3.200000),
    ("设计证书号", [1090.531612, 66.240398], 3.500000),
    ("阶段", [1077.279206, 23.542724], 3.200000),
    ("页次", [24.225908, 613.316247], 3.000000),
    ("项目号", [1120.587282, 13.316557], 3.200000),
];

/// The same 34 cells as [`CPE_SQ_A0_LABELS`], on A1's page.
static CPE_SQ_A1_LABELS: [PinnedLabel; 34] = [
    ("010065-kj", [789.931554, 66.349412], 3.500000),
    ("1 版", [824.607467, 12.694846], 3.000000),
    ("2012,05,15", [752.447381, 12.813810], 3.200000),
    ("A111001641", [743.960694, 66.560620], 3.500000),
    ("CADD号", [772.587282, 25.124926], 3.200000),
    ("CPENC-DD11008", [796.435787, 12.694846], 3.200000),
    (
        "中国石油集团工程设计有限责任公司华北分公司",
        [715.413661, 74.145125],
        4.500000,
    ),
    ("会 签 专 业", [23.999970, 446.499998], 3.000000),
    ("会 签 专 业", [23.999970, 521.499998], 3.000000),
    ("修改", [24.225908, 396.746037], 3.000000),
    ("修改号", [24.225908, 351.289163], 3.000000),
    ("制  图", [698.499970, 61.194305], 3.200000),
    ("勘察证书号", [788.391170, 66.014040], 3.500000),
    ("华北油田苏桥储气库群", [777.246205, 57.730782], 4.000000),
    ("审  定", [698.499970, 37.143899], 3.200000),
    ("审  核", [698.499970, 43.160701], 3.200000),
    ("审核", [24.225908, 411.773103], 3.000000),
    ("批  准", [698.499970, 31.143899], 3.200000),
    ("文件号", [772.587282, 19.311566], 3.200000),
    ("施工图", [751.732930, 24.762068], 3.200000),
    ("日    期", [23.999970, 496.499998], 3.000000),
    ("日    期", [23.999970, 571.499998], 3.000000),
    ("日期", [733.127194, 13.005989], 3.200000),
    ("日期", [24.225908, 381.906810], 3.000000),
    ("校  对", [698.499970, 48.923303], 3.200000),
    ("次数", [24.225908, 338.891833], 3.000000),
    ("比例", [729.328358, 17.549998], 3.200000),
    ("签     署", [23.999970, 471.499998], 3.000000),
    ("签     署", [23.999970, 546.499998], 3.000000),
    ("设  计", [698.499970, 55.194305], 3.200000),
    ("设计证书号", [742.531612, 66.240380], 3.500000),
    ("阶段", [729.279206, 23.542706], 3.200000),
    ("页次", [24.225908, 366.316229], 3.000000),
    ("项目号", [772.587282, 13.316539], 3.200000),
];

/// The same 34 cells as [`CPE_SQ_A0_LABELS`], on A2's page.
static CPE_SQ_A2_LABELS: [PinnedLabel; 34] = [
    ("010065-kj", [542.931554, 66.349408], 3.500000),
    ("1 版", [577.607467, 12.694843], 3.000000),
    ("2012,05,15", [505.447381, 12.813807], 3.200000),
    ("A111001641", [496.960694, 66.560616], 3.500000),
    ("CADD号", [525.587282, 25.124922], 3.200000),
    ("CPENC-DD11008", [549.435787, 12.694843], 3.200000),
    (
        "中国石油集团工程设计有限责任公司华北分公司",
        [468.413661, 74.145122],
        4.500000,
    ),
    ("会 签 专 业", [23.999970, 272.499995], 3.000000),
    ("会 签 专 业", [23.999970, 347.499995], 3.000000),
    ("修改", [24.225908, 222.746034], 3.000000),
    ("修改号", [24.225908, 177.289159], 3.000000),
    ("制  图", [451.499970, 61.194301], 3.200000),
    ("勘察证书号", [541.391170, 66.014037], 3.500000),
    ("华北油田苏桥储气库群", [530.246205, 57.730778], 4.000000),
    ("审  定", [451.499970, 37.143895], 3.200000),
    ("审  核", [451.499970, 43.160697], 3.200000),
    ("审核", [24.225908, 237.773100], 3.000000),
    ("批  准", [451.499970, 31.143895], 3.200000),
    ("文件号", [525.587282, 19.311562], 3.200000),
    ("施工图", [504.732930, 24.762064], 3.200000),
    ("日    期", [23.999970, 322.499995], 3.000000),
    ("日    期", [23.999970, 397.499995], 3.000000),
    ("日期", [486.127194, 13.005986], 3.200000),
    ("日期", [24.225908, 207.906806], 3.000000),
    ("校  对", [451.499970, 48.923299], 3.200000),
    ("次数", [24.225908, 164.891830], 3.000000),
    ("比例", [482.328358, 17.549995], 3.200000),
    ("签     署", [23.999970, 297.499995], 3.000000),
    ("签     署", [23.999970, 372.499995], 3.000000),
    ("设  计", [451.499970, 55.194301], 3.200000),
    ("设计证书号", [495.531612, 66.240376], 3.500000),
    ("阶段", [482.279206, 23.542702], 3.200000),
    ("页次", [24.225908, 192.316225], 3.000000),
    ("项目号", [525.587282, 13.316535], 3.200000),
];

/// The 37 cells `CPECCHB-SQ-A1.igr` letters.
///
/// Different furniture from the `CPE-SQ` family: its title block (`CPECC`, the
/// company name split across two cells, `项目经理`, an unspaced `专业`) and its
/// own set of certificate rows.
///
/// Ten of the 37 repeat a string the file letters twice, a hair apart: `专业`,
/// `比例`, `版次`, `阶段`, `项目号` twice each and `制图`, `审定`, `审核`,
/// `校对`, `设计` three times each, one copy at 2.499745 mm and one at
/// 2.500000 mm with its insertion point a few thousandths of a millimetre
/// away. That is the template, not the reader: the `.igr` carries the cell
/// twice. Pinned as measured — a change that stops double-lettering is worth a
/// look, and this is the only pin that would say so.
///
/// It also letters `签     暑` where the draughtsman meant `署`, which is the
/// kind of thing a text pin is for: the characters are the file's, and a
/// "helpful" correction anywhere in the reader would fail here.
static CPECCHB_SQ_A1_LABELS: [PinnedLabel; 37] = [
    ("CADD号", [698.513504, 5.647319], 2.499745),
    ("CPECC", [698.592631, 66.663703], 2.244375),
    ("专业", [770.298519, 13.179031], 2.499745),
    ("专业", [770.292325, 13.179355], 2.500000),
    (
        "中国石油工程建设有限公司",
        [753.954102, 76.435606],
        5.500000,
    ),
    ("仪  控", [668.757500, 59.296062], 2.499745),
    ("会 签 专 业", [23.589985, 521.499994], 2.500000),
    ("制图", [660.444788, 53.296675], 2.499745),
    ("制图", [695.441217, 59.296062], 2.499745),
    ("制图", [695.427384, 59.301093], 2.500000),
    ("华北分公司", [808.031586, 76.435606], 5.000000),
    ("审定", [660.666459, 29.997946], 2.499745),
    ("审定", [695.662887, 35.997334], 2.499745),
    ("审定", [695.649078, 35.999987], 2.500000),
    ("审核", [660.463523, 36.067340], 2.499745),
    ("审核", [695.459952, 42.066728], 2.499745),
    ("审核", [695.446121, 42.070000], 2.500000),
    (
        "工程设计证书: A111017147 综甲     工程勘察证书: B111017147 综甲",
        [731.990697, 66.885704],
        2.000000,
    ),
    ("文件号", [770.727377, 19.399462], 2.499745),
    ("日    期", [23.595003, 571.557849], 2.500000),
    ("日期", [733.656859, 13.385600], 2.499745),
    ("校对", [660.463523, 42.077323], 2.499745),
    ("校对", [695.459952, 48.076710], 2.499745),
    ("校对", [695.446121, 48.080596], 2.500000),
    ("核准", [695.366657, 29.628482], 2.499745),
    ("比例", [733.526260, 19.062941], 2.499745),
    ("比例", [733.516314, 19.063866], 2.500000),
    ("版次", [812.184351, 13.356468], 2.499745),
    ("版次", [812.182432, 13.356810], 2.500000),
    ("签     暑", [23.599454, 546.455491], 2.500000),
    ("设计", [660.456644, 47.966667], 2.499745),
    ("设计", [695.453073, 53.966055], 2.499745),
    ("设计", [695.439241, 53.970541], 2.500000),
    ("阶段", [733.436817, 24.609254], 2.499745),
    ("阶段", [733.426862, 24.610744], 2.500000),
    ("项目号", [770.800244, 24.791983], 2.499745),
    ("项目号", [770.794102, 24.793492], 2.500000),
];

/// The 36 cells `CPECCHB-SQ-A2.igr` letters.
///
/// The same furniture as [`CPECCHB_SQ_A1_LABELS`] at 1.414× the size — which
/// is A2's 420 mm against this template's own 593.939 mm extent, so its heights
/// are 3.535354 mm where the A1 file states 2.500000 — and with one more cell,
/// `修改号`, in the sign-off column.
///
/// Its company-name cell is `中国石油工程建设有限公司 ` with a trailing space,
/// which the A1 file's same cell does not carry; both are pinned as the files
/// spell them.
static CPECCHB_SQ_A2_LABELS: [PinnedLabel; 36] = [
    ("CADD号", [399.494979, 3.731490], 3.535354),
    ("CPECC", [398.236067, 91.605293], 1.000000),
    ("专业", [501.019500, 11.566782], 3.535354),
    (
        "中国石油工程建设有限公司 ",
        [479.407575, 100.801501],
        7.777778,
    ),
    ("仪  控", [357.411488, 76.789441], 3.535354),
    ("会 签 专 业", [33.939423, 218.484857], 4.242424),
    ("会 签 专 业", [33.939423, 324.545463], 4.242424),
    ("修改", [34.258931, 148.125721], 3.535354),
    ("修改号", [34.258931, 83.843272], 3.535354),
    ("制图", [345.654938, 68.304593], 3.535354),
    ("制图", [395.149887, 76.789441], 3.535354),
    ("华北分公司", [559.032975, 100.801501], 7.777778),
    ("审定", [345.968443, 35.353534], 3.535354),
    ("审定", [395.463393, 43.838382], 3.535354),
    ("审核", [345.681434, 43.937391], 3.535354),
    ("审核", [395.176384, 52.422240], 3.535354),
    ("审核", [34.258931, 169.376117], 3.535354),
    (
        "工程设计证书: A111017147 综甲     工程勘察证书: B111017147 综甲",
        [446.855582, 87.515155],
        2.828283,
    ),
    ("文件号", [501.626028, 20.364249], 3.535354),
    ("日    期", [33.939423, 289.191928], 4.242424),
    ("日    期", [33.939423, 395.252534], 4.242424),
    ("日期", [449.197725, 11.858930], 3.535354),
    ("日期", [34.258931, 127.140954], 3.535354),
    ("校对", [345.681434, 52.437223], 3.535354),
    ("校对", [395.176384, 60.922072], 3.535354),
    ("核准", [395.044438, 34.831005], 3.535354),
    ("次数", [34.258931, 66.311695], 3.535354),
    ("比例", [449.013020, 19.888312], 3.535354),
    ("版次", [561.828118, 11.817728], 3.535354),
    ("签     署", [33.939423, 253.838393], 4.242424),
    ("签     署", [33.939423, 359.898999], 4.242424),
    ("设计", [345.671705, 60.766438], 3.535354),
    ("设计", [395.166655, 69.251287], 3.535354),
    ("阶段", [448.886522, 27.732383], 3.535354),
    ("页次", [34.258931, 105.093668], 3.535354),
    ("项目号", [501.729083, 27.990814], 3.535354),
];

/// The 25 cells `A0-W - New.igr` letters.
///
/// A third furniture family, reachable only when the corpus is a backup
/// package: its title block letters `项目经理` and a wide-spaced `审  定`,
/// and its sign-off column letters a bare `签署` where the `CPE-SQ` family
/// letters `签     署`. Both W files letter the same 25 strings at their own
/// page size, and both place no symbol at all (`TEMPLATE_PLACEMENTS` below).
static A0_W_LABELS: [PinnedLabel; 25] = [
    ("专业", [20.213572, 674.384346], 3.000000),
    ("专业", [20.187827, 751.052526], 3.000000),
    ("修改", [1098.721629, 88.694982], 2.500000),
    ("审  定", [1041.215048, 44.659907], 2.500000),
    ("审  核", [1041.215048, 50.659907], 2.500000),
    ("审定", [1143.833155, 88.901014], 2.500000),
    ("审核", [1128.948888, 88.922241], 2.500000),
    ("文件号", [1076.215048, 20.659907], 2.500000),
    ("日  期", [1076.215048, 14.659907], 2.500000),
    ("日期", [1162.780233, 88.972189], 2.500000),
    ("日期", [20.110593, 724.638304], 3.000000),
    ("日期", [20.316552, 801.435210], 3.000000),
    ("校  对", [1041.215048, 56.659907], 2.500000),
    ("校对", [1113.380741, 88.808612], 2.500000),
    ("核  准", [1041.391838, 32.844114], 2.500000),
    ("比  例", [1143.967653, 14.731583], 2.500000),
    ("版  次", [1113.215048, 14.659907], 2.500000),
    ("版次", [1039.976561, 88.694982], 2.500000),
    ("签署", [20.290807, 700.798568], 3.000000),
    ("签署", [20.239317, 775.020987], 3.000000),
    ("设  计", [1041.215048, 62.659907], 2.500000),
    ("说明", [1067.899371, 88.808889], 2.500000),
    ("阶  段", [1076.215048, 26.659907], 2.500000),
    ("项目号", [1113.215048, 26.659907], 2.500000),
    ("项目经理", [1039.217319, 38.978732], 2.500000),
];

/// The same 25 cells as [`A0_W_LABELS`], on A1's page.
static A1_W_LABELS: [PinnedLabel; 25] = [
    ("专业", [20.213572, 427.384346], 3.000000),
    ("专业", [20.187827, 504.052526], 3.000000),
    ("修改", [750.153186, 89.177180], 3.000000),
    ("审  定", [691.958746, 44.984658], 3.000000),
    ("审  核", [691.850496, 50.984658], 3.000000),
    ("审定", [795.179527, 89.310615], 3.000000),
    ("审核", [780.407980, 89.384844], 3.000000),
    ("文件号", [726.917851, 21.143769], 3.000000),
    ("日  期", [726.697914, 15.143769], 3.000000),
    ("日期", [814.521916, 89.245664], 3.000000),
    ("日期", [20.110593, 477.638304], 3.000000),
    ("日期", [20.316552, 554.435210], 3.000000),
    ("校  对", [691.958746, 57.092909], 3.000000),
    ("校对", [765.315194, 89.292058], 3.000000),
    ("核  准", [691.735219, 32.963129], 3.000000),
    ("比  例", [794.978368, 15.193452], 3.000000),
    ("版  次", [764.049814, 15.231744], 3.000000),
    ("版次", [691.675367, 89.177180], 3.000000),
    ("签署", [20.290807, 453.798568], 3.000000),
    ("签署", [20.239317, 528.020987], 3.000000),
    ("设  计", [691.843854, 62.868810], 3.000000),
    ("说明", [719.314151, 89.210594], 3.000000),
    ("阶  段", [726.961839, 27.143769], 3.000000),
    ("项目号", [764.049814, 27.187757], 3.000000),
    ("项目经理", [691.628155, 39.059549], 3.000000),
];

/// What a sheet's template places, by the template's own file name: the file
/// name each placement records, sorted.
///
/// The `CPE-SQ` family keeps the `比例` value cell's 5 mm wave in one placement,
/// whose stored path is the authoring machine's own
/// `C:\Users\ADMINI~1.WIN\AppData\Local\Temp\BLFH.sym`; no share in this corpus
/// carries that file, so the body is the template's own cache. `CPECCHB-SQ-A1`
/// places nothing at all — its title block has no `比例` cell. `CPECCHB-SQ-A2`
/// places three: the wave under another authoring-machine temp name, and the two
/// sign-off cells' `HQZ.sym`.
///
/// Pinned per sheet rather than per package, so the numbers are the template's
/// and not the extraction's — a fuller backup moves nothing here, while a
/// template that starts placing something else fails.
///
/// Two of the seven only reach a page when the corpus can read the backup's own
/// `RefData~*.zip` (`A0-W - New.igr` on `A井场`, `A1-W - New.igr` on `稳定塔
/// (600td)`): same extent as their `CPE-SQ` siblings, so the pick's last key —
/// the file name on disk — decides, and the archive offers both. Neither title
/// block places a symbol; the two are pinned so the gate holds whichever corpus
/// it is run against — `PID_PARSE_SELF_CHECK_DIR` at a share (`Ref/Template
/// Files` first) or at a backup package, whose templates then come out of its
/// own `RefData~4~682.zip`. A backup package is not a share: it carries no
/// `fonts/`, so that run also has to name the share's faces in `PID_PLOT_FONTS`
/// — otherwise the gate refuses to pass for want of a face (see
/// [`require_faces`]) instead of reporting these two pins unread.
///
/// The second number is `BorderTemplate::extent_mm` — the template's own ink
/// box, the size its outer frame spans. The pick already refuses a template
/// more than 2 mm off the sheet (`TEMPLATE_EXTENT_TOLERANCE_MM` in the model),
/// so this pins the exact size rather than "close enough": a template that
/// loses a millimetre to a scale slip still gets picked, and only this says so.
/// It cannot see a *translation* — `extent_mm` is a width and a height, with no
/// origin — which is why the strokes are measured on the page as well, below.
///
/// The third number is `BorderTemplate::primitives` — every stroke the
/// template contributed, labels included. It is the same file whether the
/// corpus found it on disk or inside the backup's own archive, so the count is
/// the template's and not the extraction's; a title block that gains a cell
/// table line or loses a piece of its frame fails here even when it places no
/// symbol. (`CPECCHB-SQ-A1`'s 96 against `CPE-SQ-A1`'s 175 is the sign-off
/// column plus the `比例` cell it does not have.)
///
/// The extent is the template's, not the sheet's: `CPECCHB-SQ-A2` is authored
/// 593.939 mm wide on a 594.3 mm page, and `A1-W - New.igr` is 841 × 594 on the
/// `600td` sheet's 842.54 × 595.54 page — both inside the pick's tolerance, so
/// pinning the sheet here instead would be pinning the wrong number.
///
/// The fourth number is where each placement sits, in paper millimetres, paired
/// with the symbol's file name and sorted by it. It belongs to the template the
/// same way its extent does: the `.igr` states its placements, the sheet only
/// says which `.igr` it wears, so every one of the twenty-one A1 sheets that
/// wear `CPE-SQ-A1` carries its 比例 wave at the same point. A title block whose
/// cell moved — a re-authored template, a nibble read one field off — shows up
/// here and nowhere else, because the placement list alone still matches.
///
/// The fifth is the template's own lettering: every label it puts on the page,
/// as `(characters, insertion point, body height)`, sorted by text then
/// position, one `static` per template above. This is what the stroke count
/// above cannot say — a title block re-authored with the same number of strokes
/// in the same box, spelling the company name differently or sliding one cell
/// two millimetres, passes extent, strokes and placements alike. Twenty-five to
/// thirty-seven cells per template, 225 in all, and the count is pinned by each
/// list's own length. A template that gains or loses a cell therefore fails
/// here, whatever the drawing it lands on.
const TEMPLATE_PLACEMENTS: [PinnedTemplate; 7] = [
    (
        "CPE-SQ-A0.igr",
        [1189.0, 841.0],
        175,
        &[("blfh.sym", [1100.6925, 19.0])],
        &CPE_SQ_A0_LABELS,
    ),
    (
        "CPE-SQ-A1.igr",
        [841.0, 594.0],
        175,
        &[("blfh.sym", [752.6925, 19.0])],
        &CPE_SQ_A1_LABELS,
    ),
    (
        "CPE-SQ-A2.igr",
        [594.0, 420.0],
        175,
        &[("blfh.sym", [505.6925, 19.0])],
        &CPE_SQ_A2_LABELS,
    ),
    (
        "CPECCHB-SQ-A1.igr",
        [841.0, 594.0],
        96,
        &[],
        &CPECCHB_SQ_A1_LABELS,
    ),
    (
        "CPECCHB-SQ-A2.igr",
        [593.939, 420.0],
        116,
        &[
            ("a$c40d91a4b.sym", [0.0, 59.0404]),
            ("hqz.sym", [35.3536, 412.9293]),
            ("hqz.sym", [35.3536, 306.8687]),
        ],
        &CPECCHB_SQ_A2_LABELS,
    ),
    ("A0-W - New.igr", [1189.0, 841.0], 105, &[], &A0_W_LABELS),
    ("A1-W - New.igr", [841.0, 594.0], 101, &[], &A1_W_LABELS),
];

/// How far a pinned placement point may drift before the gate speaks, in
/// millimetres.
///
/// A ten-thousandth of a millimetre: a micrometre is two orders below what a
/// plotter resolves and orders below the float noise in a metre-to-millimetre
/// conversion, so it holds the point exactly while still tolerating the last
/// bits of `insertion.x * 1000.0`.
const PLACEMENT_SLACK_MM: f64 = 0.0001;

/// How far a pinned extent may drift before the gate speaks, in millimetres.
///
/// Two hundredths of a millimetre: a hundred times tighter than the pick's own
/// 2 mm tolerance, so a scale slip the pick waves through still fails here,
/// while the float noise in a template's authored size (`593.9393939…`) is far
/// inside it. It is deliberately below what a plotter resolves — this is a pin
/// on the file, not on the printed sheet.
const TEMPLATE_EXTENT_SLACK_MM: f64 = 0.02;

/// How far a pinned label's insertion point and body height may drift before
/// the gate speaks, in millimetres.
///
/// The same ten-thousandth as [`PLACEMENT_SLACK_MM`] and for the same reason:
/// a text record's point and size come out of the same metre-to-millimetre
/// conversion as a placement's, so this holds the authored value while
/// tolerating the last bits of the multiply.
///
/// It is deliberately tighter than the one place the corpus asks it to be:
/// `CPECCHB-SQ-A1` letters ten of its cells twice, the two copies 0.000255 mm
/// apart in body height (2.500000 against 2.499745). A slack wider than that
/// would let one copy stand in for the other; this one keeps them apart, and
/// their points — a few thousandths of a millimetre apart — apart as well.
const LABEL_SLACK_MM: f64 = 0.0001;

/// Where a page's furniture ink reaches, paper millimetres.
///
/// Not the same measure as `BorderTemplate::extent_mm`: that one is taken
/// from the `.igr` before it is drawn, so it is blind to where the template
/// was put. This one is the box the template's *strokes* cover once they are on
/// the page, which is what moves when a template is translated and what shrinks
/// when it is scaled. Labels are left out, as the template's own measure leaves
/// them out.
fn furniture_extent_mm(model: &PaperModel) -> Option<[[f64; 2]; 2]> {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    let mut seen = false;
    for primitive in model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
    {
        let mut take = |point: [f64; 2]| {
            seen = true;
            for axis in 0..2 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        };
        match &primitive.shape {
            PaperShape::Line { start_mm, end_mm } => {
                take(*start_mm);
                take(*end_mm);
            }
            PaperShape::Polyline { points_mm, .. } => {
                for point in points_mm {
                    take(*point);
                }
            }
            PaperShape::Fill { chain_mm, .. } => {
                for point in chain_mm {
                    take(*point);
                }
            }
            // A circle's box is its whole circle, as the model measures an
            // arc when it picks the template; a template's arcs are small
            // corner rounds, so this overstates by at most the round.
            PaperShape::Arc {
                center_mm,
                radius_mm,
                ..
            }
            | PaperShape::Circle {
                center_mm,
                radius_mm,
            } => {
                take([center_mm[0] - radius_mm, center_mm[1] - radius_mm]);
                take([center_mm[0] + radius_mm, center_mm[1] + radius_mm]);
            }
            PaperShape::Text { .. } => {}
        }
    }
    seen.then_some([min, max])
}

/// The file name a placement's stored path ends in, lowercased.
fn leaf_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase()
}

/// Whether a template's path names a `RefData~*.zip` member rather than a file
/// exploded on a share.
///
/// A member is labelled as the archive path with the member's own name on the
/// end (`…\RefData~4~682.zip\CPE-SQ-A1.igr`), so the `.zip` component is what
/// tells the two corpora apart.
fn from_archive(path: &str) -> bool {
    Path::new(path).components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".zip")
    })
}

/// One symbol a template places: the file name it stores, lowercased, and the
/// point it sits at on the page.
type Placement = (String, [f64; 2]);

/// What one template placed, over every sheet wearing it: the placements of the
/// first such sheet, and how many sheets those were.
type TemplateAccount = (Vec<Placement>, usize);

/// One label a template letters: the characters, the point it starts at on the
/// page, and its body height — `f64::NAN` where the record states none.
type Label = (String, [f64; 2], f64);

/// What one template lettered, over every sheet wearing it: the labels of the
/// first such sheet, and how many sheets those were.
type LabelAccount = (Vec<Label>, usize);

/// One template's placements as text, for a failure that names them.
fn format_placements(placements: &[Placement]) -> String {
    placements
        .iter()
        .map(|(name, point)| format!("{name}@({:.4}, {:.4})", point[0], point[1]))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether two placement lists agree: the same names in the same order, each at
/// the same point within [`PLACEMENT_SLACK_MM`].
///
/// The order is part of the answer. A template that places two `HQZ.sym`s in
/// its sign-off column states which cell comes first, and a decoder that begins
/// reading that column from the other side has changed the file the page came
/// from even though both points are still in the list.
fn same_placements(measured: &[Placement], pinned: &[Placement]) -> bool {
    measured.len() == pinned.len()
        && measured
            .iter()
            .zip(pinned)
            .all(|((name, point), (want_name, want_point))| {
                name == want_name
                    && (point[0] - want_point[0]).abs() < PLACEMENT_SLACK_MM
                    && (point[1] - want_point[1]).abs() < PLACEMENT_SLACK_MM
            })
}

/// Every label the template's furniture letters, in a deterministic order.
///
/// Sorted by text and then by position, so a template that letters one string
/// twice (`CPECCHB-SQ-A1`'s three `制图` cells) keeps a stable order and a
/// respelled cell shifts one entry rather than reshuffling the list.
///
/// Only furniture is read: a drawing's own labels are the drawing's business,
/// and the furniture flag is what tells the template's ink from the sheet's.
fn furniture_labels(model: &PaperModel) -> Vec<Label> {
    let mut out: Vec<Label> = model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Text {
                origin_mm,
                height_mm,
                value,
                ..
            } => Some((value.clone(), *origin_mm, height_mm.unwrap_or(f64::NAN))),
            _ => None,
        })
        .collect();
    out.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1[1].total_cmp(&right.1[1]))
            .then(left.1[0].total_cmp(&right.1[0]))
    });
    out
}

/// One label as text, for a failure that names it.
fn format_label(label: &Label) -> String {
    let (text, point, height) = label;
    format!(
        "{text:?} at ({:.4}, {:.4}) mm, {height:.4} mm high",
        point[0], point[1]
    )
}

/// Whether one label matches its pin: the same characters, the same insertion
/// point and the same body height, each within [`LABEL_SLACK_MM`].
///
/// An absent height matches only an absent one, so a record that gains or loses
/// its size is a change rather than a match.
fn same_label(measured: &Label, pinned: &Label) -> bool {
    let (text, point, height) = measured;
    let (want_text, want_point, want_height) = pinned;
    text == want_text
        && (point[0] - want_point[0]).abs() < LABEL_SLACK_MM
        && (point[1] - want_point[1]).abs() < LABEL_SLACK_MM
        && ((height - want_height).abs() < LABEL_SLACK_MM
            || (height.is_nan() && want_height.is_nan()))
}

/// Whether two label lists agree: the same characters in the same order, each
/// at the same insertion point and body height.
fn same_labels(measured: &[Label], pinned: &[Label]) -> bool {
    measured.len() == pinned.len()
        && measured
            .iter()
            .zip(pinned)
            .all(|(measured, pinned)| same_label(measured, pinned))
}

/// Where two label lists part company, one line per difference.
///
/// The lists are compared in order, so a difference is reported by position and
/// the thirty-odd cells that agree are left out: a template that respells one
/// cell should read as one line, not as two walls of text. A length that
/// differs gets its own line plus the cells one side has and the other does
/// not, since those are what a template that gained a cell looks like.
fn label_differences(measured: &[Label], pinned: &[Label]) -> String {
    let mut lines: Vec<String> = measured
        .iter()
        .zip(pinned)
        .enumerate()
        .filter(|(_, (measured, pinned))| !same_label(measured, pinned))
        .map(|(index, (measured, pinned))| {
            format!(
                "    [{index}] page {}, pinned {}",
                format_label(measured),
                format_label(pinned)
            )
        })
        .collect();
    if measured.len() != pinned.len() {
        lines.push(format!(
            "    {} labels on the page, {} pinned",
            measured.len(),
            pinned.len()
        ));
        lines.extend(
            measured
                .iter()
                .skip(pinned.len())
                .map(|extra| format!("    page only: {}", format_label(extra))),
        );
        lines.extend(
            pinned
                .iter()
                .skip(measured.len())
                .map(|extra| format!("    pin only: {}", format_label(extra))),
        );
    }
    lines.join("\n")
}

/// The corpus this run is reading, one line per source.
///
/// A passing gate has to say what it read. The archive-template run — the
/// corpus pointed at a backup package instead of the share — once took 85 s and
/// reported ok while touching no pin at all, and nothing in its output would
/// have said which corpus that `ok` was about.
fn describe_sources(corpus: &Path, backup: &Path, drawings: usize, dirs: &[PathBuf]) -> String {
    let origin = if std::env::var_os("PID_PARSE_SELF_CHECK_DIR").is_some() {
        "PID_PARSE_SELF_CHECK_DIR"
    } else {
        "built-in default"
    };
    let faces: Vec<String> = dirs
        .iter()
        .filter(|dir| dir.is_dir())
        .map(|dir| dir.display().to_string())
        .collect();
    format!(
        "corpus {} ({origin}); backup {} ({drawings} drawings); faces {}",
        corpus.display(),
        backup.display(),
        if faces.is_empty() {
            "none present".to_string()
        } else {
            faces.join(", ")
        }
    )
}

/// Refuse to pass for want of a face, naming what the page pins never saw.
///
/// A backup package keeps its templates in `RefData~*.zip` but not the
/// licensed faces, so the archive configuration has no `fonts/` under its
/// corpus. The layout pins all read without one — how many sheets drew
/// furniture, which template, its extent, its strokes, what it placed — but
/// every pin that needs a written PDF does not, and this used to be an
/// `eprintln!` and a `return`, which the harness reports as a pass. It did, for
/// one run.
fn require_faces(corpus: &Path, dirs: &[PathBuf], drawings: usize) {
    if dirs.iter().any(|dir| dir.is_dir()) {
        return;
    }
    let searched = dirs
        .iter()
        .map(|dir| dir.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    panic!(
        "no font directory is present, so the written-page pins were not read for {drawings} \
         drawings: each PDF's page, its weight, its ink and its lettering, and the \
         {DERIVED_PAGES}-sheet measured-page ratchet.\n  \
         searched: {searched}\n  \
         corpus: {} — a backup package carries no `fonts/`; set PID_PLOT_FONTS to the \
         share's fonts, or point PID_PARSE_SELF_CHECK_DIR at the share",
        corpus.display()
    );
}

#[test]
fn every_backup_drawing_exports_a_readable_page() {
    let corpus = corpus_dir();
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the backup export; no backup-plant-data directory is present");
        return;
    };
    let mut drawings = Vec::new();
    collect_pids(&backup, &mut drawings);
    if drawings.is_empty() {
        eprintln!(
            "skipping the backup export; {} carries no .pid",
            backup.display()
        );
        return;
    }
    assert!(
        drawings.len() >= BACKUP_SHEETS,
        "{} drawings found under {}, expected at least {BACKUP_SHEETS}",
        drawings.len(),
        backup.display()
    );
    let dirs = font_dirs(Some(&corpus));
    let sources = describe_sources(&corpus, &backup, drawings.len(), &dirs);
    eprintln!("{sources}");

    let mut models: Vec<(PathBuf, PaperModel)> = Vec::new();
    for path in &drawings {
        let model = lay_out_drawing(path, &corpus)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        models.push((path.clone(), model));
    }

    // The furniture's symbol account, over the whole package. A sheet's border
    // is a linked `.igr`, so the symbols its title block places are the
    // template's own and are not in the drawing's `missing_symbols` — that is
    // what keeps the source gaps (`BLFH.sym` and `HQZ.sym`, whose stored paths
    // are absolute paths on the authoring machine) out of the drawing's report
    // and on the page's own ledger, where `--residual` reads it.
    let mut templated = 0usize;
    let mut placements = 0usize;
    let mut gap_named = 0usize;
    let mut templated_from_archive = 0usize;
    // What each template placed, as the first sheet wearing it drew it, and how
    // many sheets wore it. Keyed by the template's own file name, because a
    // placement is the template's and not the sheet's.
    let mut by_template: BTreeMap<String, TemplateAccount> = BTreeMap::new();
    // Collected rather than asserted per sheet, and keyed by template: one run
    // should report every template that moved, once each, instead of stopping
    // at the first or repeating itself once per sheet wearing it.
    let mut moved: BTreeMap<String, String> = BTreeMap::new();
    // The same two accounts for what each template letters, keyed the same way.
    // A label is the template's text, so the per-sheet agreement and the
    // collect-then-report shape are the placement account's, not a new rule.
    let mut by_template_labels: BTreeMap<String, LabelAccount> = BTreeMap::new();
    let mut mislettered: BTreeMap<String, String> = BTreeMap::new();
    for (path, model) in &models {
        let name = path.display();
        assert!(
            !model
                .missing_symbols
                .iter()
                .any(|missing| missing.to_ascii_lowercase().ends_with("blfh.sym")),
            "{name}: the template's own gap entered the drawing's report: {:?}",
            model.missing_symbols
        );
        let Some(template) = model.template.as_ref() else {
            continue;
        };
        templated += 1;
        if from_archive(&template.path) {
            templated_from_archive += 1;
        }
        let Some((file, extent, primitives, wanted, wanted_labels)) = TEMPLATE_PLACEMENTS
            .iter()
            .find(|(file, ..)| template.path.ends_with(file))
        else {
            panic!(
                "{name}: {} drew furniture; no pin says what it places",
                template.path
            );
        };
        for axis in 0..2 {
            assert!(
                (template.extent_mm[axis] - extent[axis]).abs() < TEMPLATE_EXTENT_SLACK_MM,
                "{name}: {} measures {:?} mm, not the {extent:?} mm it is pinned at",
                template.path,
                template.extent_mm
            );
        }
        assert_eq!(
            template.primitives, *primitives,
            "{name}: {} contributed {} strokes",
            template.path, template.primitives
        );
        // The template states its own box; the page has to carry it from the
        // page's origin, because a template is authored in page millimetres and
        // laid out at 1:1. Measuring the strokes — not reusing `extent_mm` — is
        // what makes a translation visible: move the ink 2 mm and the box moves
        // with it while `extent_mm` does not budge.
        let Some([ink_min, ink_max]) = furniture_extent_mm(model) else {
            panic!(
                "{name}: {} is on the page but not one stroke of it landed",
                template.path
            );
        };
        for axis in 0..2 {
            assert!(
                ink_min[axis].abs() < TEMPLATE_EXTENT_SLACK_MM,
                "{name}: {}'s furniture starts at {ink_min:?} mm instead of the page origin",
                template.path
            );
            assert!(
                (ink_max[axis] - extent[axis]).abs() < TEMPLATE_EXTENT_SLACK_MM,
                "{name}: {}'s furniture reaches {ink_max:?} mm, not the {extent:?} mm box \
                 the template states",
                template.path
            );
        }
        // The template's own lettering, cell by cell. `template.primitives`
        // counts these strokes but says nothing about which characters they
        // carry or where the writer put them; this is the pin that does.
        let drawn_labels = furniture_labels(model);
        let pinned_labels: Vec<Label> = wanted_labels
            .iter()
            .map(|(text, point, height)| ((*text).to_string(), *point, *height))
            .collect();
        if same_labels(&drawn_labels, &pinned_labels) {
            // The same per-sheet agreement a placement gets, for the same
            // reason: the text belongs to the template, so a page that letters
            // it at another point is a difference only this can place.
            let seen = by_template_labels
                .entry((*file).to_string())
                .or_insert_with(|| (drawn_labels.clone(), 0));
            assert!(
                same_labels(&seen.0, &drawn_labels),
                "{name}: {} letters differ from the first sheet wearing it:\n{}",
                template.path,
                label_differences(&drawn_labels, &seen.0)
            );
            seen.1 += 1;
        } else {
            mislettered.entry((*file).to_string()).or_insert_with(|| {
                format!(
                    "  {file} (on {}):\n{}",
                    template.path,
                    label_differences(&drawn_labels, &pinned_labels)
                )
            });
        }
        let mut drawn: Vec<Placement> = Vec::new();
        for symbol in &template.symbols {
            placements += 1;
            assert_eq!(
                symbol.body,
                SymbolBodySource::Cached,
                "{name}: {} drew from the library; a template's own copy is the one",
                symbol.key
            );
            // The two halves of the account must agree: a name lookup that
            // found nothing and a body that resolved cannot both be true.
            assert_eq!(
                symbol.library_available,
                symbol.name_gap.is_none(),
                "{name}: the ledger disagrees with the lookup for {:?}",
                symbol
            );
            if symbol.name_gap.is_some() {
                gap_named += 1;
            }
            drawn.push((leaf_name(&symbol.key), symbol.insertion_mm));
        }
        // Stable, so the two `HQZ.sym` placements a sign-off column makes keep
        // the template's own order rather than an arbitrary one.
        drawn.sort_by(|a, b| a.0.cmp(&b.0));
        let pinned: Vec<Placement> = wanted
            .iter()
            .map(|(symbol, point)| ((*symbol).to_string(), *point))
            .collect();
        if !same_placements(&drawn, &pinned) {
            moved.entry((*file).to_string()).or_insert_with(|| {
                format!(
                    "  {file} (on {}): places {}, pinned {}",
                    template.path,
                    format_placements(&drawn),
                    format_placements(&pinned)
                )
            });
            continue;
        }
        // The pin is per template, so the sheets that wear one have to agree
        // with each other as well: a per-sheet offset would otherwise hide
        // inside "the template places these" until one sheet wore it alone.
        let seen = by_template
            .entry((*file).to_string())
            .or_insert_with(|| (drawn.clone(), 0));
        assert!(
            same_placements(&seen.0, &drawn),
            "{name}: {} places {} here and {} on the first sheet wearing it",
            template.path,
            format_placements(&drawn),
            format_placements(&seen.0)
        );
        seen.1 += 1;
    }
    assert!(
        moved.is_empty(),
        "{} template(s) place their symbols somewhere other than the pin says:\n{}",
        moved.len(),
        moved.values().cloned().collect::<Vec<_>>().join("\n")
    );
    assert!(
        mislettered.is_empty(),
        "{} template(s) letter something other than the pin says:\n{}",
        mislettered.len(),
        mislettered.values().cloned().collect::<Vec<_>>().join("\n")
    );
    for (file, (measured, sheets)) in &by_template {
        let cells = by_template_labels
            .get(file)
            .map_or(0, |(labels, _)| labels.len());
        eprintln!(
            "  {file}: {sheets} sheets, {cells} labels, {}",
            format_placements(measured)
        );
    }
    assert!(
        templated >= TEMPLATED_SHEETS,
        "{templated} sheets drew furniture; {TEMPLATED_SHEETS} did when this ratchet was set"
    );
    // No "one placement per sheet" equality: it held of the two templates the
    // corpus had and not of the six it has now — `CPECCHB-SQ-A1` places none
    // and `CPECCHB-SQ-A2` places three. What each template places is pinned
    // above, per sheet, which is where that fact belongs.
    let cells: usize = by_template_labels
        .values()
        .map(|(labels, _)| labels.len())
        .sum();
    eprintln!(
        "{templated} sheets drew furniture ({} disk, {} from RefData archives), \
         {placements} placements, {cells} labels, {gap_named} with a named gap",
        templated - templated_from_archive,
        templated_from_archive
    );
    // Last, because every pin above reads without a face and should speak
    // first: a template that moved is worth knowing about before a missing
    // face.
    require_faces(&corpus, &dirs, drawings.len());
    let refs: Vec<&PaperModel> = models.iter().map(|(_, model)| model).collect();
    let prepared = prepare_fonts(&refs, &dirs).unwrap_or_else(|error| panic!("fonts: {error}"));
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
        })
        .collect();

    let mut derived = 0usize;
    for (path, model) in &models {
        let name = path.display();
        assert_eq!(
            model.off_sheet_primitives, 0,
            "{name}: {} strokes were refused for not fitting the sheet",
            model.off_sheet_primitives
        );
        for primitive in &model.primitives {
            if let PaperShape::Text { value, .. } = &primitive.shape {
                assert!(
                    !value.contains(['\r', '\n', '\t']),
                    "{name}: a label reached the page with a line break in it: {value:?}"
                );
            }
            // A drawing's own ink never carries a width no pen carries. The one
            // that did is `Q井场`'s globe valve, placed under `BandAid` — the
            // 10 mm band SmartPlant paints over a flagged object — which the
            // model now treats as a mark rather than a pen. Furniture is out of
            // scope: a template's strokes are its own, drawn from its own body.
            if !primitive.furniture
                && let Some(stroke) = &primitive.stroke
            {
                assert!(
                    stroke.width_mm <= WIDEST_PEN_MM,
                    "{name}: a {:?} paints at {} mm, past the widest pen",
                    primitive.shape,
                    stroke.width_mm
                );
            }
        }
        for missing in &model.missing_symbols {
            assert!(
                KNOWN_LIBRARY_GAPS.contains(&missing.as_str()),
                "{name}: a symbol no library carries entered the set: {missing}"
            );
        }
        assert!(
            model.missing_boxes.is_empty(),
            "{name}: {} placement(s) had neither a library body nor a body of the \
             drawing's own and drew a placeholder: {:?}",
            model.missing_boxes.len(),
            model.missing_boxes
        );

        let pdf = write_plot(model, &fonts).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            pdf.len() < PAGE_CEILING,
            "{name}: the page weighs {} bytes; the faces are not subset",
            pdf.len()
        );
        let ink = read_plot(&pdf).unwrap_or_else(|error| panic!("{name}: read back: {error}"));
        // The page the PDF got is the page the model promised, for every
        // sheet — the ones that state a sheet and the ones measured for want
        // of one. This used to be skipped when `page_mm` was `None`, which is
        // exactly the case that needed watching: a measured page could drift
        // to any box the ink reached and no gate would say so.
        let page = model.resolved_page_mm().unwrap_or_else(|| {
            panic!("{name}: the model resolved no page; the writer must have errored")
        });
        assert!((ink.page_mm[0] - page[0]).abs() < 0.05, "{name} width");
        assert!((ink.page_mm[1] - page[1]).abs() < 0.05, "{name} height");
        if model.page_mm.is_none() {
            derived += 1;
            let standard = STANDARD_SHEETS_MM.iter().any(|sheet| {
                [*sheet, [sheet[1], sheet[0]]]
                    .iter()
                    .any(|paper| paper == &page)
            });
            assert!(
                standard,
                "{name}: a page the drawing never stated was measured as {page:?}, \
                 which is not a standard sheet"
            );
        }
        if model.primitives.is_empty() {
            // `D03` is an authored but empty sheet: a declared page with no
            // objects. It exports a blank page rather than failing.
            assert!(ink.segments.is_empty(), "{name}: unexpected ink");
            continue;
        }
        assert!(
            !ink.segments.is_empty(),
            "{name}: the page carries no ink at all"
        );
        assert!(
            !ink.spans.is_empty(),
            "{name}: the page carries no lettering at all"
        );
    }
    assert_eq!(
        derived, DERIVED_PAGES,
        "{derived} sheets stated no page and were measured; {DERIVED_PAGES} did when this ratchet was set"
    );
    eprintln!(
        "{} backup drawings exported, {} of them on a measured page, {} faces embedded",
        models.len(),
        derived,
        prepared.len()
    );
}
