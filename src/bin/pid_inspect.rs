use pid_parse::{PidParser, PidWriter, WritePlan};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: pid_inspect <file.pid> [--json] [--schema]\n                    [--probe-cluster] [--probe-dynamic] [--probe-sheet]\n                    [--probe-relationships] [--probe-endpoints]\n                    [--crossref] [--graph-mermaid] [--crossref-mermaid]\n                    [--round-trip <output.pid> [--verify]]\n                    [--set-drawing-number <NEW> --output <output.pid>]\n                    [--set-xml-tag <stream> <tag> <value> --output <output.pid>]\n                    [--diff <other.pid>]"
        );
        std::process::exit(1);
    }

    let path = &args[1];
    let json_mode = args.iter().any(|a| a == "--json");
    let schema_mode = args.iter().any(|a| a == "--schema");
    let probe_cluster = args.iter().any(|a| a == "--probe-cluster");
    let probe_dynamic = args.iter().any(|a| a == "--probe-dynamic");
    let probe_sheet = args.iter().any(|a| a == "--probe-sheet");
    let probe_relationships = args.iter().any(|a| a == "--probe-relationships");
    let probe_endpoints = args.iter().any(|a| a == "--probe-endpoints");
    let crossref = args.iter().any(|a| a == "--crossref");
    let graph_mermaid = args.iter().any(|a| a == "--graph-mermaid");
    let crossref_mermaid = args.iter().any(|a| a == "--crossref-mermaid");

    let round_trip = flag_value(&args, "--round-trip");
    let set_drawing_number = flag_value(&args, "--set-drawing-number");
    let set_xml_tag_args = flag_triple(&args, "--set-xml-tag");
    let output = flag_value(&args, "--output");
    let diff_other = flag_value(&args, "--diff");
    let verify = args.iter().any(|a| a == "--verify");

    // Writer / diff modes are handled up-front because they don't print
    // the standard report and always exit after completing.
    if let Some(other) = diff_other {
        run_diff(path, &other);
        return;
    }
    if let Some(out) = round_trip {
        run_round_trip(path, &out, verify);
        return;
    }
    if let Some(new_number) = set_drawing_number {
        let Some(out) = output.clone() else {
            eprintln!("--set-drawing-number requires --output <file.pid>");
            std::process::exit(2);
        };
        run_set_drawing_number(path, &new_number, &out);
        return;
    }
    if let Some((stream, tag, value)) = set_xml_tag_args {
        let Some(out) = output else {
            eprintln!("--set-xml-tag requires --output <file.pid>");
            std::process::exit(2);
        };
        run_set_xml_tag(path, &stream, &tag, &value, &out);
        return;
    }

    if schema_mode {
        match pid_parse::schema::pid_document_schema_pretty() {
            Ok(s) => println!("{}", s),
            Err(e) => {
                eprintln!("Schema serialization error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    let parser = PidParser::new();
    // Use parse_package so the default report can surface container-level
    // CLSID metadata (root + non-root storages) captured since v0.3.2+.
    let pkg = match parser.parse_package(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };
    let doc = &pkg.parsed;

    if json_mode {
        match serde_json::to_string_pretty(doc) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("JSON serialization error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if probe_cluster {
        print_probe_cluster(doc);
    }

    if probe_dynamic {
        print_probe_dynamic(doc);
    }

    if probe_sheet {
        print_probe_sheet(doc);
    }

    if probe_relationships {
        print_probe_relationships(doc);
    }

    if probe_endpoints {
        print_probe_endpoints(doc);
    }

    if crossref {
        print_crossref(doc);
    }

    if graph_mermaid {
        let out = pid_parse::inspect::mermaid::object_graph_mermaid(doc);
        if out.is_empty() {
            eprintln!("(no object graph available — nothing to render)");
        } else {
            print!("{}", out);
        }
    }

    if crossref_mermaid {
        let out = pid_parse::inspect::mermaid::crossref_mermaid(doc);
        if out.is_empty() {
            eprintln!("(no cross-reference graph — nothing to render)");
        } else {
            print!("{}", out);
        }
    }

    if !probe_cluster
        && !probe_dynamic
        && !probe_sheet
        && !probe_relationships
        && !probe_endpoints
        && !crossref
        && !graph_mermaid
        && !crossref_mermaid
    {
        let report = pid_parse::inspect::report::generate_package_report(&pkg);
        print!("{}", report);
    }
}

/// Extract the value of a `--flag <value>` pair. Returns `None` when the
/// flag is absent; exits with a friendly error when the flag is present
/// but unterminated.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    match args.get(idx + 1) {
        Some(v) if !v.starts_with("--") => Some(v.clone()),
        _ => {
            eprintln!("{} requires a value", flag);
            std::process::exit(2);
        }
    }
}

/// Extract three consecutive positional values after `flag`. Used for
/// `--set-xml-tag <stream> <tag> <value>`.
fn flag_triple(args: &[String], flag: &str) -> Option<(String, String, String)> {
    let idx = args.iter().position(|a| a == flag)?;
    let fetch = |offset: usize, label: &str| -> String {
        match args.get(idx + offset) {
            Some(v) if !v.starts_with("--") => v.clone(),
            _ => {
                eprintln!("{} requires <{}> as argument #{}", flag, label, offset);
                std::process::exit(2);
            }
        }
    };
    Some((fetch(1, "stream"), fetch(2, "tag"), fetch(3, "value")))
}

/// Passthrough round-trip: re-serialize the package to a new CFB without
/// any plan changes. Proves the writer pipeline on the full fixture and
/// is useful as a diff baseline. When `verify` is true, the written file
/// is re-parsed and diffed against the source; the run exits with code 1
/// if any diffs are found.
fn run_round_trip(input: &str, output: &str, verify: bool) {
    let parser = PidParser::new();
    let pkg = match parser.parse_package(input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };
    if let Err(e) = PidWriter::write_to(&pkg, &WritePlan::default(), std::path::Path::new(output)) {
        eprintln!("Write error: {}", e);
        std::process::exit(1);
    }
    eprintln!("round-trip ok: {} -> {}", input, output);
    eprintln!("  streams written: {}", pkg.streams.len());
    if let Some(clsid) = pkg.root_clsid {
        eprintln!("  root CLSID preserved: {{{}}}", clsid);
    } else {
        eprintln!("  root CLSID: (none in source)");
    }

    if verify {
        let pkg_back = match parser.parse_package(output) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Verify parse error: {}", e);
                std::process::exit(1);
            }
        };
        let diff = pid_parse::diff_packages(&pkg, &pkg_back);
        if diff.is_empty() {
            eprintln!("  verified: 0 diffs");
        } else {
            eprintln!(
                "  verification FAILED: {} diff(s) — see report below",
                diff.diff_count()
            );
            print!("{}", pid_parse::inspect::diff::render(&diff));
            std::process::exit(1);
        }
    }
}

