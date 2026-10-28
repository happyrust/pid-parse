//! What does each border template letter, where, and how big?
//!
//! The pin this answers for is `TEMPLATE_PLACEMENTS`' fifth member in
//! `tests/export_plant_backup.rs` — every cell a template's title block and
//! sign-off column carry, as `(characters, insertion point, body height)`. The
//! gate prints its own list per template on failure and per cell when one
//! differs; this probe is what regenerates them, and it is the only form that
//! shows a whole template's lettering at once.
//!
//! Layout-only, no PDF and no faces. Groups the plant backup's sheets by the
//! `.igr` their title block links and prints, for the first sheet wearing each
//! template, every furniture label sorted by text then position, in the shape
//! the pin is written in. Prints a disagreement line when a later sheet wearing
//! the same template letters something else, so a per-template pin is known to
//! hold before it is written — and a `!!!` there means the pin has to be per
//! sheet instead.
//!
//! Two of the seven templates only win the pick when the corpus is a backup
//! package, so their cells come from a second run:
//!
//! ```text
//! cargo run -p pid-plot --example probe_what_each_template_letters
//! PID_PARSE_SELF_CHECK_DIR=D:\work\cad\pid-test-data \
//!   cargo run -p pid-plot --example probe_what_each_template_letters
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

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

fn leaf_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

/// One cell a template letters: its characters, its insertion point and the
/// body height its record states.
type Cell = (String, [f64; 2], Option<f64>);

/// The first sheet wearing a template, and the cells that template letters.
type FirstSheet = (String, Vec<Cell>);

/// Every cell the template's furniture letters, sorted by text then position.
fn labels(model: &PaperModel) -> Vec<Cell> {
    let mut out: Vec<Cell> = model
        .primitives
        .iter()
        .filter(|primitive| primitive.furniture)
        .filter_map(|primitive| match &primitive.shape {
            PaperShape::Text {
                origin_mm,
                height_mm,
                value,
                ..
            } => Some((value.clone(), *origin_mm, *height_mm)),
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

fn main() {
    let corpus = corpus_dir();
    let Some(backup) = backup_dir() else {
        eprintln!("no backup-plant-data directory");
        std::process::exit(2);
    };
    let mut drawings = Vec::new();
    collect_pids(&backup, &mut drawings);
    eprintln!("{} drawings under {}", drawings.len(), backup.display());

    // First sheet wearing each template, and every sheet after it that
    // disagrees with the first.
    let mut first: BTreeMap<String, FirstSheet> = BTreeMap::new();
    let mut sheets: BTreeMap<String, usize> = BTreeMap::new();
    let mut disagreements: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in &drawings {
        let model = lay_out_drawing(path, &corpus)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let Some(template) = model.template.as_ref() else {
            continue;
        };
        let file = leaf_name(&template.path);
        let drawn = labels(&model);
        *sheets.entry(file.clone()).or_default() += 1;
        match first.get(&file) {
            None => {
                first.insert(file, (path.display().to_string(), drawn));
            }
            Some((_, want)) => {
                if want != &drawn {
                    disagreements
                        .entry(file)
                        .or_default()
                        .push(path.display().to_string());
                }
            }
        }
    }

    for (file, (sheet, drawn)) in &first {
        println!(
            "=== {file} — {} sheets, {} labels (from {sheet})",
            sheets[file],
            drawn.len()
        );
        for (value, origin, height) in drawn {
            println!(
                "    ({value:?}, [{:.6}, {:.6}], {}),",
                origin[0],
                origin[1],
                match height {
                    Some(height) => format!("{height:.6}"),
                    None => "f64::NAN".to_string(),
                }
            );
        }
    }
    for (file, sheets) in &disagreements {
        println!(
            "!!! {file}: {} sheet(s) letter something else:",
            sheets.len()
        );
        for sheet in sheets {
            println!("    {sheet}");
        }
    }
}
