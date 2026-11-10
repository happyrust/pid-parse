//! Typefaces named by a drawing's style table, loaded as font files from
//! caller-chosen directories and subset in Rust to the characters the
//! drawing actually letters.
//!
//! The reference plots use Arial, Arial Narrow, FangSong, SimSun and
//! NSimSun. Their files are document input, not a fixed system folder:
//! [`font_dirs`] lists `PID_PLOT_FONTS` first, then `<corpus>/fonts`, then
//! the repository's `test-file/fonts`. Subsetting is `fontcull` (klippa,
//! the fontations engine) — no Python, no fontTools — so SimSun's 18 MiB
//! program shrinks under pdfcraft's 16 MiB ceiling the same way the old
//! `pyftsubset` step made it fit, and a face file that is already small
//! enough embeds whole.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::model::{PaperModel, PaperShape, PaperTextSpan};

/// One face ready to embed. `key` is the name [`typeface_key`] returns.
pub struct PreparedFont {
    /// Canonical typeface name, such as `SimSun` or `Arial Narrow`.
    pub key: String,
    /// The program to embed: the file itself, or `fontcull`'s subset of it.
    pub bytes: Vec<u8>,
    /// Whether `bytes` is a subset rather than the whole file.
    ///
    /// A subset has to say so in the PDF's own names — ISO 32000-1 §9.6.4
    /// wants `ABCDEF+Name` on both `BaseFont` and `FontName` — while a whole
    /// file must not wear a tag it has not earned. The writer needs the fact,
    /// not a guess at it from the bytes.
    pub subset: bool,
}

/// Which face a run should use.
///
/// A name the style table states is used as stated. `ËÎÌå` is the vendor's
/// damaged spelling of `宋体`, documented on
/// [`pid_parse::style_link::TEXT_CHAR_FONT_NAME_OFFSET`]. Anything else
/// with a Han character uses FangSong; Latin-only lettering uses Arial
/// Narrow. Individual characters the chosen face cannot encode borrow one
/// of the faces of [`BORROW_ORDER`], one character at a time — see
/// [`crate::write`].
#[must_use]
pub fn typeface_key(requested: Option<&str>, text: &str) -> &'static str {
    if let Some(name) = requested.map(str::trim).filter(|name| !name.is_empty())
        && let Some(key) = known_typeface(name)
    {
        return key;
    }
    if text.chars().any(is_cjk) {
        "FangSong"
    } else {
        "Arial Narrow"
    }
}

/// The faces a label borrows from, in the order they are asked: one order
/// for every label.
///
/// A character the face a label names cannot draw — Han in an Arial Narrow
/// style, the `℃` after its digits — is lettered in the first of these whose
/// program draws it.
///
/// NSimSun is first because it is the face the reference plots borrow. All
/// sixteen borrowed runs on the eleven acceptance sheets — fourteen `℃` and
/// the A2 sheet's two `动火点` — are set in it, whatever the label holds:
/// 新宋体 is the face a Chinese Windows associates with a font of the ANSI
/// character set (`FontAssoc`'s `FontPackage`), and the plotter leaves the
/// characters a font lacks to that association.
///
/// SimSun follows for a machine that has no NSimSun file. The two are one
/// design — NSimSun is SimSun with a fixed-pitch Latin — so every character
/// the reference borrows is the same outline in either, and a borrowed run
/// stays in the reference's family. FangSong is third, for a machine with
/// neither.
///
/// The three after them are the other faces [`typeface_key`] knows, by name.
/// They answer for a character none of the first three draws: Latin beyond
/// what a Chinese face carries — Arial has `Ż`, SimSun does not — or a Han
/// character of extension B, which only SimSun-ExtB holds. The list is every
/// face this crate loads, so a character none of the six draws is drawn by
/// none, and [`prepare_fonts`] refuses it. The face a character is named in
/// stands in the list like any other and is passed over, having just been
/// found not to draw it.
///
/// Reading the lender off the programs in hand instead would make the answer
/// follow the batch. Those are subsets cut to the labels of whichever sheets
/// were prepared together, so the first program that holds `动火点` would be
/// one face for a sheet prepared alone and another beside a sheet whose own
/// label holds the same three characters. [`prepare_fonts`] gives each face
/// what this order sends it, judged against the face's whole program, and
/// the writer asks in the same order, so the two agree whatever else was
/// prepared with the sheet.
pub const BORROW_ORDER: [&str; 6] = [
    "NSimSun",
    "SimSun",
    "FangSong",
    "Arial",
    "Arial Narrow",
    "SimSun-ExtB",
];

/// The directories [`prepare_fonts`] looks in, in order.
///
/// `PID_PLOT_FONTS` is a `;`-separated list like `PATH`. `<corpus>/fonts`
/// follows, and the repository's `test-file/fonts` last — the corpus on
/// this machine keeps its licensed faces there.
#[must_use]
pub fn font_dirs(corpus: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(list) = std::env::var_os("PID_PLOT_FONTS") {
        dirs.extend(std::env::split_paths(&list));
    }
    if let Some(corpus) = corpus {
        dirs.push(corpus.join("fonts"));
    }
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-file/fonts"));
    dirs
}

