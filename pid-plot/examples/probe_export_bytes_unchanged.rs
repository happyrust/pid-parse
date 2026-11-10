//! Whether the export still writes the same pages: 71 recorded, two
//! recordings compared, every page that differs named with what moved on it.
//!
//! The export ledger (`.kiro/specs/pdf-export-ledger/`) only observes the plot
//! pipeline, and its hard guardrail was that nothing it added may change what
//! the writer emits: 53 backup sheets, 7 committed fixtures and 11 reference
//! sheets had to keep the same PDF bytes **and** the same `PaperModel` bytes.
//! That is what this probe was written to hold. It is also what a merge
//! needs: record the pages on each side, record them again on the merged
//! tree, and read off which pages moved and in what.
//!
//! ```text
//! cargo run -q -p pid-plot --example probe_export_bytes_unchanged -- record before
//! cargo run -q -p pid-plot --example probe_export_bytes_unchanged -- record after
//! cargo run -q -p pid-plot --example probe_export_bytes_unchanged -- compare before after
//! cargo run -q -p pid-plot --example probe_export_bytes_unchanged -- show after
//! ```
//!
//! A recording lands in `target/export-parity/<label>.json` (`target/` is
//! gitignored). `compare` and `show` take a label or the path of a recording,
//! so one made in another checkout is read where it lies. Each page keeps the
//! PDF's SHA-256 and byte count, the SHA-256 of `serde_json::to_vec(&model)`,
//! and the model taken apart ([`ModelParts`]); the header keeps the commit,
//! whether `pid-plot/src` or `src` had uncommitted changes, the typeface keys
//! the run embedded, and how each batch was laid out.
//!
//! # The three batches, and the corpus each is laid out against
//!
//! A recording is of three batches, each laid out one way, so that two
//! recordings taken the same way differ only by what the code between them
//! changed:
//!
//! - **backup** (53) — `backup-plant-data/**/*.pid`, laid out with
//!   `lay_out_drawing(path, corpus)`, faces prepared over all 53 at once. The
//!   backup directory is found the way `export_plant_backup` finds it (see
//!   [`backup_dir`]), and the corpus is the one that gate reads by default:
//!   the package the backup was shelled out of — the directory above it, when
//!   that holds `RefData~*.zip` — and the share otherwise.
//! - **fixtures** (7) — `test-file/**/*.pid`, laid out against `test-file`
//!   itself, so each wears the border template the repository commits for it.
//!   Against a plant's package the 熊安 fixture finds no `XA-A1.igr` and
//!   draws 478 of its 5 478 primitives; a change to that template would not
//!   show.
//! - **reference** (11) — `PID_ACCEPTANCE_CORPUS/<name>.pid` for the eleven
//!   names the acceptance gate pins, laid out against the acceptance corpus
//!   itself and lettered with that gate's font search order.
//!
//! `PID_PARSE_SELF_CHECK_DIR` names one corpus for the first two batches
//! instead, which is how the recordings of the ledger work were taken
//! (`before-share`, `before-package`). Two recordings whose batches were laid
//! out against different corpora answer a different question, and `compare`
//! says so before it lists anything.
//!
//! The fixtures batch is not the pages its gates pin: `export_every_fixture`
//! and `paper_model_golden` lay the seven out with `lay_out_with_symbol_roots`
//! from declared sources, where this probe lets `lay_out_drawing` search, and
//! on a machine that links a symbol library in under `test-file` the search
//! finds it.
//!
//! # What `compare` says of a page
//!
//! A page whose PDF and model both hash as before is not listed. One that is
//! listed says which of these moved:
//!
//! - **geometry** — every primitive's shape, less its colour: where the ink is.
//! - **ink** — every primitive's pen, and a fill's colour: what it is drawn with.
//! - **provenance** — the record each primitive came from, its symbol, its
//!   confidence, whether it is furniture.
//! - **facts** — the model less its primitives: page size, template, the
//!   symbols it placed and missed, its counters.
//! - **PDF only** — the model reads the same in all four and the bytes differ:
//!   the writer changed, or the faces did.
//!
//! So a change of pens reads `ink` on every page it touches and nothing else,
//! and a decoder that starts drawing a record reads `geometry, ink, provenance`
//! with a primitive count that went up.
//!
//! # A checkout's own path is not part of a page
//!
//! `PaperModel` serializes absolute paths whose values follow where the corpus
//! and the checkout live, not what the page draws: `library` (the symbol
//! directories `symbol_library()` found), `template.path`, and
//! `template.symbols[].key`. `model_sha256` hashes the model as it is, so it
//! agrees only between recordings made in one checkout against one corpus.
//! The four parts are hashed with the checkout's root written `<repo>`, which
//! is what lets a recording made in one worktree be held against one made in
//! another; a path under the corpus still follows the corpus, and a path the
//! `.pid` itself stores is source bytes and is left alone.
//!
//! # What it calls
//!
//! `lay_out_drawing`, `font_dirs`, `prepare_fonts`, `write_plot`, and the
//! model through `serde_json` — never a field of `PaperModel` by name. That is
//! what lets this one file be dropped into either side of a change and built
//! there.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::write::{PlotFont, write_plot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The eleven reference sheets, by the name each one's `.pid` and `.pdf` share.
///
/// Copied from `pid-plot/tests/reference_plots_acceptance.rs`'s `PINS` rather
/// than read out of that file: a probe that parses a test's source is a probe
/// that breaks when the test is reformatted. A page count other than 11 is an
/// error, so a drift here is loud.
const PINS: [&str; 11] = [
    "I井场 注采阀组工艺及自控流程图",
    "J K井场 注采阀组工艺及自控流程图",
    "L M井场 注采阀组工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图",
    "集注站 烃水露点控制单元（一） 空冷器A集预冷分离器A工艺及自控流程图",
    "集注站 烃水露点控制单元（二） 空冷器B集预冷分离器B工艺及自控流程图",
    "集注站 烃水露点控制单元（三） 空冷器C集预冷分离器C工艺及自控流程图",
    "集注站 轻烃分离器I气出口接口位置改造工艺及自控流程图",
];