/// Print a byte-level diff between two `.pid` packages.
fn run_diff(a_path: &str, b_path: &str) {
    let parser = PidParser::new();
    let a = match parser.parse_package(a_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse A error: {}", e);
            std::process::exit(1);
        }
    };
    let b = match parser.parse_package(b_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse B error: {}", e);
            std::process::exit(1);
        }
    };
    let diff = pid_parse::diff_packages(&a, &b);
    eprintln!("A: {}", a_path);
    eprintln!("B: {}", b_path);
    print!("{}", pid_parse::inspect::diff::render(&diff));
    // Non-empty diff exits with non-zero to be CI-friendly.
    if !diff.is_empty() {
        std::process::exit(1);
    }
}

/// Rewrite the `<DrawingNumber>` element inside `/TaggedTxtData/Drawing`.
fn run_set_drawing_number(input: &str, new_number: &str, output: &str) {
    let old = perform_xml_tag_write(
        input,
        "/TaggedTxtData/Drawing",
        "DrawingNumber",
        new_number,
        output,
    );
    eprintln!(
        "set-drawing-number ok: DrawingNumber {:?} -> {:?}  ({} -> {})",
        old, new_number, input, output
    );
}

/// Replace the text of a simple `<tag>...</tag>` element inside the
/// provided `/TaggedTxtData/*` stream and write the result.
fn run_set_xml_tag(input: &str, stream: &str, tag: &str, value: &str, output: &str) {
    let old = perform_xml_tag_write(input, stream, tag, value, output);
    eprintln!(
        "set-xml-tag ok: {} <{}>: {:?} -> {:?}  ({} -> {})",
        stream, tag, old, value, input, output
    );
}

