use crate::error::PidError;
use crate::model::{EntryKind, StorageNode};
use std::io::{Read, Seek};

pub fn build_tree<R: Read + Seek>(
    cfb: &::cfb::CompoundFile<R>,
    path: &str,
) -> Result<StorageNode, PidError> {
    let entry = if path == "/" {
        cfb.root_entry()
    } else {
        cfb.entry(path)?
    };

    let kind = if entry.is_root() {
        EntryKind::Root
    } else if entry.is_storage() {
        EntryKind::Storage
    } else {
        EntryKind::Stream
    };

    let mut node = StorageNode {
        name: entry.name().to_string(),
        path: entry.path().to_string_lossy().replace('\\', "/"),
        kind,
        children: vec![],
    };

    if entry.is_storage() {
        for child in cfb.read_storage(path)? {
            let child_path = child.path().to_string_lossy().replace('\\', "/");
            node.children.push(build_tree(cfb, &child_path)?);
        }
    }

    Ok(node)
}