/// How many pages a full recording covers: 53 backup + 7 fixtures + 11
/// reference.
const TOTAL_PAGES: usize = 53 + 7 + 11;

/// What stands for the checkout's root in a hashed part and in a header.
const REPO: &str = "<repo>";

/// A page model taken apart, each part hashed on its own.
///
/// Every string in the model has the checkout's root written [`REPO`] before
/// a part is hashed, and every object is written with its keys in order, so a
/// part reads the same from any worktree and under either `serde_json` map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModelParts {
    /// How many primitives the page holds.
    primitives: usize,
    /// SHA-256 over each primitive's `shape` less its `rgb`, in order.
    geometry_sha256: String,
    /// SHA-256 over each primitive's `stroke` and its shape's `rgb`, in order.
    ink_sha256: String,
    /// SHA-256 over what is left of each primitive, in order: the record it
    /// came from, its symbol, its confidence, whether it is furniture.
    provenance_sha256: String,
    /// SHA-256 over the model less its `primitives`.
    facts_sha256: String,
}

impl ModelParts {
    /// The parts of `self` that `other` hashes differently, by name.
    fn moved(&self, other: &Self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.geometry_sha256 != other.geometry_sha256 {
            out.push("geometry");
        }
        if self.ink_sha256 != other.ink_sha256 {
            out.push("ink");
        }
        if self.provenance_sha256 != other.provenance_sha256 {
            out.push("provenance");
        }
        if self.facts_sha256 != other.facts_sha256 {
            out.push("facts");
        }
        out
    }
}

/// One page's reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PageRecord {
    /// Which of the three batches this page belongs to.
    batch: String,
    /// The page's key: relative to the backup root, relative to the repository,
    /// or the reference sheet's name.
    page: String,
    /// How many bytes the PDF took.
    pdf_bytes: usize,
    /// SHA-256 of the PDF, lowercase hex.
    pdf_sha256: String,
    /// SHA-256 of `serde_json::to_vec(&model)`, lowercase hex.
    ///
    /// The model as it is, the checkout's paths included: it agrees only
    /// between recordings made in one checkout against one corpus.
    model_sha256: String,
    /// The model taken apart; absent from a recording made before the parts
    /// were read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parts: Option<ModelParts>,
}