/// Shared implementation for `--set-drawing-number` and `--set-xml-tag`.
/// Returns the pre-edit text of the target tag so the caller can log it.
fn perform_xml_tag_write(
    input: &str,
    stream: &str,
    tag: &str,
    value: &str,
    output: &str,
) -> String {
    let parser = PidParser::new();
    let mut pkg = match parser.parse_package(input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };
    let old = match pkg.set_xml_tag(stream, tag, value) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("XML edit failed: {}", e);
            std::process::exit(1);
        }
    };
    if let Err(e) = PidWriter::write_to(&pkg, &WritePlan::default(), std::path::Path::new(output)) {
        eprintln!("Write error: {}", e);
        std::process::exit(1);
    }
    old
}

fn print_probe_endpoints(doc: &pid_parse::PidDocument) {
    println!("=== Relationship Endpoint Resolution ===\n");
    let Some(ref graph) = doc.object_graph else {
        println!("(no object graph available)");
        return;
    };
    if graph.relationships.is_empty() {
        println!("(no relationships in graph)");
        return;
    }

    let fully = graph
        .relationships
        .iter()
        .filter(|r| r.source_drawing_id.is_some() && r.target_drawing_id.is_some())
        .count();
    let partial = graph
        .relationships
        .iter()
        .filter(|r| r.source_drawing_id.is_some() ^ r.target_drawing_id.is_some())
        .count();
    let unresolved = graph.relationships.len() - fully - partial;

    let total_eps: usize = doc
        .sheet_streams
        .iter()
        .map(|s| s.endpoint_records.len())
        .sum();
    println!(
        "relationships = {}   sheet endpoint records = {}",
        graph.relationships.len(),
        total_eps
    );
    println!(
        "resolution   : {} fully / {} partial / {} unresolved\n",
        fully, partial, unresolved
    );

    let item_type_by_did: std::collections::HashMap<&str, &str> = graph
        .objects
        .iter()
        .map(|o| (o.drawing_id.as_str(), o.item_type.as_str()))
        .collect();
    let render = |did: Option<&str>| -> String {
        match did {
            Some(d) => {
                let ty = item_type_by_did.get(d).copied().unwrap_or("?");
                format!("{} [{}]", d, ty)
            }
            None => "(off-drawing)".to_string(),
        }
    };

    for (i, rel) in graph.relationships.iter().enumerate() {
        let id = if rel.guid.is_empty() {
            format!("(template rec={:?})", rel.record_id)
        } else {
            rel.guid.clone()
        };
        let src = render(rel.source_drawing_id.as_deref());
        let tgt = render(rel.target_drawing_id.as_deref());
        println!(
            "[{:>3}]  {}   field_x={:?}\n         {}  ->  {}",
            i, id, rel.field_x, src, tgt
        );
    }
}

