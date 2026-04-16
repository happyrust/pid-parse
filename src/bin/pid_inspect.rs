use pid_parse::PidParser;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: pid_inspect <file.pid> [--json] [--probe-cluster] [--probe-dynamic]");
        std::process::exit(1);
    }

    let path = &args[1];
    let json_mode = args.iter().any(|a| a == "--json");
    let probe_cluster = args.iter().any(|a| a == "--probe-cluster");
    let probe_dynamic = args.iter().any(|a| a == "--probe-dynamic");

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
        return;
    }

    if probe_cluster {
        print_probe_cluster(&doc);
    }

    if probe_dynamic {
        print_probe_dynamic(&doc);
    }

    if !probe_cluster && !probe_dynamic {
        let report = pid_parse::inspect::report::generate_report(&doc);
        print!("{}", report);
    }
}

fn print_probe_cluster(doc: &pid_parse::PidDocument) {
    println!("=== Cluster Probe ===\n");
    for c in &doc.clusters {
        println!("--- {} ---", c.name);
        println!("  path: {}", c.path);
        println!("  size: {} bytes (0x{:X})", c.size, c.size);
        if let Some(m) = c.magic_u32_le {
            println!("  magic: 0x{:08X}", m);
        }
        if let Some(ref hdr) = c.header {
            println!("  header: magic=0x{:08X} type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.magic, hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags);
        } else {
            println!("  header: (not detected / wrong magic)");
        }
        if let Some(ref pi) = c.probe_info {
            println!("  [PROBE] string_table_offset=0x{:04X} ({} decimal)", pi.string_table_offset, pi.string_table_offset);
            println!("  [PROBE] detection_method={}", pi.detection_method);
            println!("  [PROBE] entries_parsed={}", pi.entries_parsed);
            println!("  [PROBE] end_offset=0x{:04X} ({} decimal)", pi.end_offset, pi.end_offset);
        }
        if let Some(ref table) = c.string_table {
            println!("  string_table: {} entries", table.len());
            for entry in table {
                println!("    [{:>4}] \"{}\"", entry.index, entry.value);
            }
        }
        println!();
    }
}

fn print_probe_dynamic(doc: &pid_parse::PidDocument) {
    println!("=== Dynamic Attributes Probe ===\n");
    if let Some(ref da) = doc.dynamic_attributes {
        println!("path: {}", da.path);
        println!("size: {} bytes (0x{:X})", da.size, da.size);
        if let Some(m) = da.magic_u32_le {
            println!("magic: 0x{:08X}", m);
        }
        if let Some(ref hdr) = da.header {
            println!("header: type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags);
        }
        if let Some(ref ps) = da.probe_summary {
            println!("\n[PROBE] body_start_offset=0x{:04X} ({} decimal)", ps.body_start_offset, ps.body_start_offset);
            println!("[PROBE] 0x89 markers found: {}", ps.marker_count);
            println!("[PROBE] records extracted: {}", ps.records_extracted);
            println!("[PROBE] bytes scanned: {} / {} total", ps.bytes_scanned, da.size);
        }
        println!("\nrecords: {} [EXPERIMENTAL/heuristic]", da.attribute_records.len());
        for (i, rec) in da.attribute_records.iter().enumerate() {
            println!("  [{}] class=\"{}\" attrs={} confidence={}",
                i, rec.class_name, rec.attributes.len(), rec.confidence);
            for attr in &rec.attributes {
                println!("       {}: {:?}", attr.name, attr.value);
            }
        }
    } else {
        println!("(no dynamic attributes stream found)");
    }
}