/// Load the faces `models` letter in from `dirs`.
///
/// The face a label's own style names is required; a face file that no
/// directory carries is an error naming where the search looked. A face a
/// formatting **run** names is optional — the label draws in its own face
/// where the run's face is absent, the way it did before the partition was
/// read — since one missing typeface must not refuse a whole export.
/// A character the face it is named in cannot draw borrows one of the faces
/// of [`BORROW_ORDER`], and that lender is loaded, or its
/// program widened, to carry it — the run keeps its named face for
/// everything that face draws and only the rest borrows. Which lender
/// carries a character is settled against the lenders' whole programs, never
/// against the subsets this call builds, so a sheet's faces carry the same
/// characters whether it is prepared alone or with any other sheets.
///
/// # Errors
///
/// Returns an error when a needed face is absent from every directory, when
/// a file is present but not an embeddable standalone font, or when a label
/// holds a character that no face of [`BORROW_ORDER`] draws — the error
/// names the character and the label, and is the same for a sheet prepared
/// alone and in any batch.
pub fn prepare_fonts(
    models: &[&PaperModel],
    dirs: &[PathBuf],
) -> Result<Vec<PreparedFont>, String> {
    let mut wanted: BTreeMap<&'static str, String> = BTreeMap::new();
    for model in models {
        for primitive in &model.primitives {
            if !crate::write::is_plotted(primitive.confidence) {
                continue;
            }
            let PaperShape::Text { value, font, .. } = &primitive.shape else {
                continue;
            };
            if value.trim().is_empty() {
                continue;
            }
            wanted
                .entry(typeface_key(font.as_deref(), value))
                .or_default()
                .push_str(value);
        }
    }
    let mut faces: Vec<PreparedFont> = Vec::new();
    for (key, text) in &wanted {
        let (bytes, subset) = load_face(dirs, key, text)?;
        faces.push(PreparedFont {
            key: (*key).to_string(),
            bytes,
            subset,
        });
    }
    // A run inside a label may name a face the label's own style does not —
    // `Wastewater Pit.sym` letters one `*` in 宋体 inside an Arial line, and a
    // sheet label may mix them the same way. Those faces are **optional**: a
    // machine without the file draws that stretch in the label's own face,
    // which is what the page did before partitions were read, while
    // demanding it would refuse a whole export over one missing typeface. A
    // file that is present but unusable is skipped for the same reason.
    let mut run_faces: BTreeMap<&'static str, String> = BTreeMap::new();
    for model in models {
        for primitive in &model.primitives {
            if !crate::write::is_plotted(primitive.confidence) {
                continue;
            }
            let PaperShape::Text { value, spans, .. } = &primitive.shape else {
                continue;
            };
            if value.trim().is_empty() {
                continue;
            }
            for span in spans.iter() {
                let Some(named) = span.font.as_deref() else {
                    continue;
                };
                run_faces
                    .entry(typeface_key(Some(named), value))
                    .or_default()
                    .push_str(value);
            }
        }
    }
    for (key, text) in &run_faces {
        if faces.iter().any(|face| face.key == *key) {
            continue;
        }
        let Some(bytes) = face_file(dirs, key)? else {
            continue;
        };
        let Ok((bytes, subset)) = subset_or_whole(key, &bytes, text) else {
            continue;
        };
        faces.push(PreparedFont {
            key: (*key).to_string(),
            bytes,
            subset,
        });
    }
    // A character the face it is named in cannot draw is borrowed, and the
    // lender's program has to hold it. Which lender that is comes from the
    // label and from the lenders' whole programs ([`loans`]), so it is the
    // same whichever other sheets share this call.
    let mut programs: BTreeMap<&'static str, Option<Vec<u8>>> = BTreeMap::new();
    let lending = loans(models, &faces, dirs, &mut programs)?;
    for (lender, lent) in &lending.lent {
        let held = faces.iter().position(|face| face.key == *lender);
        if held.is_some_and(|index| lent.iter().all(|ch| draws(&faces[index].bytes, *ch))) {
            continue;
        }
        let Some(program) = whole_program(&mut programs, dirs, lender)? else {
            continue;
        };
        // The lender is often already on the page because the drawing names
        // it — FangSong is the default for Han lettering — and then its one
        // program has to carry the borrowed characters as well. Pushing a
        // second entry for the same face embeds one typeface twice, once per
        // subset: the I drawing's export carried two `/FangSong` resources
        // for one name and `pdffonts` listed both.
        let mut text = wanted
            .get(lender)
            .or_else(|| run_faces.get(lender))
            .cloned()
            .unwrap_or_default();
        text.extend(lent);
        let (bytes, subset) = subset_or_whole(lender, program, &text)?;
        match held {
            Some(index) => {
                faces[index].bytes = bytes;
                faces[index].subset = subset;
            }
            None => faces.push(PreparedFont {
                key: (*lender).to_string(),
                bytes,
                subset,
            }),
        }
    }
    // No face of the order draws these, on this machine's files, so no face
    // the writer asks could. Refused here, by the label, whatever else shares
    // the call: a face another sheet happened to load is in the same order
    // and was asked with the rest.
    if let Some(first) = lending.unplaced.first() {
        let characters = first
            .characters
            .iter()
            .map(|ch| format!("{ch:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        let others = match lending.unplaced.len() - 1 {
            0 => String::new(),
            more => format!("; {more} more label(s) hold a character no face draws"),
        };
        return Err(format!(
            "no face draws {characters} of the label {:?}, which names {} (asked {}; searched \
             {}){others}",
            first.label,
            first.named,
            BORROW_ORDER.join(", "),
            searched_dirs(dirs)
        ));
    }
    Ok(faces)
}

/// What the lenders have to carry for a set of drawings.
struct Loans {
    /// The characters each face is asked for beyond the labels that name it.
    lent: BTreeMap<&'static str, BTreeSet<char>>,
    /// The labels that hold a character no face of the order draws, in the
    /// order the drawings letter them.
    unplaced: Vec<Unplaced>,
}

/// A label one or more of whose characters no face draws.
struct Unplaced {
    /// The label's text.
    label: String,
    /// The face its style names.
    named: &'static str,
    /// The characters of it no face of [`BORROW_ORDER`] draws.
    characters: BTreeSet<char>,
}

/// Which face lends each character the face it is named in cannot draw.
///
/// Every character a label letters is named in a face ([`named_faces`]) —
/// the blanks inside the line with the rest, since each is set for its
/// advance, and not the trailing pad, which the writer drops. The ones
/// that face cannot draw go to the faces of [`BORROW_ORDER`], asked in
/// turn: a face is given every such character its program draws — none, for
/// a face this machine has no file of, which is how SimSun comes to lend
/// where NSimSun is absent —
/// and the next is asked only when that leaves some over. It is then given
/// every one *it* draws, not just the remainder, because the writer hands a
/// stretch its named face draws none of to one lender whole, and when an
/// earlier face cannot take all of it a later one has to be able to. A face
/// that draws nothing an earlier one does not already draw all of is given
/// nothing: the writer, asking in the same order, never gets as far as it.
/// The asking stops at the first face that draws every one of them.
///
/// A lender is judged by its whole program (`programs`), never by a subset:
/// a subset holds a character only where some label prepared in the same call
/// needed it, so it would answer for the batch instead of for the face. The
/// writer then finds, in each face up to the one that ends the asking,
/// exactly the label's characters that face's program draws — so the first
/// face it finds able to draw a character, or a whole stretch, is the same
/// one whatever else was prepared with the sheet.
fn loans(
    models: &[&PaperModel],
    faces: &[PreparedFont],
    dirs: &[PathBuf],
    programs: &mut BTreeMap<&'static str, Option<Vec<u8>>>,
) -> Result<Loans, String> {
    let mut loans = Loans {
        lent: BTreeMap::new(),
        unplaced: Vec::new(),
    };
    for model in models {
        for primitive in &model.primitives {
            if !crate::write::is_plotted(primitive.confidence) {
                continue;
            }
            let PaperShape::Text {
                value, font, spans, ..
            } = &primitive.shape
            else {
                continue;
            };
            if value.trim().is_empty() {
                continue;
            }
            let own = typeface_key(font.as_deref(), value);
            // Only what the page letters. The writer drops a trailing pad
            // before it sets a label, so a character there is never drawn
            // and asks nothing of a lender; every character before it is
            // set, a blank included — it inks nothing and still takes its
            // advance, so a face has to carry it. `动　火　点` spaces its
            // characters with U+3000, which Arial Narrow does not draw.
            let inked = value.trim_end().chars().count();
            let mut lacking: BTreeSet<char> = BTreeSet::new();
            for (ch, named) in value
                .chars()
                .zip(named_faces(value, own, spans))
                .take(inked)
            {
                // A run's face this machine has no file for is not loaded,
                // and the label letters that stretch in its own face.
                let (key, face) = match loaded(faces, named) {
                    Some(face) => (named, face),
                    None => match loaded(faces, own) {
                        Some(face) => (own, face),
                        None => continue,
                    },
                };
                if draws(&face.bytes, ch) {
                    continue;
                }
                // A face is subset to the labels that name it as their own,
                // so a run that names it may hold a character its program
                // draws and its subset lacks. The face then needs the
                // character itself; nothing is borrowed.
                if key != own
                    && whole_program(programs, dirs, key)?.is_some_and(|program| draws(program, ch))
                {
                    loans.lent.entry(key).or_default().insert(ch);
                    continue;
                }
                lacking.insert(ch);
            }
            if lacking.is_empty() {
                continue;
            }
            // What each face asked so far draws of `lacking`, in the order
            // asked.
            let mut drawn: Vec<BTreeSet<char>> = Vec::new();
            for lender in BORROW_ORDER {
                let from_lender = drawn_by(programs, dirs, lender, &lacking)?;
                if from_lender.is_empty()
                    || drawn.iter().any(|earlier| from_lender.is_subset(earlier))
                {
                    continue;
                }
                loans.lent.entry(lender).or_default().extend(&from_lender);
                let all = from_lender.len() == lacking.len();
                drawn.push(from_lender);
                if all {
                    break;
                }
            }
            let characters: BTreeSet<char> = lacking
                .iter()
                .copied()
                .filter(|ch| !drawn.iter().any(|from_lender| from_lender.contains(ch)))
                .collect();
            if !characters.is_empty() {
                loans.unplaced.push(Unplaced {
                    label: value.clone(),
                    named: own,
                    characters,
                });
            }
        }
    }
    Ok(loans)
}

/// The face each character of a label is named in: its run's, where a run
/// names one, and the label's own otherwise.
///
/// A later run overrides an earlier one over the characters they share, and
/// a run that names no face puts its characters back in the label's own —
/// the same reading the writer cuts the label by.
fn named_faces(value: &str, own: &'static str, spans: &[PaperTextSpan]) -> Vec<&'static str> {
    let mut named = vec![own; value.chars().count()];
    for span in spans {
        let face = span
            .font
            .as_deref()
            .map_or(own, |font| typeface_key(Some(font), value));
        let start = usize::from(span.start);
        let stop = start.saturating_add(usize::from(span.len));
        for slot in named.iter_mut().take(stop).skip(start) {
            *slot = face;
        }
    }
    named
}

/// The loaded face `key` names, if one is.
fn loaded<'f>(faces: &'f [PreparedFont], key: &str) -> Option<&'f PreparedFont> {
    faces.iter().find(|face| face.key == key)
}