/// How one batch was laid out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BatchRecord {
    /// The batch's name.
    batch: String,
    /// The corpus its pages were laid out against, the checkout's root
    /// written [`REPO`].
    corpus: String,
    /// Why it was that corpus.
    why: String,
    /// Where its faces were looked for, in search order.
    font_dirs: Vec<String>,
}

/// One recording: a header plus one record per page.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    /// The label this recording was written under.
    label: String,
    /// `git rev-parse HEAD` at recording time.
    head: String,
    /// Whether `git status --porcelain -- pid-plot/src src` was non-empty.
    worktree_dirty: bool,
    /// The typeface keys this run embedded, sorted and deduplicated.
    font_faces: Vec<String>,
    /// How each batch was laid out; empty in a recording made before this was
    /// kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    batches: Vec<BatchRecord>,
    /// One record per page, sorted by `(batch, page)`.
    pages: Vec<PageRecord>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("record") => match args.get(1) {
            Some(label) => record(label),
            None => usage("record needs a label"),
        },
        Some("compare") => match (args.get(1), args.get(2)) {
            (Some(before), Some(after)) => compare(before, after),
            _ => usage("compare needs two recordings"),
        },
        Some("show") => match args.get(1) {
            Some(recording) => show(recording),
            None => usage("show needs a recording"),
        },
        _ => usage("expected `record <label>`, `compare <before> <after>` or `show <recording>`"),
    };
    if code != 0 {
        std::process::exit(code);
    }
}

fn usage(message: &str) -> i32 {
    eprintln!("probe_export_bytes_unchanged: {message}");
    eprintln!();
    eprintln!("usage:");
    eprintln!("  probe_export_bytes_unchanged record <label>");
    eprintln!("  probe_export_bytes_unchanged compare <before> <after>");
    eprintln!("  probe_export_bytes_unchanged show <recording>");
    eprintln!();
    eprintln!("`record` writes target/export-parity/<label>.json; a recording is named by");
    eprintln!("its label or by the path of its file");
    2
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pid-plot sits in the repository")
        .to_path_buf()
}

fn parity_dir() -> PathBuf {
    repo_root().join("target").join("export-parity")
}

/// The corpus a run names for the backup and the fixtures, if it names one.
fn named_corpus() -> Option<PathBuf> {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map(PathBuf::from)
}

/// The reference share this machine keeps the licensed faces in.
fn share_dir() -> PathBuf {
    PathBuf::from(r"D:\work\cad\PID自研软件验证")
}

/// The reference-plot corpus.
fn acceptance_corpus() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

/// Where the shelled-out plant backup sits, if this machine has one.
///
/// The same lookup order `export_plant_backup`'s `backup_dir` uses, so the
/// probe records the pages that gate actually exports.
fn backup_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(named_corpus().unwrap_or_else(share_dir));
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots
        .into_iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|candidate| candidate.is_dir())
}

/// Every `.pid` under `dir`, recursively, in sorted path order.
fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

