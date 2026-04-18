use pid_parse::PidParser;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: pid_inspect <file.pid> [--json] [--probe-cluster] [--probe-dynamic] [--probe-sheet] [--probe-relationships] [--probe-endpoints] [--crossref] [--graph-mermaid] [--crossref-mermaid]"
        );
        std::process::exit(1);
    }

    let path = &args[1];
    let json_mode = args.iter().any(|a| a == "--json");
    let probe_cluster = args.iter().any(|a| a == "--probe-cluster");
    let probe_dynamic = args.iter().any(|a| a == "--probe-dynamic");
    let probe_sheet = args.iter().any(|a| a == "--probe-sheet");
    let probe_relationships = args.iter().any(|a| a == "--probe-relationships");
    let probe_endpoints = args.iter().any(|a| a == "--probe-endpoints");
    let crossref = args.iter().any(|a| a == "--crossref");
    let graph_mermaid = args.iter().any(|a| a == "--graph-mermaid");
    let crossref_mermaid = args.iter().any(|a| a == "--crossref-mermaid");

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

    if probe_sheet {
        print_probe_sheet(&doc);
    }

    if probe_relationships {
        print_probe_relationships(&doc);
    }

    if probe_endpoints {
        print_probe_endpoints(&doc);
    }

    if crossref {
        print_crossref(&doc);
    }

    if graph_mermaid {
        let out = pid_parse::inspect::mermaid::object_graph_mermaid(&doc);
        if out.is_empty() {
            eprintln!("(no object graph available — nothing to render)");
        } else {
            print!("{}", out);
        }
    }

    if crossref_mermaid {
        let out = pid_parse::inspect::mermaid::crossref_mermaid(&doc);
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
        let report = pid_parse::inspect::report::generate_report(&doc);
        print!("{}", report);
    }
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
            u.symbol_name.clone().unwrap_or_else(|| u.symbol_path.clone())
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
        println!(
            "  [{}] id=0x{:08X}  {}",
            where_, r.id, r.name
        );
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