/// The whole program of face `key`, read from `dirs` once and kept; `None`
/// when this machine has no file for it.
fn whole_program<'p>(
    programs: &'p mut BTreeMap<&'static str, Option<Vec<u8>>>,
    dirs: &[PathBuf],
    key: &'static str,
) -> Result<Option<&'p [u8]>, String> {
    if !programs.contains_key(key) {
        let program = face_file(dirs, key)?;
        programs.insert(key, program);
    }
    Ok(programs.get(key).and_then(Option::as_deref))
}

/// The characters of `wanted` the whole program of face `key` draws.
fn drawn_by(
    programs: &mut BTreeMap<&'static str, Option<Vec<u8>>>,
    dirs: &[PathBuf],
    key: &'static str,
    wanted: &BTreeSet<char>,
) -> Result<BTreeSet<char>, String> {
    let Some(program) = whole_program(programs, dirs, key)? else {
        return Ok(BTreeSet::new());
    };
    Ok(wanted
        .iter()
        .copied()
        .filter(|ch| draws(program, *ch))
        .collect())
}

/// Whether a font program draws `ch`.
fn draws(program: &[u8], ch: char) -> bool {
    let mut buf = [0u8; 4];
    pdfcraft_fonts::embedding::covers(program, ch.encode_utf8(&mut buf))
}

