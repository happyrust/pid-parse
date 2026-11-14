use std::path::{Path, PathBuf};

use pid_parse::PidParser;
use pid_plot::model::lay_out_drawing;

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "pid") {
            out.push(path);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let corpus = PathBuf::from(&args[0]);
    let mut files = Vec::new();
    for arg in &args[1..] {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            collect(&path, &mut files);
        } else {
            files.push(path);
        }
    }
    for path in files {
        let name: String = path
            .file_stem()
            .map(|s| s.to_string_lossy().chars().take(28).collect())
            .unwrap_or_default();
        let doc = match PidParser::new().parse_file(&path) {
            Ok(doc) => doc,
            Err(error) => {
                println!("ERR {name}: {error}");
                continue;
            }
        };
        let links: Vec<String> = doc
            .jsites
            .iter()
            .flat_map(|site| site.ole_links.iter().cloned())
            .collect();
        let geometry = pid_parse::geometry::build_normalized_geometry(&doc);
        let stated = geometry.page_dimensions_mm;
        let model = lay_out_drawing(&path, &corpus);
        let picked = match &model {
            Ok(model) => model.template.as_ref().map_or_else(
                || "-".to_string(),
                |t| t.path.rsplit(['\\', '/']).next().unwrap_or("").to_string(),
            ),
            Err(error) => format!("ERR {error}"),
        };
        println!("{name:<28} stated {stated:?} links {links:?} -> {picked}");
    }
}