/// A page key: `path` below `root`, with `/` separators.
fn relative_key(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn git_line(root: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// `text` with the checkout's root written [`REPO`].
///
/// The root reaches a model spelled two ways: as the directory itself, and as
/// the plot crate's manifest directory with `..` after it, which is how the
/// crate names `test-file` — with either separator after either.
fn portable(text: &str) -> String {
    let root = repo_root().to_string_lossy().to_string();
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let mut out = text.to_string();
    for spelling in [
        format!("{crate_dir}\\.."),
        format!("{crate_dir}/.."),
        root.clone(),
        root.replace('\\', "/"),
    ] {
        out = out.replace(&spelling, REPO);
    }
    out
}

/// [`portable`], over every string a value holds.
fn make_portable(value: &mut Value) {
    match value {
        Value::String(text) => *text = portable(text),
        Value::Array(items) => items.iter_mut().for_each(make_portable),
        Value::Object(map) => map.values_mut().for_each(make_portable),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// `value` as JSON with every object's keys in order, appended to `out`.
fn canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        leaf => out.push_str(&leaf.to_string()),
    }
}

/// Take a model apart and hash each part.
fn parts_of(model: &PaperModel) -> Result<ModelParts, serde_json::Error> {
    let mut value = serde_json::to_value(model)?;
    make_portable(&mut value);
    let primitives = match value.get_mut("primitives").map(Value::take) {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    };
    if let Value::Object(map) = &mut value {
        map.remove("primitives");
    }
    let count = primitives.len();
    let mut geometry = String::new();
    let mut ink = String::new();
    let mut provenance = String::new();
    for mut primitive in primitives {
        let mut shape = primitive
            .get_mut("shape")
            .map(Value::take)
            .unwrap_or(Value::Null);
        let colour = match &mut shape {
            Value::Object(map) => map.remove("rgb").unwrap_or(Value::Null),
            _ => Value::Null,
        };
        let stroke = primitive
            .get_mut("stroke")
            .map(Value::take)
            .unwrap_or(Value::Null);
        if let Value::Object(map) = &mut primitive {
            map.remove("shape");
            map.remove("stroke");
        }
        canonical(&shape, &mut geometry);
        geometry.push('\n');
        canonical(&stroke, &mut ink);
        ink.push(' ');
        canonical(&colour, &mut ink);
        ink.push('\n');
        canonical(&primitive, &mut provenance);
        provenance.push('\n');
    }
    let mut facts = String::new();
    canonical(&value, &mut facts);
    Ok(ModelParts {
        primitives: count,
        geometry_sha256: sha256_hex(geometry.as_bytes()),
        ink_sha256: sha256_hex(ink.as_bytes()),
        provenance_sha256: sha256_hex(provenance.as_bytes()),
        facts_sha256: sha256_hex(facts.as_bytes()),
    })
}

/// One batch: its pages, the corpus they lay out against, and its face search.
struct Batch {
    name: &'static str,
    corpus: PathBuf,
    /// Why the batch is laid out against that corpus.
    why: &'static str,
    dirs: Vec<PathBuf>,
    /// `(page key, .pid path)`.
    pages: Vec<(String, PathBuf)>,
}

impl Batch {
    fn record(&self) -> BatchRecord {
        BatchRecord {
            batch: self.name.to_string(),
            corpus: portable(&self.corpus.to_string_lossy()),
            why: self.why.to_string(),
            font_dirs: self
                .dirs
                .iter()
                .map(|dir| portable(&dir.to_string_lossy()))
                .collect(),
        }
    }
}

/// The face search of a batch laid out against `corpus`: [`font_dirs`], and
/// after it the share's `fonts` when the run named no corpus — a backup
/// writes no faces into its package and the repository commits none, which is
/// the order `export_plant_backup` searches a package it picked by default.
fn face_dirs(corpus: &Path, named: bool) -> Vec<PathBuf> {
    let mut dirs = font_dirs(Some(corpus));
    let beside = share_dir().join("fonts");
    if !named && !dirs.contains(&beside) {
        dirs.push(beside);
    }
    dirs
}

fn batches() -> Result<Vec<Batch>, String> {
    let repo = repo_root();
    let named = named_corpus();
    let mut out = Vec::new();

    let backup =
        backup_dir().ok_or_else(|| "no backup-plant-data directory is present".to_string())?;
    let mut drawings = Vec::new();
    collect_pids(&backup, &mut drawings);
    if drawings.is_empty() {
        return Err(format!("{} carries no .pid", backup.display()));
    }
    let (backup_corpus, backup_why) = match (&named, backup.parent()) {
        (Some(dir), _) => (dir.clone(), "named by PID_PARSE_SELF_CHECK_DIR"),
        (None, Some(package))
            if !pid_parse::refdata_archive::refdata_archives(package).is_empty() =>
        {
            (
                package.to_path_buf(),
                "the backup package, by default: backup-plant-data sits in it, beside its \
                 RefData~*.zip",
            )
        }
        (None, _) => (
            share_dir(),
            "the share, by default: no RefData~*.zip sits beside backup-plant-data",
        ),
    };
    out.push(Batch {
        name: "backup",
        dirs: face_dirs(&backup_corpus, named.is_some()),
        corpus: backup_corpus,
        why: backup_why,
        pages: drawings
            .iter()
            .map(|path| (relative_key(&backup, path), path.clone()))
            .collect(),
    });

    let fixtures_root = repo.join("test-file");
    let mut fixtures = Vec::new();
    collect_pids(&fixtures_root, &mut fixtures);
    if fixtures.is_empty() {
        return Err(format!("{} carries no .pid", fixtures_root.display()));
    }
    let (fixture_corpus, fixture_why) = match &named {
        Some(dir) => (dir.clone(), "named by PID_PARSE_SELF_CHECK_DIR"),
        None => (
            fixtures_root.clone(),
            "test-file, by default: the templates the repository commits for them",
        ),
    };
    out.push(Batch {
        name: "fixtures",
        dirs: face_dirs(&fixture_corpus, named.is_some()),
        corpus: fixture_corpus,
        why: fixture_why,
        pages: fixtures
            .iter()
            .map(|path| (relative_key(&repo, path), path.clone()))
            .collect(),
    });

    let acceptance = acceptance_corpus();
    let mut reference_pages = Vec::new();
    for name in PINS {
        let path = acceptance.join(format!("{name}.pid"));
        if !path.is_file() {
            return Err(format!("the reference sheet is absent: {}", path.display()));
        }
        reference_pages.push((format!("{name}.pid"), path));
    }
    // The acceptance gate's own search order: `font_dirs` over the acceptance
    // corpus, then the share's fonts beside it — `验收数据` ships none.
    let mut reference_dirs = font_dirs(Some(&acceptance));
    let beside = share_dir().join("fonts");
    if beside.is_dir() {
        reference_dirs.push(beside);
    }
    out.push(Batch {
        name: "reference",
        corpus: acceptance,
        why: "the acceptance corpus itself",
        dirs: reference_dirs,
        pages: reference_pages,
    });

    Ok(out)
}

/// Lay one batch out, write its pages and append one record per page.
fn record_batch(
    batch: &Batch,
    pages: &mut Vec<PageRecord>,
    faces: &mut Vec<String>,
) -> Result<(), String> {
    if !batch.dirs.iter().any(|dir| dir.is_dir()) {
        return Err(format!(
            "the {} batch has no font directory; searched {:?}",
            batch.name, batch.dirs
        ));
    }
    let mut models: Vec<(&str, PaperModel)> = Vec::new();
    for (key, path) in &batch.pages {
        let model = lay_out_drawing(path, &batch.corpus)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        models.push((key.as_str(), model));
    }
    let refs: Vec<&PaperModel> = models.iter().map(|(_, model)| model).collect();
    let prepared = prepare_fonts(&refs, &batch.dirs)
        .map_err(|error| format!("the {} batch's faces: {error}", batch.name))?;
    for face in &prepared {
        faces.push(face.key.clone());
    }
    let fonts: Vec<PlotFont<'_>> = prepared
        .iter()
        .map(|face| PlotFont {
            key: &face.key,
            bytes: &face.bytes,
            subset: face.subset,
        })
        .collect();
    for (key, model) in &models {
        let pdf = write_plot(model, &fonts).map_err(|error| format!("{key}: {error}"))?;
        let model_bytes =
            serde_json::to_vec(model).map_err(|error| format!("{key}: model: {error}"))?;
        let parts = parts_of(model).map_err(|error| format!("{key}: model parts: {error}"))?;
        pages.push(PageRecord {
            batch: batch.name.to_string(),
            page: (*key).to_string(),
            pdf_bytes: pdf.len(),
            pdf_sha256: sha256_hex(&pdf),
            model_sha256: sha256_hex(&model_bytes),
            parts: Some(parts),
        });
    }
    eprintln!(
        "{}: {} pages recorded against {} ({})",
        batch.name,
        models.len(),
        batch.corpus.display(),
        batch.why
    );
    Ok(())
}

fn record(label: &str) -> i32 {
    let repo = repo_root();
    let head = git_line(&repo, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    let dirty = git_line(
        &repo,
        &["status", "--porcelain", "--", "pid-plot/src", "src"],
    )
    .is_some_and(|status| !status.is_empty());

    let batches = match batches() {
        Ok(batches) => batches,
        Err(error) => {
            eprintln!("probe_export_bytes_unchanged: {error}");
            return 1;
        }
    };

    let mut pages: Vec<PageRecord> = Vec::new();
    let mut faces: Vec<String> = Vec::new();
    for batch in &batches {
        if let Err(error) = record_batch(batch, &mut pages, &mut faces) {
            eprintln!("probe_export_bytes_unchanged: {error}");
            return 1;
        }
    }

    pages.sort_by(|left, right| (&left.batch, &left.page).cmp(&(&right.batch, &right.page)));
    faces.sort();
    faces.dedup();

    if pages.len() != TOTAL_PAGES {
        eprintln!(
            "probe_export_bytes_unchanged: {} pages recorded, expected {TOTAL_PAGES} \
             (53 backup + 7 fixtures + 11 reference)",
            pages.len()
        );
        return 1;
    }

    let snapshot = Snapshot {
        label: label.to_string(),
        head,
        worktree_dirty: dirty,
        font_faces: faces,
        batches: batches.iter().map(Batch::record).collect(),
        pages,
    };
    let dir = parity_dir();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("probe_export_bytes_unchanged: {}: {error}", dir.display());
        return 1;
    }
    let path = dir.join(format!("{label}.json"));
    let mut json = match serde_json::to_string_pretty(&snapshot) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("probe_export_bytes_unchanged: {error}");
            return 1;
        }
    };
    json.push('\n');
    if let Err(error) = std::fs::write(&path, json) {
        eprintln!("probe_export_bytes_unchanged: {}: {error}", path.display());
        return 1;
    }
    eprintln!("{}", describe(&snapshot));
    eprintln!("wrote {}", path.display());
    0
}

