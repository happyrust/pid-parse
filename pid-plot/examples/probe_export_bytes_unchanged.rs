//! Whether the export still writes the same bytes: 71 pages, before and after.
//!
//! The export ledger (`.kiro/specs/pdf-export-ledger/`) only observes the plot
//! pipeline. Its hard guardrail is that nothing it adds may change what the
//! writer emits: 53 backup sheets, 7 committed fixtures and 11 reference
//! sheets must keep the same PDF bytes **and** the same `PaperModel` bytes as
//! they had before the first library change. This probe records both, then
//! compares two recordings page by page.
//!
//! ```text
//! cargo run -q -p pid-plot --example probe_export_bytes_unchanged -- record before-share
//! cargo run -q -p pid-plot --example probe_export_bytes_unchanged -- record mid-skeleton
//! cargo run -q -p pid-plot --example probe_export_bytes_unchanged -- compare before-share mid-skeleton
//! ```
//!
//! Each recording lands in `target/export-parity/<label>.json` (`target/` is
//! gitignored), one record per page: the PDF's SHA-256 and byte count, and the
//! SHA-256 of `serde_json::to_vec(&model)`. The file header carries the commit,
//! whether `pid-plot/src` or `src` had uncommitted changes, and the typeface
//! keys the run embedded.
//!
//! # The three batches, and the two corpora
//!
//! The pages are laid out exactly the way their own gates lay them out, because
//! a page laid out differently is a different page:
//!
//! - **backup** (53) — `<corpus>/backup-plant-data/**/*.pid`, laid out with
//!   `lay_out_drawing(path, corpus)`, faces prepared over all 53 at once, the
//!   way `export_plant_backup` does it. The backup directory is found the way
//!   that gate finds it (see [`backup_dir`]).
//! - **fixtures** (7) — `test-file/**/*.pid`, the way `export_every_fixture`
//!   does it: `lay_out_drawing(path, corpus)` with the same corpus, faces
//!   prepared over all 7.
//! - **reference** (11) — `PID_ACCEPTANCE_CORPUS/<name>.pid` for the eleven
//!   names the acceptance gate pins, laid out against the acceptance corpus
//!   itself and lettered with that gate's font search order.
//!
//! `PID_PARSE_SELF_CHECK_DIR` names the share corpus (default
//! `D:\work\cad\PID自研软件验证`). Two recordings taken under different values
//! are **not** comparable — see the next section.
//!
//! # Three fields make the model hash environment-dependent
//!
//! `PaperModel` serializes three absolute-path fields whose values follow where
//! the corpus lives, not what the page draws:
//!
//! - `library` — the symbol directories `symbol_library()` found, verbatim;
//! - `template.path` — the `.igr`'s disk path, or "archive path + member name";
//! - `template.symbols[].key` — a symbol key that can be an absolute path from
//!   the machine that authored the `.pid`.
//!
//! So `before-share` and `before-package` were recorded under two different
//! variable sets (a share corpus, then a backup package corpus) and their model
//! hashes are expected to differ even where the PDFs agree. **Compare in
//! pairs**: `before-share` against `mid-share`, `before-package` against
//! `mid-package`. A cross-pair comparison answers a different question and this
//! probe does not pretend otherwise.
//!
//! # What it calls
//!
//! Only APIs whose signatures are the same before and after the ledger lands:
//! `lay_out_drawing`, `font_dirs`, `prepare_fonts`, `write_plot`. That is what
//! lets one binary run on both sides of the change.

use std::path::{Path, PathBuf};

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, lay_out_drawing};
use pid_plot::write::{PlotFont, write_plot};
use serde::{Deserialize, Serialize};
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
    /// The model is serialized without normalization on purpose: normalizing
    /// would hide a changed `primitives` entry behind a path rewrite. The three
    /// environment-dependent fields are handled by recording in pairs instead.
    model_sha256: String,
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
            _ => usage("compare needs two labels"),
        },
        _ => usage("expected `record <label>` or `compare <before> <after>`"),
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
    eprintln!();
    eprintln!("writes and reads target/export-parity/<label>.json");
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

/// The reference share this machine keeps the licensed faces in.
fn share_corpus() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
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
    roots.push(share_corpus());
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

/// One batch: its pages, the corpus they lay out against, and its face search.
struct Batch {
    name: &'static str,
    corpus: PathBuf,
    dirs: Vec<PathBuf>,
    /// `(page key, .pid path)`.
    pages: Vec<(String, PathBuf)>,
}

