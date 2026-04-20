//! Re-emit a [`PidPackage`] as a CFB file on disk.
//!
//! First-version semantics:
//! - Brand-new container created via `::cfb::create`; original CLSID,
//!   storage timestamps, and sector size hints are **not** copied.
//! - All intermediate storage paths are auto-created in lexicographic
//!   order before any stream is written, so deeply nested streams like
//!   `/A/B/C/D` work without the caller pre-declaring parents.
//! - Streams are written in `BTreeMap` key order for reproducibility — the
//!   physical layout will not match the source file byte-for-byte even on
//!   passthrough, but the *content view* (every stream's bytes accessible
//!   by path) is preserved.

use crate::error::PidError;
use crate::package::PidPackage;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

/// Write `package` to `output`. Overwrites any existing file at the path.
pub fn write_package(package: &PidPackage, output: &Path) -> Result<(), PidError> {
    // `::cfb::create` opens-or-truncates the destination path itself; we
    // don't pre-create the file (cfb-0.10's `create` takes a path, not a
    // `File` handle).
    let mut cfb = ::cfb::create(output)?;

    // 1. Materialize every parent storage path. We sort ascending so a
    //    parent like "/A" is always created before "/A/B".
    for storage in collect_storage_paths(package).into_iter() {
        // `::cfb::create` already provides the root storage; skip "/".
        if storage == "/" || storage.is_empty() {
            continue;
        }
        cfb.create_storage(&storage)?;
    }

    // 2. Write every stream. BTreeMap iteration is sorted by key.
    for raw in package.streams.values() {
        let mut stream = cfb.create_stream(&raw.path)?;
        stream.write_all(&raw.data)?;
    }

    // 3. cfb 0.10 flushes on drop, but explicit flush makes write errors
    //    surface here rather than at end-of-scope.
    cfb.flush()?;
    Ok(())
}

/// Walk every stream path and collect the unique set of parent storage
/// paths, sorted ascending so callers can create them top-down without
/// hitting "parent missing" errors.
fn collect_storage_paths(package: &PidPackage) -> Vec<String> {
    let mut storages: BTreeSet<String> = BTreeSet::new();
    for path in package.streams.keys() {
        // Split forward-slash path into parent components and accumulate
        // every prefix. `/A/B/C` → "/A", "/A/B".
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.len() <= 1 {
            continue;
        }
        let mut current = String::new();
        for component in &parts[..parts.len() - 1] {
            current.push('/');
            current.push_str(component);
            storages.insert(current.clone());
        }
    }
    storages.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{PidPackage, RawStream};
    use std::collections::BTreeMap;

    fn pkg_with_paths(paths: &[&str]) -> PidPackage {
        let mut streams = BTreeMap::new();
        for p in paths {
            streams.insert(
                (*p).to_string(),
                RawStream {
                    path: (*p).to_string(),
                    data: vec![],
                    modified: false,
                },
            );
        }
        PidPackage {
            source_path: None,
            streams,
            parsed: Default::default(),
        }
    }

    #[test]
    fn collects_parent_storages_topdown() {
        let pkg = pkg_with_paths(&["/A/B/C", "/A/D", "/X"]);
        assert_eq!(
            collect_storage_paths(&pkg),
            vec!["/A".to_string(), "/A/B".to_string()]
        );
    }

    #[test]
    fn root_streams_yield_no_storages() {
        let pkg = pkg_with_paths(&["/Foo", "/Bar"]);
        assert!(collect_storage_paths(&pkg).is_empty());
    }
}
