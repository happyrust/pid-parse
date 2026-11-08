//! Whether a committed `PaperModel` golden is feasible: same bytes twice, and
//! how many bytes seven fixtures cost.
//!
//! The CI golden gate (D4) serializes a [`PaperModel`] per committed fixture and
//! compares it with a file under `tests/golden/paper/`. Two things decide
//! whether that is worth shipping: the serialization has to be reproducible
//! (no `HashMap` iteration order, no timestamp, no absolute path), and the
//! seven files together have to stay small enough to live in the repository.
//!
//! The layout is driven from **stated** symbol sources rather than discovered
//! ones, because a developer machine carries `test-file/symbols-full` (618
//! `.sym`, gitignored) and a clean checkout does not — a golden produced with
//! it would not reproduce on CI. By default the search is
//! `test-file/Ref/Symbols` (the 4 committed files) plus the backup package's
//! committed `RefData~4~681.zip` (616 `.sym`); `--bare` drops the archive.
//!
//! ```text
//! cargo run -q -p pid-plot --example probe_paper_model_golden_size
//! cargo run -q -p pid-plot --example probe_paper_model_golden_size -- --bare
//! ```

use std::path::Path;

use pid_plot::model::{PaperModel, lay_out_with_symbol_roots};

/// The committed fixtures, with the golden filename stem each one takes.
const FIXTURES: &[(&str, &str)] = &[
    ("D06.pid", "d06"),
    ("DWG-0201GP06-01.pid", "dwg-0201gp06-01"),
    ("DWG-0202GP06-01.pid", "dwg-0202gp06-01"),
    ("工艺管道及仪表流程-1.pid", "gongyi-guandao-1"),
    (
        "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        "jizhuzhan-flash-b",
    ),
    ("export-test/publish-data/A01/A01.pid", "publish-a01"),
    (
        "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
        "publish-dwg-0202gp06-01",
    ),
];

fn main() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pid-plot sits in the repository")
        .to_path_buf();
    let corpus = repo.join("test-file");
    let bare = std::env::args().any(|arg| arg == "--bare");
    let dump = std::env::args().any(|arg| arg == "--dump");

    let roots = vec![corpus.join("Ref").join("Symbols")];
    let archive = corpus
        .join("backup-test")
        .join("DWG-0202GP06-01_p")
        .join("RefData~4~681.zip");
    let mut archives = Vec::new();
    let mut label = vec!["test-file/Ref/Symbols".to_string()];
    if !bare && archive.is_file() {
        archives.push(archive);
        label.push("test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip".to_string());
    }

    println!("repo     {}", repo.display());
    println!("bare     {bare}");
    println!("label    {label:?}");

    let mut total = 0usize;
    let mut unstable = 0usize;
    for (path, slug) in FIXTURES {
        let fixture = corpus.join(path);
        // Two **separate layouts**, not one model rendered twice: a `PaperModel`
        // built once and serialized twice only proves the serializer is
        // deterministic. The question the golden gate depends on is whether
        // `lay_out_drawing` itself is — two independent walks over the same
        // `.pid`, which is what two runs of the test do.
        let mut laid_out = Vec::new();
        for _ in 0..2 {
            let model = lay_out_with_symbol_roots(
                &fixture,
                &corpus,
                roots.clone(),
                archives.clone(),
                &label,
            )
            .unwrap_or_else(|error| panic!("{path}: {error}"));
            laid_out.push(model);
        }
        let first = render(&laid_out[0], &repo);
        let second = render(&laid_out[1], &repo);
        let same = first == second;
        if !same {
            unstable += 1;
        }
        let model = &laid_out[0];
        total += first.len();
        println!(
            "{slug:<24} bytes={:<9} primitives={:<6} symbols={}/{} template={} same_twice={same}",
            first.len(),
            model.primitives.len(),
            model.symbols_found,
            model.symbols_referenced,
            model
                .template
                .as_ref()
                .map(|template| template.path.replace('\\', "/"))
                .unwrap_or_else(|| "<none>".to_string()),
        );
        if dump {
            std::fs::write(format!("target/readiness/paper-{slug}.json"), &first).ok();
        }
    }
    println!(
        "total bytes {total} ({:.2} MiB)",
        total as f64 / (1024.0 * 1024.0)
    );
    println!("unstable {unstable}");
    if unstable > 0 {
        std::process::exit(1);
    }
}

/// Canonical serialization: pretty JSON with a trailing newline, every path
/// normalized onto `<repo>`, every float quantized to a micrometre.
///
/// The quantization is what the golden gate needs to be portable: `sin_cos`,
/// `hypot` and `atan2` come from the platform's libm, so the same layout can
/// differ in the last bit between Windows and the Linux runner. See
/// `tests/paper_model_golden.rs`.
fn render(model: &PaperModel, repo: &Path) -> String {
    let mut value = serde_json::to_value(model).expect("PaperModel is serializable");
    let prefix = repo.display().to_string().replace('\\', "/");
    normalize(&mut value, &prefix);
    let mut json = serde_json::to_string_pretty(&value).expect("value is serializable");
    json.push('\n');
    json
}

/// How many decimals a coordinate keeps, in paper millimetres.
const GOLDEN_DECIMALS: usize = 6;

/// Unify separators, replace the repository root, and quantize every float.
fn normalize(value: &mut serde_json::Value, prefix: &str) {
    match value {
        serde_json::Value::String(text) => {
            let unified = text.replace('\\', "/");
            *text = unified.replace(prefix, "<repo>");
        }
        serde_json::Value::Array(items) => {
            for item in items {
                normalize(item, prefix);
            }
        }
        serde_json::Value::Object(map) => {
            for (_, item) in map.iter_mut() {
                normalize(item, prefix);
            }
        }
        serde_json::Value::Number(number) if number.is_f64() => {
            let Some(raw) = number.as_f64() else {
                return;
            };
            if !raw.is_finite() {
                return;
            }
            let spelled = format!("{raw:.GOLDEN_DECIMALS$}");
            let Ok(rounded) = spelled.parse::<f64>() else {
                return;
            };
            let rounded = if rounded == 0.0 { 0.0 } else { rounded };
            if let Some(quantized) = serde_json::Number::from_f64(rounded) {
                *value = serde_json::Value::Number(quantized);
            }
        }
        _ => {}
    }
}
