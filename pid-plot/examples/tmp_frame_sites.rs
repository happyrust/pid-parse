use std::path::PathBuf;

use pid_parse::PidParser;

fn main() {
    for arg in std::env::args().skip(1) {
        let path = PathBuf::from(&arg);
        let name: String = path
            .file_stem()
            .map(|s| s.to_string_lossy().chars().take(30).collect())
            .unwrap_or_default();
        let doc = PidParser::new().parse_file(&path).expect("parse");
        println!("=== {name}  ({} jsites)", doc.jsites.len());
        for site in doc
            .jsites
            .iter()
            .filter(|s| !s.ole_links.is_empty() || s.has_ole_stream)
        {
            let streams: Vec<String> = site
                .raw_streams
                .iter()
                .map(|s| format!("{}={}", s.name.escape_default(), s.size))
                .collect();
            let nested = site.nested_geometry.as_ref().map(|g| {
                (
                    g.lines.len(),
                    g.texts.len(),
                    g.circles.len(),
                    g.arcs.len(),
                    g.polylines.len(),
                )
            });
            println!(
                "  {} sym={:?} ole={} links={:?}\n     streams [{}]\n     nested(lines,texts,circles,arcs,polys)={:?}",
                site.path,
                site.symbol_name,
                site.has_ole_stream,
                site.ole_links,
                streams.join(", "),
                nested
            );
        }
    }
}