fn print_crossref(doc: &pid_parse::PidDocument) {
    println!("=== Cross Reference ===\n");
    let Some(ref xr) = doc.cross_reference else {
        println!("(no cross-reference graph)");
        return;
    };

    println!("--- Cluster Coverage ---");
    let cov = &xr.cluster_coverage;
    println!("  declared: {:?}", cov.declared);
    println!("  found:    {:?}", cov.found);
    println!("  matched:  {:?}", cov.matched);
    if !cov.declared_missing.is_empty() {
        println!("  declared_missing: {:?}", cov.declared_missing);
    }
    if !cov.found_extra.is_empty() {
        println!("  found_extra: {:?}", cov.found_extra);
    }

    println!("\n--- Symbol Usage ({} unique) ---", xr.symbol_usage.len());
    for u in &xr.symbol_usage {
        println!(
            "  [{}x] {}",
            u.usage_count,
            u.symbol_name
                .clone()
                .unwrap_or_else(|| u.symbol_path.clone())
        );
        println!("      path: {}", u.symbol_path);
        println!("      jsites: {:?}", u.jsite_names);
    }

    println!(
        "\n--- Attribute Classes ({}) ---",
        xr.attribute_classes.len()
    );
    for c in &xr.attribute_classes {
        println!(
            "  {} (records={}, attr_names={}, drawings={}, models={})",
            c.class_name,
            c.record_count,
            c.unique_attribute_names.len(),
            c.drawing_ids.len(),
            c.model_ids.len()
        );
        if !c.drawing_ids.is_empty() {
            println!("    drawings: {:?}", c.drawing_ids);
        }
        if !c.model_ids.is_empty() {
            let preview: Vec<_> = c.model_ids.iter().take(8).cloned().collect();
            println!(
                "    models (first {}): {:?}",
                preview.len().min(c.model_ids.len()),
                preview
            );
        }
        if !c.unique_attribute_names.is_empty() {
            println!("    attr names: {:?}", c.unique_attribute_names);
        }
    }

    println!("\n--- Root Presence ---");
    for r in &xr.root_presence {
        let where_ = match (r.found_as_storage, r.found_as_stream) {
            (true, _) => "STORAGE",
            (_, true) => "STREAM",
            _ => "MISSING",
        };
        println!("  [{}] id=0x{:08X}  {}", where_, r.id, r.name);
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
            println!(
                "  header: magic=0x{:08X} type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.magic, hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            );
        } else {
            println!("  header: (not detected / wrong magic)");
        }
        if let Some(ref pi) = c.probe_info {
            println!(
                "  [PROBE] string_table_offset=0x{:04X} ({} decimal)",
                pi.string_table_offset, pi.string_table_offset
            );
            println!("  [PROBE] detection_method={}", pi.detection_method);
            println!("  [PROBE] entries_parsed={}", pi.entries_parsed);
            println!(
                "  [PROBE] end_offset=0x{:04X} ({} decimal)",
                pi.end_offset, pi.end_offset
            );
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

fn print_probe_sheet(doc: &pid_parse::PidDocument) {
    println!("=== Sheet Probe ===\n");
    if doc.sheet_streams.is_empty() {
        println!("(no sheet streams found)");
        return;
    }
    for sh in &doc.sheet_streams {
        println!("--- {} ---", sh.name);
        println!("  path: {}", sh.path);
        println!("  size: {} bytes (0x{:X})", sh.size, sh.size);
        if let Some(m) = sh.magic_u32_le {
            print!("  magic: 0x{:08X}", m);
            if let Some(ref tag) = sh.magic_tag {
                print!(" '{}'", tag);
            }
            println!();
        }
        if let Some(ref hdr) = sh.header {
            println!(
                "  header: magic=0x{:08X} type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.magic, hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            );
        } else {
            println!("  header: (not detected / wrong magic)");
        }
        if let Some(ref ps) = sh.probe_summary {
            println!(
                "  [PROBE] body_start_offset=0x{:04X} ({} decimal)",
                ps.body_start_offset, ps.body_start_offset
            );
            println!("  [PROBE] 0x89 markers found: {}", ps.marker_count);
            println!("  [PROBE] records extracted: {}", ps.records_extracted);
            println!(
                "  [PROBE] bytes scanned: {} / {} total",
                ps.bytes_scanned, sh.size
            );
        }
        if !sh.attribute_records.is_empty() {
            println!(
                "\n  records: {} [EXPERIMENTAL/heuristic]",
                sh.attribute_records.len()
            );
            for (i, rec) in sh.attribute_records.iter().enumerate() {
                println!(
                    "    [{}] class=\"{}\" attrs={} confidence={}",
                    i,
                    rec.class_name,
                    rec.attributes.len(),
                    rec.confidence
                );
                for attr in &rec.attributes {
                    println!("         {}: {:?}", attr.name, attr.value);
                }
            }
        }
        if !sh.extracted_texts.is_empty() {
            println!(
                "\n  ASCII preview ({} strings, first 10):",
                sh.extracted_texts.len()
            );
            for t in sh.extracted_texts.iter().take(10) {
                println!("    {}", t);
            }
        }
        println!();
    }
}

fn print_probe_relationships(doc: &pid_parse::PidDocument) {
    println!("=== Relationship Probe ===\n");
    println!("Scope note: this probe only inspects bytes adjacent to each");
    println!("  Relationship.<GUID> record inside /Unclustered Dynamic");
    println!("  Attributes. Endpoint (source/target) decoding is NOT performed");
    println!("  because the Relationship GUIDs occur nowhere else in the CFB");
    println!("  container (neither raw nor Windows GUID layouts).\n");

    let Some(ref da) = doc.dynamic_attributes else {
        println!("(no dynamic attributes stream found)");
        return;
    };
    if da.relationship_probes.is_empty() {
        println!("(no Relationship.<GUID> records detected in the stream)");
        return;
    }

    println!(
        "probed {} relationship records in {}\n",
        da.relationship_probes.len(),
        da.path
    );
    for (i, p) in da.relationship_probes.iter().enumerate() {
        println!(
            "[{:>3}] guid={} @0x{:06X}  window=[0x{:06X}..0x{:06X})",
            i, p.guid, p.ascii_offset, p.window_start, p.window_end
        );
        if p.nearby_ascii_guids.is_empty() {
            println!("      nearby GUIDs: (none)");
        } else {
            for (off, g) in &p.nearby_ascii_guids {
                let annotation = if *g == p.guid { " (this record)" } else { "" };
                println!("      nearby GUID @0x{:06X}  {}{}", off, g, annotation);
            }
        }
        if !p.trailing_tokens.is_empty() {
            let summary: Vec<String> = p
                .trailing_tokens
                .iter()
                .map(|t| format!("{}=0x{:04X}@0x{:06X}", t.label, t.value, t.offset))
                .collect();
            println!("      trailing tokens: {}", summary.join(", "));
        }
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
            println!(
                "header: type=0x{:04X} records={} body_len={} flags=0x{:04X}",
                hdr.stream_type, hdr.record_count, hdr.body_len, hdr.flags
            );
        }
        if let Some(ref ps) = da.probe_summary {
            println!(
                "\n[PROBE] body_start_offset=0x{:04X} ({} decimal)",
                ps.body_start_offset, ps.body_start_offset
            );
            println!("[PROBE] 0x89 markers found: {}", ps.marker_count);
            println!("[PROBE] records extracted: {}", ps.records_extracted);
            println!(
                "[PROBE] bytes scanned: {} / {} total",
                ps.bytes_scanned, da.size
            );
        }
        println!(
            "\nrecords: {} [EXPERIMENTAL/heuristic]",
            da.attribute_records.len()
        );
        for (i, rec) in da.attribute_records.iter().enumerate() {
            println!(
                "  [{}] class=\"{}\" attrs={} confidence={}",
                i,
                rec.class_name,
                rec.attributes.len(),
                rec.confidence
            );
            for attr in &rec.attributes {
                println!("       {}: {:?}", attr.name, attr.value);
            }
        }
    } else {
        println!("(no dynamic attributes stream found)");
    }
}
