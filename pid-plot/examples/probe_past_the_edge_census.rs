//! Every sheet's ink that sits outside its page, at four thresholds.
//!
//! The one overhang the 53-sheet gate pins is `N井场`'s sideways run, and where
//! it hangs depends on how the frame places its content: at the drawing's own
//! origin it starts 1.546 mm under the bottom edge, and from the frame's range
//! origin it starts 0.208 mm under it. The thresholds here show which slack
//! still sees it and which shapes sit exactly on the edge.
//!
//! ```text
//! cargo run -p pid-plot --example probe_past_the_edge_census -- "<backup dir>"
//! ```

use std::path::{Path, PathBuf};

use pid_plot::model::lay_out_drawing;

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pid"))
        {
            out.push(path);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::args().nth(1).expect("usage: <backup dir>"));
    let mut pids = Vec::new();
    collect(&root, &mut pids);
    pids.sort();
    println!("{} sheets", pids.len());
    for slack in [0.0f64, 0.05, 0.1, 0.5] {
        let mut count = 0usize;
        let mut names: Vec<String> = Vec::new();
        for pid in &pids {
            let Ok(model) = lay_out_drawing(pid, &root) else {
                continue;
            };
            for primitive in model.primitives_past_the_edge(slack) {
                count += 1;
                names.push(format!(
                    "{}: {:?}",
                    pid.file_name().unwrap_or_default().to_string_lossy(),
                    primitive.shape
                ));
            }
        }
        println!("slack {slack}: {count}");
        for name in names.iter().take(8) {
            println!("  {name}");
        }
    }
    Ok(())
}
