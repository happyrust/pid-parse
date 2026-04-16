use pid_parse::PidParser;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(v) => v,
        None => return,
    };

    let parser = PidParser::new();
    let result = parser.parse_file(&path);

    match result {
        Ok(doc) => {
            println!("{}", doc.streams.len());
            println!("{}", doc.jsites.len());
            println!("{}", doc.clusters.len());
        }
        Err(_) => {}
    }
}