/// The face `key`'s file, subset to the run's characters when the source
/// program is worth shrinking, and whether what came back is a subset.
fn load_face(dirs: &[PathBuf], key: &str, text: &str) -> Result<(Vec<u8>, bool), String> {
    let bytes = face_file(dirs, key)?.ok_or_else(|| missing_face(dirs, key))?;
    subset_or_whole(key, &bytes, text)
}

/// `fontcull`'s subset of `bytes` down to `text` plus the printable ASCII
/// baseline; the whole file where subsetting fails or the program already
/// fits — a file that fits is validated the same way a subset is, so a
/// bad file never loads silently.
///
/// The second value says which of the two happened, because the PDF's own
/// names have to agree with it: a subset's `BaseFont` and `FontName` carry an
/// `ABCDEF+` tag (ISO 32000-1 §9.6.4) and a whole file's must not.
fn subset_or_whole(key: &str, bytes: &[u8], text: &str) -> Result<(Vec<u8>, bool), String> {
    let chars: HashSet<char> = ('\u{20}'..='\u{7e}').chain(text.chars()).collect();
    if let Ok(subset) = subset_face(bytes, &chars)
        && pdfcraft_fonts::embedding::validate(&subset).is_ok()
    {
        return Ok((subset, true));
    }
    pdfcraft_fonts::embedding::validate(bytes).map_err(|error| format!("{key}: {error}"))?;
    Ok((bytes.to_vec(), false))
}

/// `fontcull`'s own plan with one fix: the name table stays whole — the
/// public [`fontcull::subset_font_data`] plans an empty name-ID set,
/// which drops `name` entirely and leaves the subset unnameable, and a
/// face pdfcraft cannot name cannot embed.
fn subset_face(font_data: &[u8], chars: &HashSet<char>) -> Result<Vec<u8>, String> {
    use fontcull_klippa::{Plan, SubsetFlags, subset_font};
    use fontcull_read_fonts::collections::IntSet;
    use fontcull_skrifa::{FontRef, GlyphId, Tag};
    use fontcull_write_fonts::types::NameId;

    let font = FontRef::new(font_data).map_err(|error| format!("parse: {error:?}"))?;
    let mut unicodes: IntSet<u32> = IntSet::empty();
    for ch in chars {
        unicodes.insert(u32::from(*ch));
    }
    let empty_gids: IntSet<GlyphId> = IntSet::empty();
    // `klippa` passes the monochrome bitmap tables through whole: SimSun's
    // `EBDT` + `EBLC` are 6.2 MiB of the face, so a three-character subset
    // still weighed 6.4 MiB and every plotted page was ~6.8 MiB. A PDF draws
    // the outline, never the bitmap strike, so the tables go.
    let bitmap_tables: IntSet<Tag> = [Tag::new(b"EBDT"), Tag::new(b"EBLC"), Tag::new(b"EBSC")]
        .into_iter()
        .collect();
    let name_ids: IntSet<NameId> = IntSet::all();
    let langs: IntSet<u16> = IntSet::all();
    let layout_scripts: IntSet<Tag> = IntSet::all();
    let layout_features: IntSet<Tag> =
        IntSet::from_iter(fontcull_klippa::DEFAULT_LAYOUT_FEATURES.iter().copied());
    let plan = Plan::new(
        &empty_gids,
        &unicodes,
        &font,
        SubsetFlags::default(),
        &bitmap_tables,
        &layout_scripts,
        &layout_features,
        &name_ids,
        &langs,
    );
    subset_font(&font, &plan).map_err(|error| format!("subset: {error:?}"))
}

/// The file a face lives in, first hit across `dirs`.
fn face_file(dirs: &[PathBuf], key: &str) -> Result<Option<Vec<u8>>, String> {
    for dir in dirs {
        for name in face_files(key) {
            let path = dir.join(name);
            if path.is_file() {
                return std::fs::read(&path)
                    .map(Some)
                    .map_err(|error| format!("read {}: {error}", path.display()));
            }
        }
    }
    Ok(None)
}

/// Filenames a face answers to, most common first.
fn face_files(key: &str) -> Vec<String> {
    let mut names: Vec<String> = match key {
        "Arial" => vec!["arial.ttf".into()],
        "Arial Narrow" => vec!["ARIALN.TTF".into(), "arialn.ttf".into()],
        "FangSong" => vec!["simfang.ttf".into(), "fangsong.ttf".into()],
        "SimSun" => vec!["simsun.ttf".into()],
        "NSimSun" => vec!["nsimsun.ttf".into()],
        "SimSun-ExtB" => vec!["simsunb.ttf".into()],
        _ => Vec::new(),
    };
    names.push(format!("{key}.ttf"));
    names.push(format!("{key}.otf"));
    names.push(format!("{}.ttf", key.replace(' ', "")));
    names
}

fn missing_face(dirs: &[PathBuf], key: &str) -> String {
    format!(
        "no font file for {key} (searched {}; set PID_PLOT_FONTS or drop {key}.ttf into one)",
        searched_dirs(dirs)
    )
}