fn batches() -> Result<Vec<Batch>, String> {
    let repo = repo_root();
    let share = share_corpus();
    let mut out = Vec::new();

    let backup =
        backup_dir().ok_or_else(|| "no backup-plant-data directory is present".to_string())?;
    let mut drawings = Vec::new();
    collect_pids(&backup, &mut drawings);
    if drawings.is_empty() {
        return Err(format!("{} carries no .pid", backup.display()));
    }
    let backup_pages = drawings
        .iter()
        .map(|path| (relative_key(&backup, path), path.clone()))
        .collect();
    out.push(Batch {
        name: "backup",
        corpus: share.clone(),
        dirs: font_dirs(Some(&share)),
        pages: backup_pages,
    });

    let fixtures_root = repo.join("test-file");
    let mut fixtures = Vec::new();
    collect_pids(&fixtures_root, &mut fixtures);
    if fixtures.is_empty() {
        return Err(format!("{} carries no .pid", fixtures_root.display()));
    }
    let fixture_pages = fixtures
        .iter()
        .map(|path| (relative_key(&repo, path), path.clone()))
        .collect();
    out.push(Batch {
        name: "fixtures",
        corpus: share.clone(),
        dirs: font_dirs(Some(&share)),
        pages: fixture_pages,
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
    let beside = PathBuf::from("D:/work/cad/PID自研软件验证/fonts");
    if beside.is_dir() {
        reference_dirs.push(beside);
    }
    out.push(Batch {
        name: "reference",
        corpus: acceptance,
        dirs: reference_dirs,
        pages: reference_pages,
    });

    Ok(out)
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
        if !batch.dirs.iter().any(|dir| dir.is_dir()) {
            eprintln!(
                "probe_export_bytes_unchanged: the {} batch has no font directory; searched {:?}",
                batch.name, batch.dirs
            );
            return 1;
        }
        let mut models: Vec<(&str, PaperModel)> = Vec::new();
        for (key, path) in &batch.pages {
            let model = match lay_out_drawing(path, &batch.corpus) {
                Ok(model) => model,
                Err(error) => {
                    eprintln!("probe_export_bytes_unchanged: {}: {error}", path.display());
                    return 1;
                }
            };
            models.push((key.as_str(), model));
        }
        let refs: Vec<&PaperModel> = models.iter().map(|(_, model)| model).collect();
        let prepared = match prepare_fonts(&refs, &batch.dirs) {
            Ok(prepared) => prepared,
            Err(error) => {
                eprintln!(
                    "probe_export_bytes_unchanged: the {} batch's faces: {error}",
                    batch.name
                );
                return 1;
            }
        };
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
            let pdf = match write_plot(model, &fonts) {
                Ok(pdf) => pdf,
                Err(error) => {
                    eprintln!("probe_export_bytes_unchanged: {key}: {error}");
                    return 1;
                }
            };
            let model_bytes = match serde_json::to_vec(model) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("probe_export_bytes_unchanged: {key}: model: {error}");
                    return 1;
                }
            };
            pages.push(PageRecord {
                batch: batch.name.to_string(),
                page: (*key).to_string(),
                pdf_bytes: pdf.len(),
                pdf_sha256: sha256_hex(&pdf),
                model_sha256: sha256_hex(&model_bytes),
            });
        }
        eprintln!("{}: {} pages recorded", batch.name, models.len());
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
    eprintln!(
        "{label}: {} pages, HEAD {}, tree {}, faces {:?}",
        snapshot.pages.len(),
        snapshot.head,
        if snapshot.worktree_dirty {
            "dirty"
        } else {
            "clean"
        },
        snapshot.font_faces
    );
    eprintln!("wrote {}", path.display());
    0
}

fn load(label: &str) -> Option<Snapshot> {
    let path = parity_dir().join(format!("{label}.json"));
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

fn compare(before_label: &str, after_label: &str) -> i32 {
    let Some(before) = load(before_label) else {
        return 1;
    };
    let Some(after) = load(after_label) else {
        return 1;
    };
    if before.font_faces != after.font_faces {
        eprintln!(
            "probe_export_bytes_unchanged: the two runs embedded different faces — \
             {before_label} {:?} vs {after_label} {:?}; they are not comparable",
            before.font_faces, after.font_faces
        );
        return 1;
    }

    let index = |snapshot: &Snapshot| -> Vec<(String, PageRecord)> {
        snapshot
            .pages
            .iter()
            .map(|record| (format!("{}/{}", record.batch, record.page), record.clone()))
            .collect()
    };
    let before_pages = index(&before);
    let after_pages = index(&after);

    let mut differences = 0usize;
    let mut checked = 0usize;
    for (key, old) in &before_pages {
        let Some(new) = after_pages
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, record)| record)
        else {
            println!("{key}: only in {before_label}");
            differences += 1;
            continue;
        };
        checked += 1;
        if old.pdf_sha256 != new.pdf_sha256 || old.pdf_bytes != new.pdf_bytes {
            println!(
                "{key}: PDF changed — {before_label} {} bytes {}, {after_label} {} bytes {}",
                old.pdf_bytes, old.pdf_sha256, new.pdf_bytes, new.pdf_sha256
            );
            differences += 1;
        } else if old.model_sha256 != new.model_sha256 {
            println!(
                "{key}: model changed, PDF identical — {before_label} {}, {after_label} {}",
                old.model_sha256, new.model_sha256
            );
            differences += 1;
        }
    }
    for (key, _) in &after_pages {
        if !before_pages.iter().any(|(candidate, _)| candidate == key) {
            println!("{key}: only in {after_label}");
            differences += 1;
        }
    }

    println!(
        "{before_label} ({}, HEAD {}) vs {after_label} ({}, HEAD {}): {checked} pages compared, \
         {differences} differences",
        if before.worktree_dirty {
            "dirty"
        } else {
            "clean"
        },
        before.head,
        if after.worktree_dirty {
            "dirty"
        } else {
            "clean"
        },
        after.head
    );
    if checked != TOTAL_PAGES {
        eprintln!("probe_export_bytes_unchanged: {checked} pages compared, expected {TOTAL_PAGES}");
        return 1;
    }
    if differences == 0 { 0 } else { 1 }
}
