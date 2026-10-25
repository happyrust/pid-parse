//! Print every JSite's ole_links for the files named on the command line,
//! under the same options the importer runs (`ParseOptions::geometry()`).
use pid_parse::{ParseOptions, PidParser};

fn main() {
    for arg in std::env::args().skip(1) {
        let doc = match PidParser::with_options(ParseOptions::geometry()).parse_file(&arg) {
            Ok(doc) => doc,
            Err(e) => {
                println!("{arg}: parse failed: {e}");
                continue;
            }
        };
        println!("{arg}: {} jsite(s)", doc.jsites.len());
        for site in &doc.jsites {
            if site.has_ole_stream || !site.ole_links.is_empty() {
                println!(
                    "  {} ole_stream={} links={:?}",
                    site.name, site.has_ole_stream, site.ole_links
                );
            }
        }
    }
}
