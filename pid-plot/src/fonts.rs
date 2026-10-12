//! Typefaces named by a drawing's style table, taken from this machine's
//! font folder and subset to the characters the drawing actually letters.
//!
//! The reference plots use Arial, Arial Narrow, FangSong, SimSun and
//! NSimSun. Those faces are installed with Windows and declare editable
//! embedding. SimSun and NSimSun ship in one collection that is larger
//! than the 16 MiB a PDF font may be, so each used face is subset before
//! it is embedded. The outlines of the characters that are drawn stay the
//! face's own.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::{PaperModel, PaperShape};

/// One face ready to embed. `key` is the name [`typeface_key`] returns.
pub struct PreparedFont {
    /// Canonical typeface name, such as `SimSun` or `Arial Narrow`.
    pub key: String,
    /// Static TrueType bytes, subset to the characters assigned to this face.
    pub bytes: Vec<u8>,
}

/// Which installed face a run should use.
///
/// A name the style table states and this machine has is used as stated.
/// `ËÎÌå` is the vendor's damaged spelling of `宋体`, documented on
/// [`pid_parse::style_link::TEXT_CHAR_FONT_NAME_OFFSET`]. Anything else
/// with a Han character uses FangSong; Latin-only lettering uses Arial Narrow.
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

/// Subset the installed faces `models` actually use.
///
/// # Errors
///
/// Returns an error when a face file is missing, Python or fontTools cannot
/// subset it, or the subset file cannot be read back.
pub fn prepare_local_fonts(models: &[&PaperModel]) -> Result<Vec<PreparedFont>, String> {
    let mut chars: BTreeMap<&'static str, String> = BTreeMap::new();
    for model in models {
        for primitive in &model.primitives {
            let PaperShape::Text { value, font, .. } = &primitive.shape else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            let key = typeface_key(font.as_deref(), value);
            chars.entry(key).or_default().push_str(value);
            // Arial Narrow has no ℃. The fallback faces must carry the
            // whole run, so a character the named face lacks can still be drawn.
            for fallback in ["FangSong", "SimSun"] {
                if fallback != key {
                    chars.entry(fallback).or_default().push_str(value);
                }
            }
        }
    }
    let mut faces = Vec::new();
    for (key, text) in chars {
        let source = installed_face(key)?;
        let bytes = subset_face(&source.path, source.face_index, &text, key)?;
        faces.push(PreparedFont {
            key: key.to_string(),
            bytes,
        });
    }
    Ok(faces)
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

struct InstalledFace {
    path: PathBuf,
    face_index: u32,
}

fn installed_face(key: &str) -> Result<InstalledFace, String> {
    let (file, face_index) = match key {
        "Arial" => ("arial.ttf", 0),
        "Arial Narrow" => ("ARIALN.TTF", 0),
        "FangSong" => ("simfang.ttf", 0),
        "SimSun" => ("simsun.ttc", 0),
        "NSimSun" => ("simsun.ttc", 1),
        "SimSun-ExtB" => ("simsunb.ttf", 0),
        _ => return Err(format!("no installed face for {key}")),
    };
    let path = fonts_dir().join(file);
    if !path.is_file() {
        return Err(format!("{} is not installed at {}", key, path.display()));
    }
    Ok(InstalledFace { path, face_index })
}

fn fonts_dir() -> PathBuf {
    std::env::var_os("WINDIR").map_or_else(
        || PathBuf::from(r"C:\Windows\Fonts"),
        |windir| PathBuf::from(windir).join("Fonts"),
    )
}

fn subset_face(source: &Path, face_index: u32, text: &str, key: &str) -> Result<Vec<u8>, String> {
    let stamp = std::process::id();
    let safe: String = key.chars().filter(char::is_ascii_alphanumeric).collect();
    let dir = std::env::temp_dir();
    let script = dir.join(format!("pid-plot-subset-face-{stamp}.py"));
    let text_path = dir.join(format!("pid-plot-chars-{stamp}-{safe}.txt"));
    let dest = dir.join(format!("pid-plot-face-{stamp}-{safe}.ttf"));
    std::fs::write(&script, SUBSET_PY).map_err(|error| error.to_string())?;
    std::fs::write(&text_path, text).map_err(|error| error.to_string())?;
    let output = Command::new("python")
        .arg(&script)
        .arg(source)
        .arg(&dest)
        .arg(&text_path)
        .arg(face_index.to_string())
        .output()
        .map_err(|error| format!("python: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!("subset {key} failed: {stderr}{stdout}"));
    }
    std::fs::read(&dest).map_err(|error| format!("read subset {key}: {error}"))
}

const SUBSET_PY: &str = r#"
import sys
from fontTools.ttLib import TTFont
from fontTools.subset import Subsetter, Options

source, dest, text_path, face = sys.argv[1:5]
text = open(text_path, encoding="utf-8").read()
font = TTFont(source, fontNumber=int(face))
for tag in ("fvar", "gvar", "avar", "cvar", "HVAR", "VVAR", "MVAR", "STAT"):
    if tag in font:
        del font[tag]
options = Options()
options.name_IDs = ["*"]
options.notdef_outline = True
options.recommended_glyphs = True
subsetter = Subsetter(options=options)
subsetter.populate(text=text)
subsetter.subset(font)
font.save(dest)
"#;

#[cfg(test)]
mod tests {
    use super::typeface_key;

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
