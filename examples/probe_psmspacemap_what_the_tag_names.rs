//! What class does a `PSMspacemap` tag name?
//!
//! [`probe_psmspacemap_tag_and_span`] settled that the tag is a property of
//! the object a member points at, not of the reference: all 2415 targets in
//! the corpus carry exactly one tag, the same low ids carry the same tag in
//! four unrelated documents, and the value set is a fixed 14. What none of
//! that says is *which* class each value names.
//!
//! The obvious candidate is ruled out on the DLL side rather than here.
//! `radsrvitem.dll`'s type_code -> CLSID table resolves all 13 non-zero tags
//! to Intergraph GUIDs, but the RAD class registry (four copies of it, in
//! `jutil.dll` / `i2mnuctl.ocx` / `igrresource412.dll` / `jcntrls412.ocx`)
//! names **none** of them -- while it names 230 of the 400 codes in that
//! range, including every control this crate already decodes. Landing 13 for
//! 13 on the unnamed half is not what a type code does. See
//! `docs/analysis/2026-08-27-psmspacemap-tag-is-not-a-type-code.md`.
//!
//! So this probe asks the file instead, on three channels that are already
//! decoded and that carry names or counts of their own:
//!
//! 1. **`PSMroots`** is the one id -> name table in the document. Every root
//!    it names is an object in the top-level index space. If a root's id
//!    carries a tag, that tag has a name attached to it -- and if the same
//!    root name draws the same tag in all four documents, the name is the
//!    class.
//! 2. **`PSMclustertable`** names the document's clusters. "Which cluster
//!    holds this object" is intrinsic to the object, shared by every
//!    reference to it, and drawn from a small per-document set -- so it fits
//!    every observation the tag satisfies. The test is arithmetic: does the
//!    number of distinct tags track the number of clusters, document by
//!    document?
//! 3. **The tag's own reach.** A class should not care which storage it is
//!    in. If a tag appears in the top-level map and in the `JSite` maps
//!    alike, it is a document-independent property; if each storage has its
//!    own tags, it is allocated per storage.
//!
//! ```powershell
//! cargo run --example probe_psmspacemap_what_the_tag_names
//! ```

use std::collections::{BTreeMap, BTreeSet};

use pid_parse::model::PidDocument;
use pid_parse::PidParser;

const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];

const SEGMENT_SHIFT: u32 = 13;

fn leaf(fixture: &str) -> &str {
    fixture.rsplit('/').next().unwrap_or(fixture)
}

/// The storage a space-map member stream hangs under. Each one numbers its
/// objects independently, so an id only resolves inside its own.
fn container_of(path: &str) -> &str {
    match path.rfind("PSMspacemap") {
        Some(at) => &path[..at],
        None => path,
    }
}

/// The segment a member stream's name states: `swprintf_s(L"0x%.8x", n << 13)`.
fn segment_of(path: &str) -> Option<u32> {
    let leaf = path.rsplit(['/', '\\']).next()?;
    u32::from_str_radix(leaf.strip_prefix("0x")?, 16)
        .ok()
        .map(|address| address >> SEGMENT_SHIFT)
}

/// Every `(persist id, tag)` the document states, per container.
fn tags_by_container(doc: &PidDocument) -> BTreeMap<&str, BTreeMap<u32, u16>> {
    let mut out: BTreeMap<&str, BTreeMap<u32, u16>> = BTreeMap::new();
    for (path, map) in &doc.psm_space_maps {
        let seen = out.entry(container_of(path)).or_default();
        for entry in &map.entries {
            for member in entry.live_members() {
                seen.insert(member.value, member.tag);
            }
        }
    }
    out
}

/// Which persist ids carry an entry of their own, per container.
fn entries_by_container(doc: &PidDocument) -> BTreeMap<&str, BTreeSet<u32>> {
    let mut out: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for (path, map) in &doc.psm_space_maps {
        let Some(segment) = segment_of(path) else {
            continue;
        };
        let seen = out.entry(container_of(path)).or_default();
        for entry in &map.entries {
            seen.insert((segment << SEGMENT_SHIFT) | u32::from(entry.index));
        }
    }
    out
}

fn main() {
    let mut loaded: Vec<(&str, PidDocument)> = Vec::new();
    for fixture in FIXTURES {
        if !std::path::Path::new(fixture).exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        match PidParser::new().parse_file(fixture) {
            Ok(doc) => loaded.push((fixture, doc)),
            Err(e) => println!("skip: {fixture} did not parse: {e}"),
        }
    }
    if loaded.is_empty() {
        return;
    }

    roots_report(&loaded);
    cluster_report(&loaded);
    reach_report(&loaded);
}

