//! Ground truth: for every cached body layer in every corpus .pid, is the
//! layer displayed? Prints (name, oid) -> displayed histogram, plus how
//! many primitives sit on each layer.
use pid_parse::PidParser;
use std::collections::{BTreeMap, BTreeSet};

fn main() {
    let mut tally: BTreeMap<(String, Option<bool>), usize> = BTreeMap::new();
    let mut per_layer: BTreeMap<(String, Option<bool>), (usize, BTreeSet<u32>)> = BTreeMap::new();
    for path in std::env::args().skip(1) {
        let doc = PidParser::new()
            .parse_file(std::path::Path::new(&path))
            .expect("parses");
        let geom = pid_parse::geometry::build_normalized_geometry(&doc);
        for body in &geom.symbol_definitions {
            for layer in &body.sheet_layers {
                let name = layer.name.clone().unwrap_or_default();
                *tally.entry((name.clone(), layer.displayed)).or_default() += 1;
                let on_layer = body
                    .primitive_layers
                    .iter()
                    .filter(|l| **l == layer.oid)
                    .count();
                let entry = per_layer.entry((name, layer.displayed)).or_default();
                entry.0 += on_layer;
                entry.1.insert(layer.oid);
            }
        }
    }
    for ((name, shown), n) in &tally {
        println!("{n:5} bodies  shown={shown:?}  {name:?}");
    }
    println!();
    for ((name, shown), (prims, oids)) in &per_layer {
        println!(
            "{prims:5} prims across {} layers  shown={shown:?}  {name:?}",
            oids.len()
        );
    }
}