fn searched_dirs(dirs: &[PathBuf]) -> String {
    if dirs.is_empty() {
        return "no font directories".to_string();
    }
    dirs.iter()
        .map(|dir| dir.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn known_typeface(name: &str) -> Option<&'static str> {
    match name {
        "宋体" | "ËÎÌå" => return Some("SimSun"),
        "新宋体" => return Some("NSimSun"),
        "仿宋" | "仿宋_GB2312" => return Some("FangSong"),
        _ => {}
    }
    match name.to_ascii_lowercase().as_str() {
        "arial" => Some("Arial"),
        "arial narrow" => Some("Arial Narrow"),
        "simsun" => Some("SimSun"),
        "nsimsun" => Some("NSimSun"),
        "fangsong" => Some("FangSong"),
        "simsun-extb" => Some("SimSun-ExtB"),
        _ => None,
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{3400}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'
    )
}

/// Faces and font directories for tests that have to run where no licensed
/// font file is: Inter, which pdfcraft carries, under the file names the
/// loader looks for.
#[cfg(test)]
pub(crate) mod test_faces {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A scratch font directory, removed when the test that made it lets go
    /// of it. It stands where [`super::prepare_fonts`] takes its search
    /// path: `&dir` is that one directory.
    pub(crate) struct ScratchDir([PathBuf; 1]);

    impl std::ops::Deref for ScratchDir {
        type Target = [PathBuf];

        fn deref(&self) -> &[PathBuf] {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            // Nothing rests on the removal: a directory a locked file keeps
            // behind is litter in the temporary directory, not a failed test.
            let _ = std::fs::remove_dir_all(&self.0[0]);
        }
    }

    /// A scratch directory holding each face under the name the loader looks
    /// for, so a test needs no licensed fonts on the machine.
    ///
    /// The directory is the caller's alone. Its name carries the process id
    /// and a count of the directories this process has made, so neither
    /// another run of the tests sharing the temporary directory nor another
    /// test of this run reads or replaces its faces; `name` only says whose
    /// it is.
    pub(crate) fn scratch_dir(name: &str, faces: &[(&str, &[u8])]) -> ScratchDir {
        static MADE: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "pid-plot-fonts-{name}-{}-{}",
            std::process::id(),
            MADE.fetch_add(1, Ordering::Relaxed)
        ));
        // A process id comes round again, and a run that was killed left its
        // directories behind.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        for (file, bytes) in faces {
            std::fs::write(dir.join(file), bytes).expect("scratch face");
        }
        ScratchDir([dir])
    }

    /// Inter cut down to printable ASCII: a face whose file is present and
    /// which draws nothing a label would have to borrow — what Arial Narrow
    /// is to a Han label.
    pub(crate) fn ascii_only() -> Vec<u8> {
        let (bytes, subset) = super::subset_or_whole("Inter", pdfcraft_fonts::INTER_REGULAR, "")
            .expect("Inter subsets");
        assert!(
            subset,
            "Inter came back whole, so it still draws everything"
        );
        bytes
    }

    /// Inter with every character of `extra` it has no glyph for mapped onto
    /// the glyph of `A`: a face that draws those characters as far as a
    /// character map can tell, which is all the loader and the writer ask of
    /// one. It stands in for SimSun and FangSong, whose files cannot be
    /// committed.
    pub(crate) fn drawing(extra: &str) -> Vec<u8> {
        use fontcull_skrifa::{FontRef, MetadataProvider};
        use fontcull_write_fonts::FontBuilder;
        use fontcull_write_fonts::tables::cmap::Cmap;

        let font = FontRef::new(pdfcraft_fonts::INTER_REGULAR).expect("Inter parses");
        let charmap = font.charmap();
        let stand_in = charmap.map('A').expect("Inter draws A");
        let lacking = extra.chars().filter(|ch| charmap.map(*ch).is_none());
        let mappings = charmap
            .mappings()
            .filter_map(|(code, glyph)| char::from_u32(code).map(|ch| (ch, glyph)))
            .chain(lacking.map(|ch| (ch, stand_in)));
        let cmap = Cmap::from_mappings(mappings).expect("no character is mapped twice");
        FontBuilder::new()
            .add_table(&cmap)
            .expect("the character map serializes")
            .copy_missing_tables(font)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::test_faces::{ascii_only, drawing, scratch_dir};
    use super::{BORROW_ORDER, font_dirs, prepare_fonts, subset_or_whole, typeface_key};
    use crate::model::{
        PaperModel, PaperPrimitive, PaperShape, PaperTextSpan, TextAnchor, TextLine,
    };

    /// SimSun ships a 5.5 MiB `EBDT` bitmap strike and an 11.7 MiB `glyf`
    /// table. `fontcull` subsets the outlines but passes the bitmap tables
    /// through whole, so a three-character subset used to weigh 6.36 MiB and
    /// every plotted page ~6.8 MiB — thirty times the reference plot. A PDF
    /// strokes outlines, never bitmap strikes, so those tables are dropped.
    #[test]
    fn a_cjk_face_is_subset_to_the_drawing_not_embedded_whole() {
        let corpus = std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
            || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
            PathBuf::from,
        );
        let Some(dir) = font_dirs(Some(&corpus))
            .into_iter()
            .find(|dir| dir.join("simsun.ttf").is_file())
        else {
            eprintln!("skipping the subset ceiling; no simsun.ttf in the font directories");
            return;
        };
        let bytes = std::fs::read(dir.join("simsun.ttf")).expect("read simsun.ttf");
        let text = "PT 1001101A 阀泵换热器 低温分离";
        let (subset, subsetted) = subset_or_whole("SimSun", &bytes, text).expect("subset SimSun");
        assert!(subsetted, "SimSun is too large to embed whole");
        assert!(
            pdfcraft_fonts::embedding::validate(&subset).is_ok(),
            "the subset has to stay embeddable"
        );
        for ch in text.chars() {
            let mut buf = [0u8; 4];
            assert!(
                pdfcraft_fonts::embedding::covers(&subset, ch.encode_utf8(&mut buf)),
                "the subset must still draw {ch:?}"
            );
        }
        assert!(
            subset.len() < 2 * 1024 * 1024,
            "subset is {} bytes of {} — the bitmap tables came back",
            subset.len(),
            bytes.len()
        );
    }

    /// A label whose run names its own face needs nothing extra; one whose run
    /// names another face asks for it, and gets it when the file is there.
    fn labelled(spans: Vec<PaperTextSpan>) -> PaperModel {
        PaperModel {
            library: Vec::new(),
            template: None,
            page_mm: Some([200.0, 100.0]),
            off_sheet_primitives: 0,
            skipped_endpoint_pairs: 0,
            primitives: vec![PaperPrimitive {
                confidence: "decoded",
                shape: PaperShape::Text {
                    origin_mm: [20.0, 40.0],
                    height_mm: Some(2.5),
                    rotation: 0.0,
                    value: "AB".into(),
                    font: Some("Arial".into()),
                    anchor: TextAnchor::Left,
                    line: TextLine::Bottom,
                    height_basis: crate::model::TextHeightBasis::Cap,
                    width_permille: None,
                    paragraph: None,
                    spans,
                },
                stream_path: None,
                byte_range: None,
                symbol: None,
                stroke: None,
                furniture: false,
            }],
            missing_symbols: Vec::new(),
            missing_templates: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
            ledger: crate::ledger::LayoutLedger::default(),
        }
    }

    /// A sheet of plain labels, each `(value, the font its style names)`.
    fn sheet(labels: &[(&str, &str)]) -> PaperModel {
        let mut model = labelled(Vec::new());
        let template = model.primitives.remove(0);
        for (value, named) in labels {
            let mut primitive = template.clone();
            if let PaperShape::Text {
                value: text, font, ..
            } = &mut primitive.shape
            {
                *text = (*value).to_string();
                *font = Some((*named).to_string());
            }
            model.primitives.push(primitive);
        }
        model
    }

    /// Whether the program `faces` loaded under `key` draws `ch`.
    fn carries(faces: &[super::PreparedFont], key: &str, ch: char) -> bool {
        let mut buf = [0u8; 4];
        faces.iter().any(|face| {
            face.key == key
                && pdfcraft_fonts::embedding::covers(&face.bytes, ch.encode_utf8(&mut buf))
        })
    }

    /// The A2 acceptance sheet letters two `动火点` labels in a style that
    /// names Arial Narrow, which draws no Han, so the three characters are
    /// borrowed, as the `℃` of `120℃` is. The program that carries them must
    /// not depend on the other sheets prepared in the same call: read off the
    /// subsets in hand it would be one face for the sheet alone and another
    /// beside a sheet whose own 宋体 or 仿宋 label already holds the
    /// characters. They go to NSimSun, the first face of the order, whose
    /// program carries what it lends whatever else the batch letters.
    #[test]
    fn a_borrowed_character_is_given_to_the_same_face_alone_and_in_a_batch() {
        let narrow = ascii_only();
        let song = drawing("动火点℃");
        let dir = scratch_dir(
            "borrow-alone-and-batch",
            &[
                ("ARIALN.TTF", &narrow),
                ("nsimsun.ttf", &song),
                ("simfang.ttf", &song),
                ("simsun.ttf", &song),
            ],
        );
        let borrowing = sheet(&[("动火点", "Arial Narrow"), ("120℃", "Arial Narrow")]);
        let naming = sheet(&[("动火点℃", "宋体"), ("点℃", "仿宋")]);
        let alone = prepare_fonts(&[&borrowing], &dir).expect("alone");
        let batch = prepare_fonts(&[&borrowing, &naming], &dir).expect("in a batch");
        for (prepared, faces) in [("alone", &alone), ("in a batch", &batch)] {
            for ch in "动火点℃".chars() {
                assert!(
                    carries(faces, "NSimSun", ch),
                    "prepared {prepared}: NSimSun, the face every label borrows from first, \
                     does not carry {ch:?}"
                );
            }
        }
        let keys: Vec<&str> = alone.iter().map(|face| face.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["Arial Narrow", "NSimSun"],
            "prepared alone, a face that lends nothing was loaded"
        );
    }

    /// NSimSun is a file a machine may not have. The characters then go to
    /// the next face of the order, SimSun — the same design, so the page
    /// stays in the family the reference borrows — for a label with a Han
    /// character and for one without alike, and FangSong is not asked.
    #[test]
    fn a_machine_without_nsimsun_borrows_from_simsun() {
        let narrow = ascii_only();
        let song = drawing("动火点℃");
        let dir = scratch_dir(
            "borrow-without-nsimsun",
            &[
                ("ARIALN.TTF", &narrow),
                ("simfang.ttf", &song),
                ("simsun.ttf", &song),
            ],
        );
        let borrowing = sheet(&[("动火点", "Arial Narrow"), ("120℃", "Arial Narrow")]);
        let naming = sheet(&[("动火点℃", "仿宋")]);
        let alone = prepare_fonts(&[&borrowing], &dir).expect("alone");
        let batch = prepare_fonts(&[&borrowing, &naming], &dir).expect("in a batch");
        for (prepared, faces) in [("alone", &alone), ("in a batch", &batch)] {
            for ch in "动火点℃".chars() {
                assert!(
                    carries(faces, "SimSun", ch),
                    "prepared {prepared}: SimSun, the face after an absent NSimSun, does not \
                     carry {ch:?}"
                );
            }
        }
        let keys: Vec<&str> = alone.iter().map(|face| face.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["Arial Narrow", "SimSun"],
            "prepared alone, FangSong was loaded though SimSun lends every character"
        );
    }

    /// `Wastewater Pit.sym` letters one `*` in 宋体 inside an Arial label: the
    /// span names a face the label's own style does not, so the writer has to
    /// load it — otherwise the stretch is drawn in Arial and the face change
    /// silently vanishes.
    #[test]
    fn a_span_naming_another_face_loads_it() {
        let dir = scratch_dir(
            "span-face",
            &[
                ("arial.ttf", pdfcraft_fonts::INTER_REGULAR),
                ("simsun.ttf", pdfcraft_fonts::INTER_REGULAR),
            ],
        );
        let model = labelled(vec![PaperTextSpan {
            start: 0,
            len: 1,
            height_mm: Some(2.5),
            font: Some("宋体".into()),
        }]);
        let faces = prepare_fonts(&[&model], &dir).expect("both faces load");
        let keys: Vec<&str> = faces.iter().map(|face| face.key.as_str()).collect();
        assert_eq!(keys, vec!["Arial", "SimSun"], "the run's face came along");
    }

    /// The same label on a machine without that file still plots: the run
    /// falls back to the label's face instead of refusing the export.
    #[test]
    fn a_span_face_this_machine_lacks_degrades_instead_of_refusing() {
        let dir = scratch_dir(
            "span-face-absent",
            &[("arial.ttf", pdfcraft_fonts::INTER_REGULAR)],
        );
        let model = labelled(vec![PaperTextSpan {
            start: 0,
            len: 1,
            height_mm: Some(2.5),
            font: Some("宋体".into()),
        }]);
        let faces = prepare_fonts(&[&model], &dir).expect("the label's own face is enough");
        let keys: Vec<&str> = faces.iter().map(|face| face.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["Arial"],
            "the missing run face is skipped, not fatal"
        );
    }

    /// A `ProbeOnly` record draws nothing, so it must not ask for a face
    /// either. `0x0030` `JStyleOverride` lettering is exactly that: its bytes
    /// are sound and no reading of them is, so the page never letters it — yet
    /// its characters used to pull a face out of the font directories, and a
    /// page whose only Han text came from one of those embedded a typeface it
    /// never drew.
    #[test]
    fn probe_only_lettering_asks_for_no_face() {
        let dir = scratch_dir("probe-only", &[]);
        let mut model = labelled(Vec::new());
        model.primitives[0].confidence = "probe_only";
        let faces = prepare_fonts(&[&model], &dir).expect("nothing was asked for");
        assert!(
            faces.is_empty(),
            "a record the page never draws asked for {faces:?}",
            faces = faces
                .iter()
                .map(|face| face.key.as_str())
                .collect::<Vec<_>>()
        );
    }

    /// A label of spaces draws no glyph (`show_text` trims before writing), so
    /// it must not ask for a face: a page whose only lettering is blank would
    /// otherwise embed a typeface it never inks, or refuse to export on a
    /// machine without one.
    #[test]
    fn a_label_of_spaces_asks_for_no_face() {
        let dir = scratch_dir("spaces-only", &[]);
        let mut model = labelled(vec![PaperTextSpan {
            start: 0,
            len: 1,
            height_mm: Some(2.5),
            font: Some("宋体".into()),
        }]);
        if let PaperShape::Text { value, .. } = &mut model.primitives[0].shape {
            *value = "   ".into();
        }
        let faces = prepare_fonts(&[&model], &dir).expect("nothing was asked for");
        assert!(
            faces.is_empty(),
            "a blank label asked for {faces:?}",
            faces = faces
                .iter()
                .map(|face| face.key.as_str())
                .collect::<Vec<_>>()
        );
    }

    /// A run may name a face that another label on the sheet names as its
    /// own. That face is subset to the labels that own it, so the run's
    /// character has to be added to its program: `Ω` here is drawn by Arial,
    /// which the second label's run names, and no Arial label holds it. Left
    /// out, the character would be looked for among the lenders — and with
    /// neither on this machine the export would be refused, although the same
    /// label exports when the Arial label is not on the sheet.
    #[test]
    fn a_run_naming_a_face_another_label_owns_gets_its_character_into_that_face() {
        let narrow = ascii_only();
        let dir = scratch_dir(
            "run-names-an-owned-face",
            &[
                ("ARIALN.TTF", &narrow),
                ("arial.ttf", pdfcraft_fonts::INTER_REGULAR),
            ],
        );
        let mut model = sheet(&[("AB", "Arial"), ("XΩ", "Arial Narrow")]);
        if let PaperShape::Text { spans, .. } = &mut model.primitives[1].shape {
            spans.push(PaperTextSpan {
                start: 1,
                len: 1,
                height_mm: Some(2.5),
                font: Some("Arial".into()),
            });
        }
        let faces = prepare_fonts(&[&model], &dir).expect("the run's own face draws Ω");
        let keys: Vec<&str> = faces.iter().map(|face| face.key.as_str()).collect();
        assert_eq!(keys, vec!["Arial", "Arial Narrow"], "nothing is borrowed");
        assert!(
            carries(&faces, "Arial", 'Ω'),
            "Arial, the face the run names, was not given the run's character"
        );
    }

    /// A blank the named face does not draw is borrowed like any other
    /// character: the writer sets it for its advance, so some program has to
    /// hold it. `动　火　点` spaces its characters with U+3000, which Arial
    /// Narrow lacks, and SimSun — the lender of a label with a Han character
    /// — carries the space with the three characters, for the sheet alone as
    /// beside a sheet whose own 宋体 label holds it.
    #[test]
    fn a_space_the_named_face_cannot_draw_is_given_to_the_lender() {
        let narrow = ascii_only();
        let song = drawing("动火点审定\u{3000}");
        let dir = scratch_dir(
            "borrowed-space",
            &[
                ("ARIALN.TTF", &narrow),
                ("simfang.ttf", &song),
                ("simsun.ttf", &song),
            ],
        );
        let borrowing = sheet(&[("动\u{3000}火\u{3000}点", "Arial Narrow")]);
        let naming = sheet(&[("审\u{3000}定", "宋体")]);
        let alone = prepare_fonts(&[&borrowing], &dir).expect("alone");
        let batch = prepare_fonts(&[&borrowing, &naming], &dir).expect("in a batch");
        for (prepared, faces) in [("alone", &alone), ("in a batch", &batch)] {
            for ch in "动\u{3000}火点".chars() {
                assert!(
                    carries(faces, "SimSun", ch),
                    "prepared {prepared}: SimSun does not carry {ch:?} of the label it lends to"
                );
            }
        }
    }

    /// Only what the page letters is borrowed. A trailing pad is dropped
    /// before a label is set, so a blank there that the named face does not
    /// draw asks no lender for anything — a face loaded for it would be
    /// embedded and never used.
    #[test]
    fn a_trailing_pad_the_named_face_cannot_draw_asks_no_lender() {
        let narrow = ascii_only();
        let song = drawing("\u{3000}");
        let dir = scratch_dir(
            "trailing-pad",
            &[
                ("ARIALN.TTF", &narrow),
                ("simfang.ttf", &song),
                ("simsun.ttf", &song),
            ],
        );
        let padded = sheet(&[("AB\u{3000}", "Arial Narrow")]);
        let faces = prepare_fonts(&[&padded], &dir).expect("the label's own face is enough");
        let keys: Vec<&str> = faces.iter().map(|face| face.key.as_str()).collect();
        assert_eq!(keys, vec!["Arial Narrow"], "nothing is borrowed for a pad");
    }

    /// A character none of the order's three Chinese faces draws is borrowed
    /// from the first of the faces after them whose program draws it, and
    /// that face is loaded for it. `甲` here is drawn by Arial and by
    /// SimSun-ExtB and by neither SimSun nor FangSong, and the machine has no
    /// NSimSun; Arial comes first. Looking only among the faces a call has
    /// loaded would refuse the sheet prepared alone, and beside a sheet that
    /// names SimSun-ExtB would find the character there.
    #[test]
    fn a_character_neither_lender_draws_is_given_to_the_same_face_alone_and_in_a_batch() {
        let latin = ascii_only();
        let han = drawing("甲");
        let dir = scratch_dir(
            "last-resort",
            &[
                ("ARIALN.TTF", &latin),
                ("simsun.ttf", &latin),
                ("simfang.ttf", &latin),
                ("arial.ttf", &han),
                ("simsunb.ttf", &han),
            ],
        );
        let borrowing = sheet(&[("甲", "Arial Narrow")]);
        let naming = sheet(&[("甲", "SimSun-ExtB")]);
        let alone = prepare_fonts(&[&borrowing], &dir).expect("alone");
        let batch = prepare_fonts(&[&borrowing, &naming], &dir).expect("in a batch");
        for (prepared, faces) in [("alone", &alone), ("in a batch", &batch)] {
            assert!(
                carries(faces, "Arial", '甲'),
                "prepared {prepared}: Arial, the first face after the Chinese ones that draws \
                 '甲', does not carry it"
            );
        }
        let keys: Vec<&str> = alone.iter().map(|face| face.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["Arial Narrow", "Arial"],
            "prepared alone, only the face that lends is loaded beside the label's own"
        );
    }

    /// A character no face draws refuses the export, and the refusal names
    /// the character and the label that holds it. It is refused whatever the
    /// batch: `乙` is drawn by no program here, with the sheet alone and
    /// beside a sheet that loads Arial for a label of its own.
    #[test]
    fn a_character_no_face_draws_is_refused_by_name_alone_and_in_a_batch() {
        let latin = ascii_only();
        let han = drawing("甲");
        let dir = scratch_dir(
            "no-face-draws",
            &[
                ("ARIALN.TTF", &latin),
                ("simsun.ttf", &latin),
                ("simfang.ttf", &latin),
                ("arial.ttf", &han),
            ],
        );
        let lacking = sheet(&[("甲乙", "Arial Narrow")]);
        let naming = sheet(&[("甲", "Arial")]);
        for (prepared, models) in [
            ("alone", vec![&lacking]),
            ("in a batch", vec![&lacking, &naming]),
        ] {
            let Err(error) = prepare_fonts(&models, &dir) else {
                panic!(
                    "prepared {prepared}: a label holding a character no face draws was accepted"
                );
            };
            assert!(
                error.contains("'乙'") && error.contains("\"甲乙\""),
                "prepared {prepared}: the refusal does not name the character and its label: {error}"
            );
            assert!(
                !error.contains("'甲'"),
                "prepared {prepared}: the refusal names a character Arial draws: {error}"
            );
        }
    }

    /// A scratch font directory is the test's own and goes when the test
    /// lets go of it. Two made under one name are two directories, so tests
    /// running at once cannot swap each other's faces; each carries the
    /// process id, so two runs sharing a temporary directory cannot either;
    /// and dropping one removes it with its faces and leaves the other.
    #[test]
    fn a_scratch_font_directory_is_its_tests_own_and_goes_with_it() {
        let first = scratch_dir("scratch", &[("arial.ttf", b"a face")]);
        let second = scratch_dir("scratch", &[]);
        let (first_path, second_path) = (first[0].clone(), second[0].clone());
        assert_ne!(first_path, second_path, "one name made one directory twice");
        let process = std::process::id().to_string();
        for path in [&first_path, &second_path] {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a directory name");
            assert!(
                name.split('-').any(|part| part == process),
                "{name} does not carry the process id {process}"
            );
        }
        assert!(first_path.join("arial.ttf").is_file());
        assert!(!second_path.join("arial.ttf").exists());
        drop(first);
        assert!(
            !first_path.exists(),
            "{} outlived its test",
            first_path.display()
        );
        assert!(
            second_path.is_dir(),
            "dropping one directory removed another"
        );
        drop(second);
        assert!(
            !second_path.exists(),
            "{} outlived its test",
            second_path.display()
        );
    }

    /// The face the reference plots borrow leads the order, and the face of
    /// the same design follows it, ahead of FangSong: a machine without the
    /// NSimSun file still borrows in the reference's family.
    #[test]
    fn the_borrow_order_is_led_by_the_face_the_reference_borrows() {
        assert_eq!(BORROW_ORDER[..3], ["NSimSun", "SimSun", "FangSong"]);
    }

    /// The order is every face the loader knows a file for, each once: a
    /// character none of them draws is drawn by no face this crate loads.
    #[test]
    fn the_borrow_order_holds_every_face_the_loader_knows() {
        let mut order = BORROW_ORDER.to_vec();
        order.sort_unstable();
        assert_eq!(
            order,
            [
                "Arial",
                "Arial Narrow",
                "FangSong",
                "NSimSun",
                "SimSun",
                "SimSun-ExtB"
            ]
        );
        for key in order {
            assert_eq!(
                typeface_key(Some(key), "120℃"),
                key,
                "{key} is not a face a style can name"
            );
        }
    }

    #[test]
    fn a_stated_arial_narrow_stays_arial_narrow() {
        assert_eq!(typeface_key(Some("Arial Narrow"), "PT"), "Arial Narrow");
    }

    #[test]
    fn the_damaged_songti_name_uses_simsun() {
        assert_eq!(typeface_key(Some("ËÎÌå"), "阀"), "SimSun");
    }

    #[test]
    fn han_lettering_without_a_typeface_uses_fangsong() {
        assert_eq!(typeface_key(None, "低温"), "FangSong");
    }

    #[test]
    fn latin_lettering_without_a_typeface_uses_arial_narrow() {
        assert_eq!(typeface_key(None, "NULLNULL"), "Arial Narrow");
    }
}
