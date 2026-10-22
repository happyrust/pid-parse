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

use crate::model::{PaperModel, PaperShape};

/// One face ready to embed. `key` is the name [`typeface_key`] returns.
pub struct PreparedFont {
    /// Canonical typeface name, such as `SimSun` or `Arial Narrow`.
    pub key: String,
    /// The font file's complete bytes.
    pub bytes: Vec<u8>,
}

/// Which face a run should use.
///
/// A name the style table states is used as stated. `ËÎÌå` is the vendor's
/// damaged spelling of `宋体`, documented on
/// [`pid_parse::style_link::TEXT_CHAR_FONT_NAME_OFFSET`]. Anything else
/// with a Han character uses FangSong; Latin-only lettering uses Arial
/// Narrow. Individual characters the chosen face cannot encode borrow
/// another loaded face, one character at a time — see
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
/// Characters no loaded face covers pull the CJK fallback faces (`FangSong`,
/// `SimSun`) in too — the run keeps its named face for everything it can
/// encode and only the uncovered characters borrow.
///
/// # Errors
///
/// Returns an error when a needed face is absent from every directory, or
/// a file is present but not an embeddable standalone font.
pub fn prepare_fonts(
    models: &[&PaperModel],
    dirs: &[PathBuf],
) -> Result<Vec<PreparedFont>, String> {
    let mut wanted: BTreeMap<&'static str, String> = BTreeMap::new();
    for model in models {
        for primitive in &model.primitives {
            let PaperShape::Text { value, font, .. } = &primitive.shape else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            wanted
                .entry(typeface_key(font.as_deref(), value))
                .or_default()
                .push_str(value);
        }
    }
    let mut faces: Vec<PreparedFont> = Vec::new();
    let mut covered = String::new();
    for (key, text) in &wanted {
        let bytes = load_face(dirs, key, text)?;
        for ch in text.chars() {
            let mut buf = [0u8; 4];
            if pdfcraft_fonts::embedding::covers(&bytes, ch.encode_utf8(&mut buf)) {
                covered.push(ch);
            }
        }
        faces.push(PreparedFont {
            key: (*key).to_string(),
            bytes,
        });
    }
    // A run inside a label may name a face the label's own style does not —
    // `Wastewater Pit.sym` letters one `*` in 宋体 inside an Arial line, and a
    // sheet label may mix them the same way. Those faces are **optional**: a
    // machine without the file draws that stretch in the face the encoder
    // falls back to, which is what the page did before partitions were read,
    // while demanding it would refuse a whole export over one missing
    // typeface. A file that is present but unusable is skipped for the same
    // reason.
    let mut run_faces: BTreeMap<&'static str, String> = BTreeMap::new();
    for model in models {
        for primitive in &model.primitives {
            let PaperShape::Text { value, spans, .. } = &primitive.shape else {
                continue;
            };
            if value.is_empty() {
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
        let Ok(bytes) = subset_or_whole(key, &bytes, text) else {
            continue;
        };
        faces.push(PreparedFont {
            key: (*key).to_string(),
            bytes,
        });
    }
    // A character no named face covers borrows a CJK fallback.
    let uncovered: BTreeSet<char> = wanted
        .values()
        .flat_map(|text| text.chars())
        .filter(|ch| !covered.contains(*ch) && !ch.is_whitespace())
        .collect();
    if !uncovered.is_empty() {
        let fallback_text: String = uncovered.iter().collect();
        for fallback in ["FangSong", "SimSun"] {
            let still: Vec<char> = uncovered
                .iter()
                .copied()
                .filter(|ch| !face_covers(&faces, *ch))
                .collect();
            if still.is_empty() {
                break;
            }
            if let Some(bytes) = face_file(dirs, fallback)? {
                let bytes = subset_or_whole(fallback, &bytes, &fallback_text)?;
                faces.push(PreparedFont {
                    key: fallback.to_string(),
                    bytes,
                });
            }
        }
        let still: String = uncovered
            .iter()
            .copied()
            .filter(|ch| !face_covers(&faces, *ch))
            .collect();
        if !still.is_empty() {
            return Err(format!(
                "no loaded face can draw {still:?} (searched {})",
                searched_dirs(dirs)
            ));
        }
    }
    Ok(faces)
}

/// Whether any loaded face encodes `ch`.
fn face_covers(faces: &[PreparedFont], ch: char) -> bool {
    let mut buf = [0u8; 4];
    let text = ch.encode_utf8(&mut buf);
    faces
        .iter()
        .any(|face| pdfcraft_fonts::embedding::covers(&face.bytes, text))
}

/// The face `key`'s file, subset to the run's characters when the source
/// program is worth shrinking.
fn load_face(dirs: &[PathBuf], key: &str, text: &str) -> Result<Vec<u8>, String> {
    let bytes = face_file(dirs, key)?.ok_or_else(|| missing_face(dirs, key))?;
    subset_or_whole(key, &bytes, text)
}

/// `fontcull`'s subset of `bytes` down to `text` plus the printable ASCII
/// baseline; the whole file where subsetting fails or the program already
/// fits — a file that fits is validated the same way a subset is, so a
/// bad file never loads silently.
fn subset_or_whole(key: &str, bytes: &[u8], text: &str) -> Result<Vec<u8>, String> {
    let chars: HashSet<char> = ('\u{20}'..='\u{7e}').chain(text.chars()).collect();
    if let Ok(subset) = subset_face(bytes, &chars)
        && pdfcraft_fonts::embedding::validate(&subset).is_ok()
    {
        return Ok(subset);
    }
    pdfcraft_fonts::embedding::validate(bytes).map_err(|error| format!("{key}: {error}"))?;
    Ok(bytes.to_vec())
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{font_dirs, prepare_fonts, subset_or_whole, typeface_key};
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
        let subset = subset_or_whole("SimSun", &bytes, text).expect("subset SimSun");
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
                    spans,
                },
                stream_path: None,
                byte_range: None,
                symbol: None,
                stroke: None,
            }],
            missing_symbols: Vec::new(),
            missing_boxes: Vec::new(),
            symbols_found: 0,
            symbols_referenced: 0,
        }
    }

    /// A scratch directory holding one face under the name the loader looks
    /// for, so these two tests need no licensed fonts on the machine.
    fn scratch_dir(name: &str, faces: &[(&str, &[u8])]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pid-plot-fonts-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        for (file, bytes) in faces {
            std::fs::write(dir.join(file), bytes).expect("scratch face");
        }
        dir
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
        let faces = prepare_fonts(&[&model], &[dir]).expect("both faces load");
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
        let faces = prepare_fonts(&[&model], &[dir]).expect("the label's own face is enough");
        let keys: Vec<&str> = faces.iter().map(|face| face.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["Arial"],
            "the missing run face is skipped, not fatal"
        );
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
