use pid_parse::PidParser;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: pid_inspect <file.pid> [--json]");
        std::process::exit(1);
    }

    let path = &args[1];
    let json_mode = args.iter().any(|a| a == "--json");

    let parser = PidParser::new();
    let doc = match parser.parse_file(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

    if json_mode {
        match serde_json::to_string_pretty(&doc) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("JSON serialization error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        let report = pid_parse::inspect::report::generate_report(&doc);
        print!("{}", report);
    }
}
