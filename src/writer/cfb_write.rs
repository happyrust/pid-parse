//! Serialize a [`PidPackage`] back to a new CFB container.
//!
//! The writer is intentionally minimal: we rebuild the container from
//! scratch via [`cfb::create`] / [`cfb::CompoundFile::create`] and write
//! every stream in [`PidPackage::streams`] (deterministic `BTreeMap` order).
//!
//! Trade-offs that are **not** addressed in v0.3.2:
//!
//! - The original root CLSID / creation + modified timestamps are **not**
//!   preserved. Any SPPID host that depends on them will see a "fresh"
//!   container.
//! - Stream directory order differs from the source because we serialize
//!   in lexicographic path order, not CFB directory-sector order.
//! - Stream-level CLSIDs / state flags / colors are not preserved.
use crate::error::PidError;
use crate::package::PidPackage;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::Path;

/// Write `package` into an arbitrary seekable Read+Write sink, returning
/// the sink (so callers can `into_inner()` a `Cursor<Vec<u8>>` etc.).
///
/// Phase 9o (v0.5.3+): this is the generic backend shared by
/// [`write_package`] (file path sink) and
/// [`crate::writer::PidWriter::write_to_bytes`] (in-memory sink). The
/// sink must be initially empty (otherwise `cfb::CompoundFile::create`
/// returns an error).
pub fn write_package_to_writer<F: Read + Write + Seek>(
    package: &PidPackage,
    sink: F,
) -> Result<F, PidError> {
    let mut cfb = ::cfb::CompoundFile::create(sink)?;

    // 1. Create every intermediate storage (directory) required by the
    //    stream paths. `create_storage_all` handles nested paths in one
    //    call; sorting by path keeps the call order deterministic.
    let storages = collect_storage_paths(package);
    for dir in &storages {
        // Skip the implicit root.
        if dir == "/" {
            continue;
        }
        cfb.create_storage_all(dir)?;
    }

    // 2. Write every stream. BTreeMap iteration is ascending by key, which
    //    also gives reproducible output.
    for (path, raw) in &package.streams {
        let mut stream = cfb.create_stream(path)?;
        stream.write_all(&raw.data)?;
    }

    // 3. Restore the root CLSID if the package carried one. `cfb` 0.10
    //    defaults the root CLSID to the nil UUID when we call `create`,
    //    which loses SPPID host identity; forwarding the original CLSID
    //    is the one piece of container identity we *can* preserve on this
    //    crate version.
    if let Some(clsid) = package.root_clsid {
        cfb.set_storage_clsid("/", clsid)?;
    }

    // 4. Restore non-root storage CLSIDs. Real SmartPlant samples have
    //    a handful of non-nil values (see Phase 9e) so this map is
    //    usually small but non-empty.
    for (path, clsid) in &package.storage_clsids {
        cfb.set_storage_clsid(path, *clsid)?;
    }

    // 5. Restore storage timestamps (v0.3.13+, cfb 0.14 upstream APIs).
    //    Streams don't carry their own timestamps per CFB spec; the map
    //    only has entries for storages. Note `set_modified_time` /
    //    `set_created_time` are no-ops on streams in the upstream crate,
    //    but we only store storage-level timestamps in the first place.
    for (path, ts) in &package.storage_timestamps {
        if let Some(created) = ts.created {
            cfb.set_created_time(path, created)?;
        }
        if let Some(modified) = ts.modified {
            cfb.set_modified_time(path, modified)?;
        }
    }

    // 6. Restore non-zero state_bits (v0.3.13+). The map is sparse —
    //    zero is the CFB default and is omitted at parse time.
    for (path, bits) in &package.state_bits {
        cfb.set_state_bits(path, *bits)?;
    }

    cfb.flush()?;
    Ok(cfb.into_inner())
}

/// Write `package` to a new CFB file at `output`. Overwrites existing files.
pub fn write_package(package: &PidPackage, output: &Path) -> Result<(), PidError> {
    let file = File::create(output)?;
    write_package_to_writer(package, file).map(|_| ())
}

/// Extract the unique set of storage (directory) paths needed to host every
/// stream in the package. E.g. `["/TaggedTxtData/Drawing", "/JSite0/Ole"]`
/// yields `["/", "/JSite0", "/TaggedTxtData"]`.
pub(crate) fn collect_storage_paths(package: &PidPackage) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    set.insert("/".to_string());
    for path in package.streams.keys() {
        // Strip trailing segment (the stream name) and walk up.
        let mut current = path.as_str();
        while let Some(idx) = current.rfind('/') {
            let parent = if idx == 0 { "/" } else { &current[..idx] };
            if !set.insert(parent.to_string()) {
                // Already visited this ancestor; further ancestors are
                // already in the set from an earlier path.
                break;
            }
            current = parent;
        }
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PidDocument;
    use crate::package::RawStream;
    use std::collections::BTreeMap;

    fn pkg_from(paths: &[&str]) -> PidPackage {
        let mut map = BTreeMap::new();
        for p in paths {
            map.insert(
                p.to_string(),
                RawStream {
                    path: p.to_string(),
                    data: vec![],
                    modified: false,
                },
            );
        }
        PidPackage::new(None, map, PidDocument::default())
    }

    #[test]
    fn collect_storage_paths_handles_nested_and_root_streams() {
        let pkg = pkg_from(&[
            "/TopLevel",
            "/TaggedTxtData/Drawing",
            "/JSite0/Ole",
            "/JSite0/JProperties",
        ]);
        let dirs = collect_storage_paths(&pkg);
        assert!(dirs.contains(&"/".to_string()));
        assert!(dirs.contains(&"/TaggedTxtData".to_string()));
        assert!(dirs.contains(&"/JSite0".to_string()));
        assert!(!dirs.contains(&"/TopLevel".to_string()));
    }

    #[test]
    fn collect_storage_paths_skips_empty_package() {
        let pkg = pkg_from(&[]);
        let dirs = collect_storage_paths(&pkg);
        assert_eq!(dirs, vec!["/".to_string()]);
    }
}
