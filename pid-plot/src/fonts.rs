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
use std::sync::atomic::{AtomicU64, Ordering};

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

static SUBSET_ID: AtomicU64 = AtomicU64::new(0);

struct SubsetWorkspace {
    path: PathBuf,
}

impl SubsetWorkspace {
    fn new() -> Result<Self, String> {
        loop {
            let id = SUBSET_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("pid-plot-subset-{}-{id}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("create subset workspace: {error}")),
            }
        }
    }
}

impl Drop for SubsetWorkspace {
    fn drop(&mut self) {
        // This directory was exclusively created for this invocation.
        // Cleanup must not replace a subsetting error during unwinding.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn subset_face(source: &Path, face_index: u32, text: &str, key: &str) -> Result<Vec<u8>, String> {
    subset_face_in(
        SubsetWorkspace::new()?,
        source,
        face_index,
        text,
        key,
        Path::new("python"),
    )
}

fn subset_face_in(
    workspace: SubsetWorkspace,
    source: &Path,
    face_index: u32,
    text: &str,
    key: &str,
    python: &Path,
) -> Result<Vec<u8>, String> {
    let script = workspace.path.join("subset.py");
    let text_path = workspace.path.join("chars.txt");
    let dest = workspace.path.join("face.ttf");
    std::fs::write(&script, SUBSET_PY).map_err(|error| error.to_string())?;
    std::fs::write(&text_path, text).map_err(|error| error.to_string())?;
    let output = Command::new(python)
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
    use super::{SubsetWorkspace, fonts_dir, subset_face_in, typeface_key};
    use std::path::Path;

    #[test]
    fn subset_face_cleans_up_when_python_cannot_start() {
        let workspace = SubsetWorkspace::new().expect("workspace");
        let dir = workspace.path.clone();
        let python = dir.join("missing-python.exe");
        let error = subset_face_in(workspace, Path::new("unused.ttf"), 0, "中", "test", &python)
            .expect_err("missing interpreter must fail");
        assert!(error.starts_with("python:"), "{error}");
        assert!(!dir.exists(), "leftover workspace: {}", dir.display());
    }

    #[test]
    fn subset_face_cleans_up_when_text_cannot_be_written() {
        let workspace = SubsetWorkspace::new().expect("workspace");
        let dir = workspace.path.clone();
        std::fs::create_dir(dir.join("chars.txt")).expect("block text write");
        let result = subset_face_in(
            workspace,
            Path::new("unused.ttf"),
            0,
            "中",
            "test",
            Path::new("python"),
        );
        assert!(result.is_err(), "writing text over a directory must fail");
        assert!(!dir.exists(), "leftover workspace: {}", dir.display());
    }

    #[test]
    fn subset_face_cleans_up_when_python_rejects_the_font() {
        let workspace = SubsetWorkspace::new().expect("workspace");
        let dir = workspace.path.clone();
        let source = dir.join("not-a-font.ttf");
        std::fs::write(&source, b"not a font").expect("bad font");
        let error = subset_face_in(workspace, &source, 0, "中", "test", Path::new("python"))
            .expect_err("invalid font must fail");
        assert!(error.starts_with("subset test failed:"), "{error}");
        assert!(!dir.exists(), "leftover workspace: {}", dir.display());
    }

    #[test]
    fn subset_face_cleans_up_after_concurrent_successes() {
        let source = fonts_dir().join("simsun.ttc");
        if !source.is_file() {
            eprintln!(
                "skipping subset success test; {} is absent",
                source.display()
            );
            return;
        }
        let first = SubsetWorkspace::new().expect("first workspace");
        let second = SubsetWorkspace::new().expect("second workspace");
        let first_dir = first.path.clone();
        let second_dir = second.path.clone();
        assert_ne!(first_dir, second_dir);
        let (first_result, second_result) = std::thread::scope(|scope| {
            let first_job = scope
                .spawn(|| subset_face_in(first, &source, 0, "中", "test", Path::new("python")));
            let second_job = scope
                .spawn(|| subset_face_in(second, &source, 0, "文", "test", Path::new("python")));
            (
                first_job.join().expect("first thread"),
                second_job.join().expect("second thread"),
            )
        });
        let first_bytes = first_result.expect("first subset");
        let second_bytes = second_result.expect("second subset");
        assert!(!first_bytes.is_empty());
        assert!(!second_bytes.is_empty());
        assert_ne!(
            first_bytes, second_bytes,
            "different characters need different subsets"
        );
        assert!(!first_dir.exists(), "leftover first workspace");
        assert!(!second_dir.exists(), "leftover second workspace");
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