/// A recording's header in one line.
fn describe(snapshot: &Snapshot) -> String {
    format!(
        "{}: {} pages, HEAD {}, tree {}, faces {:?}",
        snapshot.label,
        snapshot.pages.len(),
        snapshot.head,
        if snapshot.worktree_dirty {
            "dirty"
        } else {
            "clean"
        },
        snapshot.font_faces
    )
}

/// The file a recording named `recording` is read from: the path itself when
/// it names a file, `target/export-parity/<label>.json` otherwise.
fn recording_path(recording: &str) -> PathBuf {
    let given = PathBuf::from(recording);
    if given.is_file() {
        given
    } else {
        parity_dir().join(format!("{recording}.json"))
    }
}

fn load(recording: &str) -> Option<Snapshot> {
    let path = recording_path(recording);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("probe_export_bytes_unchanged: {}: {error}", path.display());
            return None;
        }
    };
    match serde_json::from_str::<Snapshot>(&text) {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            eprintln!("probe_export_bytes_unchanged: {}: {error}", path.display());
            None
        }
    }
}

/// The first twelve hex digits of a hash: enough to tell two apart by eye.
fn short(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

fn show(recording: &str) -> i32 {
    let Some(snapshot) = load(recording) else {
        return 1;
    };
    println!("{}", describe(&snapshot));
    for batch in &snapshot.batches {
        println!(
            "{}: against {} ({}); faces from {:?}",
            batch.batch, batch.corpus, batch.why, batch.font_dirs
        );
    }
    for page in &snapshot.pages {
        let parts = page.parts.as_ref().map_or_else(
            || "parts not recorded".to_string(),
            |parts| {
                format!(
                    "{} primitives, geometry {} ink {} provenance {} facts {}",
                    parts.primitives,
                    short(&parts.geometry_sha256),
                    short(&parts.ink_sha256),
                    short(&parts.provenance_sha256),
                    short(&parts.facts_sha256)
                )
            },
        );
        println!(
            "{}/{}: PDF {} bytes {}, model {}, {parts}",
            page.batch,
            page.page,
            page.pdf_bytes,
            short(&page.pdf_sha256),
            short(&page.model_sha256)
        );
    }
    0
}

/// What moved on one page between two recordings, or `None` when nothing did.
fn page_difference(old: &PageRecord, new: &PageRecord) -> Option<(String, Vec<&'static str>)> {
    let pdf_moved = old.pdf_sha256 != new.pdf_sha256 || old.pdf_bytes != new.pdf_bytes;
    let (model_moved, kinds, count) = match (&old.parts, &new.parts) {
        (Some(before), Some(after)) => {
            let kinds = before.moved(after);
            let count = if before.primitives == after.primitives {
                format!("{} primitives", after.primitives)
            } else {
                format!("primitives {} -> {}", before.primitives, after.primitives)
            };
            (!kinds.is_empty(), kinds, count)
        }
        // A recording without parts has only the whole model's hash, which
        // also follows the checkout it was made in.
        _ => (
            old.model_sha256 != new.model_sha256,
            Vec::new(),
            "parts not recorded on both sides".to_string(),
        ),
    };
    if !pdf_moved && !model_moved {
        return None;
    }
    let pdf = if !pdf_moved {
        "PDF identical".to_string()
    } else if old.pdf_bytes == new.pdf_bytes {
        format!("PDF differs at {} bytes", new.pdf_bytes)
    } else {
        format!("PDF {} -> {} bytes", old.pdf_bytes, new.pdf_bytes)
    };
    let (what, kinds) = if !model_moved {
        ("model the same in every part".to_string(), vec!["PDF only"])
    } else if kinds.is_empty() {
        ("model differs".to_string(), vec!["model"])
    } else {
        (format!("differs in {}", kinds.join(", ")), kinds)
    };
    Some((format!("{pdf}; {count}; {what}"), kinds))
}

fn compare(before_name: &str, after_name: &str) -> i32 {
    let Some(before) = load(before_name) else {
        return 1;
    };
    let Some(after) = load(after_name) else {
        return 1;
    };
    println!("before  {}", describe(&before));
    println!("after   {}", describe(&after));
    if before.font_faces != after.font_faces {
        println!(
            "note: the two runs embedded different faces, so every page lettered in one of \
             them differs in its PDF"
        );
    }
    for old in &before.batches {
        if let Some(new) = after.batches.iter().find(|new| new.batch == old.batch)
            && (new.corpus != old.corpus || new.font_dirs != old.font_dirs)
        {
            println!(
                "note: the {} batch was laid out against {} with faces from {:?} before, and \
                 against {} with faces from {:?} after; what differs below is that as well as \
                 the code",
                old.batch, old.corpus, old.font_dirs, new.corpus, new.font_dirs
            );
        }
    }
    if before.batches.is_empty() != after.batches.is_empty() {
        println!(
            "note: one recording does not say what its batches were laid out against; it was \
             made before that was kept"
        );
    }

    let key = |record: &PageRecord| format!("{}/{}", record.batch, record.page);
    let after_pages: BTreeMap<String, &PageRecord> =
        after.pages.iter().map(|page| (key(page), page)).collect();
    let before_keys: Vec<String> = before.pages.iter().map(key).collect();

    let mut differences = 0usize;
    let mut checked = 0usize;
    // Pages that differ, by batch, and by what moved on them.
    let mut by_batch: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
    for old in &before.pages {
        let page = key(old);
        let Some(new) = after_pages.get(&page) else {
            println!("{page}: only in {before_name}");
            differences += 1;
            continue;
        };
        checked += 1;
        if let Some((what, kinds)) = page_difference(old, new) {
            println!("{page}: {what}");
            differences += 1;
            *by_batch.entry(old.batch.as_str()).or_default() += 1;
            for kind in kinds {
                *by_kind.entry(kind).or_default() += 1;
            }
        }
    }
    for page in after_pages.keys() {
        if !before_keys.contains(page) {
            println!("{page}: only in {after_name}");
            differences += 1;
        }
    }

    let batches = by_batch
        .iter()
        .map(|(batch, pages)| format!("{batch} {pages}"))
        .collect::<Vec<_>>()
        .join(", ");
    let kinds = by_kind
        .iter()
        .map(|(kind, pages)| format!("{kind} on {pages}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "{checked} pages compared, {differences} differ{}{}",
        if batches.is_empty() {
            String::new()
        } else {
            format!(" ({batches})")
        },
        if kinds.is_empty() {
            String::new()
        } else {
            format!("; {kinds}")
        }
    );
    if checked != TOTAL_PAGES {
        eprintln!("probe_export_bytes_unchanged: {checked} pages compared, expected {TOTAL_PAGES}");
        return 1;
    }
    i32::from(differences != 0)
}
