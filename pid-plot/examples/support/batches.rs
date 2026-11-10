//! The three batches of pages this crate exports — the eleven sheets with a
//! vendor plot, the fifty-three of the plant backup and the seven committed
//! fixtures — each laid out the way its own gate lays it out, for the probes
//! that ask all seventy-one pages one question.
//!
//! Not an example of its own: cargo only looks for `examples/*.rs` and
//! `examples/*/main.rs`, so a file under `examples/support/` is source, not a
//! target. Each probe compiles it in with `#[path = "support/batches.rs"]`,
//! so the helper carries items no single probe reads.
//!
//! Read-only: nothing here writes to a corpus.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, lay_out_drawing, lay_out_with_symbol_roots};

/// The eleven sheets `reference_plots_acceptance` compares, in its order.
pub const REFERENCE_SHEETS: [&str; 11] = [
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

/// The seven fixtures `paper_model_golden` pins, as paths under `test-file`.
pub const FIXTURES: [&str; 7] = [
    "D06.pid",
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "工艺管道及仪表流程-1.pid",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
    "export-test/publish-data/A01/A01.pid",
    "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// One page of a batch.
pub struct Sheet {
    /// What tells the page from the others of its batch: the file stem of a
    /// reference sheet, a backup drawing's path under `backup-plant-data`, a
    /// fixture's path under `test-file`.
    pub key: String,
    /// The drawing.
    pub path: PathBuf,
    /// The page as its gate lays it out.
    pub model: PaperModel,
}

/// One batch of pages.
pub struct Batch {
    /// `reference`, `backup` or `fixtures`.
    pub name: &'static str,
    /// The corpus the pages were laid out against.
    pub corpus: PathBuf,
    /// The pages, in a stable order.
    pub sheets: Vec<Sheet>,
}

/// The repository root: `pid-plot/`'s parent.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pid-plot sits in the repository")
        .to_path_buf()
}

/// `PID_ACCEPTANCE_CORPUS`, else the acceptance gate's own default.
pub fn reference_corpus() -> PathBuf {
    std::env::var_os("PID_ACCEPTANCE_CORPUS").map_or_else(
        || PathBuf::from(r"D:\work\cad\pid-test-data\验收数据"),
        PathBuf::from,
    )
}

/// The reference share this machine keeps the licensed faces in.
pub fn share_dir() -> PathBuf {
    PathBuf::from(r"D:\work\cad\PID自研软件验证")
}

/// The shelled-out plant backup, in `export_plant_backup`'s lookup order.
pub fn backup_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(share_dir, PathBuf::from));
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots
        .into_iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|candidate| candidate.is_dir())
}

/// The corpus the backup's drawings are laid out against, as
/// `export_plant_backup` picks it: the one the run names, else the package
/// the backup was shelled out of when its `RefData~*.zip` sit above it, else
/// the share.
pub fn backup_corpus(backup: &Path) -> PathBuf {
    if let Some(dir) = std::env::var_os("PID_PARSE_SELF_CHECK_DIR") {
        return PathBuf::from(dir);
    }
    match backup.parent() {
        Some(package) if !pid_parse::refdata_archive::refdata_archives(package).is_empty() => {
            package.to_path_buf()
        }
        _ => share_dir(),
    }
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

fn relative_key(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The eleven reference sheets against `PID_ACCEPTANCE_CORPUS`.
///
/// # Errors
///
/// A message naming the drawing that did not lay out. An absent corpus is
/// `Ok(None)`.
pub fn reference_batch() -> Result<Option<Batch>, String> {
    let corpus = reference_corpus();
    if !corpus
        .join(format!("{}.pid", REFERENCE_SHEETS[0]))
        .is_file()
    {
        return Ok(None);
    }
    let mut sheets = Vec::new();
    for name in REFERENCE_SHEETS {
        let path = corpus.join(format!("{name}.pid"));
        let model =
            lay_out_drawing(&path, &corpus).map_err(|e| format!("{}: {e}", path.display()))?;
        sheets.push(Sheet {
            key: name.to_string(),
            path,
            model,
        });
    }
    Ok(Some(Batch {
        name: "reference",
        corpus,
        sheets,
    }))
}

/// The plant backup's drawings against the corpus `export_plant_backup`
/// reads.
///
/// # Errors
///
/// A message naming the drawing that did not lay out. A machine with no
/// extracted backup is `Ok(None)`.
pub fn backup_batch() -> Result<Option<Batch>, String> {
    let Some(backup) = backup_dir() else {
        return Ok(None);
    };
    let corpus = backup_corpus(&backup);
    let mut paths = Vec::new();
    collect_pids(&backup, &mut paths);
    let mut sheets = Vec::new();
    for path in paths {
        let model =
            lay_out_drawing(&path, &corpus).map_err(|e| format!("{}: {e}", path.display()))?;
        sheets.push(Sheet {
            key: relative_key(&path, &backup),
            path,
            model,
        });
    }
    Ok(Some(Batch {
        name: "backup",
        corpus,
        sheets,
    }))
}

/// The seven committed fixtures against the committed symbol sources
/// `paper_model_golden` states, so the pages are the ones its goldens pin.
///
/// # Errors
///
/// A message naming the fixture that did not lay out.
pub fn fixture_batch() -> Result<Batch, String> {
    let corpus = repo_root().join("test-file");
    let roots = vec![corpus.join("Ref").join("Symbols")];
    let archives = vec![
        corpus
            .join("backup-test")
            .join("DWG-0202GP06-01_p")
            .join("RefData~4~681.zip"),
    ];
    let label = vec![
        "test-file/Ref/Symbols".to_string(),
        "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip".to_string(),
    ];
    let mut sheets = Vec::new();
    for fixture in FIXTURES {
        let path = corpus.join(fixture);
        let model =
            lay_out_with_symbol_roots(&path, &corpus, roots.clone(), archives.clone(), &label)
                .map_err(|e| format!("{}: {e}", path.display()))?;
        sheets.push(Sheet {
            key: fixture.to_string(),
            path,
            model,
        });
    }
    Ok(Batch {
        name: "fixtures",
        corpus,
        sheets,
    })
}

/// Every batch this machine can lay out, reference sheets first; a batch
/// whose corpus is absent is named on stderr and left out.
///
/// # Errors
///
/// A message naming the drawing that did not lay out.
pub fn batches() -> Result<Vec<Batch>, String> {
    let mut out = Vec::new();
    match reference_batch()? {
        Some(batch) => out.push(batch),
        None => eprintln!(
            "the reference batch is left out: {} holds no {}.pid",
            reference_corpus().display(),
            REFERENCE_SHEETS[0]
        ),
    }
    match backup_batch()? {
        Some(batch) => out.push(batch),
        None => eprintln!("the backup batch is left out: no backup-plant-data directory is here"),
    }
    out.push(fixture_batch()?);
    Ok(out)
}

/// The leaf of a path spelled with either separator.
pub fn leaf(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}