/// Question 1: `PSMroots` is an id -> name table. What tag do the named
/// objects carry?
fn roots_report(loaded: &[(&str, PidDocument)]) {
    println!("=== 1. the tag on the objects PSMroots names ===");
    // root name -> the tags it draws, across documents.
    let mut tags_per_name: BTreeMap<String, BTreeMap<u16, usize>> = BTreeMap::new();
    let mut named_but_untagged: BTreeMap<String, usize> = BTreeMap::new();

    for (fixture, doc) in loaded {
        let Some(roots) = &doc.psm_roots else {
            println!("  {}: no PSMroots", leaf(fixture));
            continue;
        };
        let tags = tags_by_container(doc);
        let entries = entries_by_container(doc);
        let top_tags = tags.get("/").cloned().unwrap_or_default();
        let top_entries = entries.get("/").cloned().unwrap_or_default();

        println!(
            "  {} ({} roots, top-level map knows {} ids)",
            leaf(fixture),
            roots.entries.len(),
            top_tags.len()
        );
        for root in &roots.entries {
            let tag = top_tags.get(&root.id);
            let has_entry = top_entries.contains(&root.id);
            match tag {
                Some(tag) => {
                    *tags_per_name
                        .entry(root.name.clone())
                        .or_default()
                        .entry(*tag)
                        .or_default() += 1;
                }
                None => *named_but_untagged.entry(root.name.clone()).or_default() += 1,
            }
            println!(
                "     id {:>6} {:<28} tag {:<6} own entry: {}",
                root.id,
                root.name,
                tag.map(|t| t.to_string()).unwrap_or_else(|| "--".into()),
                if has_entry { "yes" } else { "no" }
            );
        }
    }

    println!("\n  -- the same root name across documents --");
    for (name, tags) in &tags_per_name {
        let verdict = if tags.len() == 1 {
            "agrees"
        } else {
            "DISAGREES"
        };
        println!("     {name:<28} {tags:?}  {verdict}");
    }
    if !named_but_untagged.is_empty() {
        println!("  -- named roots that nothing points at, so no tag --");
        for (name, hits) in &named_but_untagged {
            println!("     {name:<28} x{hits}");
        }
    }
}

/// Question 2: is the tag a cluster id? Then the tag count tracks the
/// cluster count, document by document.
fn cluster_report(loaded: &[(&str, PidDocument)]) {
    println!("\n=== 2. does the number of tags track the number of clusters? ===");
    println!(
        "  {:<26} {:>8} {:>9} {:>9}  the tags this document uses",
        "fixture", "clusters", "segments", "tags"
    );
    for (fixture, doc) in loaded {
        let clusters = doc
            .psm_cluster_table
            .as_ref()
            .map(|t| t.entries.len())
            .unwrap_or_default();
        let segments = doc
            .psm_segment_table
            .as_ref()
            .map(|t| t.entries.len())
            .unwrap_or_default();
        let mut tags: BTreeSet<u16> = BTreeSet::new();
        for map in doc.psm_space_maps.values() {
            for entry in &map.entries {
                for member in entry.live_members() {
                    tags.insert(member.tag);
                }
            }
        }
        println!(
            "  {:<26} {:>8} {:>9} {:>9}  {:?}",
            leaf(fixture),
            clusters,
            segments,
            tags.len(),
            tags
        );
    }
    if let Some((fixture, doc)) = loaded.first() {
        if let Some(table) = &doc.psm_cluster_table {
            println!("  cluster names in {}:", leaf(fixture));
            for entry in &table.entries {
                println!("     {}", entry.name);
            }
        }
    }
}

/// Question 3: does a tag belong to the document, or to the storage it is
/// used in? A class crosses storages; an allocated id does not.
///
/// `PSMroots` names the `JSite` storages too -- their numeric suffix is a
/// persist id in the top-level space -- so each storage can be labelled with
/// what the document calls it rather than with a number.
fn reach_report(loaded: &[(&str, PidDocument)]) {
    println!("\n=== 3. which storages use which tag ===");
    let mut kinds_per_tag: BTreeMap<u16, BTreeMap<String, usize>> = BTreeMap::new();
    let mut kinds_seen: BTreeMap<String, usize> = BTreeMap::new();

    for (_, doc) in loaded {
        let names: BTreeMap<u32, &str> = doc
            .psm_roots
            .as_ref()
            .map(|roots| {
                roots
                    .entries
                    .iter()
                    .map(|r| (r.id, r.name.as_str()))
                    .collect()
            })
            .unwrap_or_default();

        for (container, tags) in tags_by_container(doc) {
            // "/JSite329/" -> the name PSMroots gives object 329.
            let kind = container
                .split(['/', '\\'])
                .find_map(|part| part.strip_prefix("JSite"))
                .and_then(|n| n.parse::<u32>().ok())
                .and_then(|id| names.get(&id).copied())
                .unwrap_or("<top level>")
                .to_string();
            *kinds_seen.entry(kind.clone()).or_default() += 1;
            for tag in tags.values() {
                *kinds_per_tag
                    .entry(*tag)
                    .or_default()
                    .entry(kind.clone())
                    .or_default() += 1;
            }
        }
    }

    println!("  storages, by what the document calls them: {kinds_seen:?}");
    for (tag, kinds) in &kinds_per_tag {
        println!("  tag {tag:>3}: {kinds:?}");
    }
}
