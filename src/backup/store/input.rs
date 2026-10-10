//! What a Backup Store is built from: a Plant Backup handed over as
//! the `<Plant>_p.zip` `SmartPlant` wrote, or as the directory it
//! unpacks to.
//!
//! Both shapes give the same outer file list -- `Manifest.txt`,
//! `Export.dmp`, `PlantConfig.xml`, the `PlantData~2~*` and
//! `RefData~4~*` payloads -- and the same bytes for each file, except
//! that a directory may have been through a text-mode checkout
//! ([`BackupInput::note`]). A ZIP's entries are listed in central
//! directory order; a directory's files in byte order of their names,
//! so the same backup gives the same list on every machine.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::backup::zip_index::{zip_entry_of, ZipEntry, ZipNameEncoding};

use super::BackupStoreError;

/// The shape the Plant Backup arrived in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupInputKind {
    /// The `<Plant>_p.zip` `SmartPlant` wrote.
    Zip,
    /// The directory it unpacks to.
    Directory,
}

impl BackupInputKind {
    /// The label `store_info.input_kind` holds.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::Directory => "directory",
        }
    }
}

/// One outer file of the Plant Backup, as listed before its bytes are
/// read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputFile {
    /// Position in the ZIP's central directory, or in the name-sorted
    /// directory listing.
    pub entry_index: usize,
    /// The name as read; see [`InputFile::name_encoding`].
    pub name: String,
    /// The name's bytes as stored: the ZIP entry name, or the UTF-8
    /// of the directory entry's name.
    pub name_raw: Vec<u8>,
    /// How [`InputFile::name`] was read from [`InputFile::name_raw`].
    pub name_encoding: ZipNameEncoding,
    /// A ZIP directory entry; a directory input lists no directories.
    pub is_dir: bool,
    /// Size in bytes.
    pub size: u64,
}

/// The note a directory input carries into `store_info` (Q7): its
/// text files may not be byte-for-byte what `SmartPlant` wrote.
pub const DIRECTORY_INPUT_NOTE: &str = "directory input: text files may have had their line \
     endings changed on the way (a git checkout does); byte-level checks need the original \
     <Plant>_p.zip";

/// Where the input's bytes are read from.
enum Source {
    Zip(ZipArchive<Cursor<Vec<u8>>>),
    Directory(PathBuf),
}

/// A Plant Backup opened for reading: its kind, its outer files and
/// the SHA-256 that identifies the input.
pub struct BackupInput {
    kind: BackupInputKind,
    path: PathBuf,
    files: Vec<InputFile>,
    sha256: String,
    source: Source,
}

impl BackupInput {
    /// Opens `path`: a directory, or a file starting with the ZIP local
    /// file header magic. Anything else is refused.
    pub fn open(path: &Path) -> Result<Self, BackupStoreError> {
        let metadata = fs::metadata(path).map_err(|source| BackupStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if metadata.is_dir() {
            return Self::open_directory(path);
        }
        let bytes = fs::read(path).map_err(|source| BackupStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if !bytes.starts_with(b"PK\x03\x04") {
            return Err(BackupStoreError::UnrecognisedInput {
                path: path.to_path_buf(),
            });
        }
        let sha256 = sha256_hex(&bytes);
        let mut archive =
            ZipArchive::new(Cursor::new(bytes)).map_err(|source| BackupStoreError::Zip {
                archive: path.display().to_string(),
                source,
            })?;
        let mut files = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|source| BackupStoreError::Zip {
                    archive: path.display().to_string(),
                    source,
                })?;
            let entry = zip_entry_of(&entry);
            files.push(InputFile {
                entry_index: index,
                name: entry.name,
                name_raw: entry.name_raw,
                name_encoding: entry.name_encoding,
                is_dir: entry.is_dir,
                size: entry.size,
            });
        }
        Ok(Self {
            kind: BackupInputKind::Zip,
            path: path.to_path_buf(),
            files,
            sha256,
            source: Source::Zip(archive),
        })
    }

    /// The top-level files of `dir`, in byte order of their names;
    /// subdirectories (such as a derived `extracted/`) are not part of
    /// a Plant Backup and are left out. The input's SHA-256 is that of
    /// the listing `<sha256 of file>  <name>\n` over those files, since
    /// a directory has no bytes of its own.
    fn open_directory(dir: &Path) -> Result<Self, BackupStoreError> {
        let io_error = |source| BackupStoreError::Io {
            path: dir.to_path_buf(),
            source,
        };
        let mut names = Vec::new();
        for dirent in fs::read_dir(dir).map_err(io_error)? {
            let dirent = dirent.map_err(io_error)?;
            let metadata = dirent.metadata().map_err(io_error)?;
            if !metadata.is_file() {
                continue;
            }
            names.push((
                dirent.file_name().to_string_lossy().into_owned(),
                metadata.len(),
            ));
        }
        names.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));

        let mut listing = Sha256::new();
        let mut files = Vec::with_capacity(names.len());
        for (entry_index, (name, size)) in names.into_iter().enumerate() {
            let bytes = fs::read(dir.join(&name)).map_err(|source| BackupStoreError::Io {
                path: dir.join(&name),
                source,
            })?;
            listing.update(sha256_hex(&bytes).as_bytes());
            listing.update(b"  ");
            listing.update(name.as_bytes());
            listing.update(b"\n");
            let name_encoding = if name.is_ascii() {
                ZipNameEncoding::Ascii
            } else {
                ZipNameEncoding::Utf8
            };
            files.push(InputFile {
                entry_index,
                name_raw: name.as_bytes().to_vec(),
                name,
                name_encoding,
                is_dir: false,
                size,
            });
        }
        Ok(Self {
            kind: BackupInputKind::Directory,
            path: dir.to_path_buf(),
            files,
            sha256: hex(&listing.finalize()),
            source: Source::Directory(dir.to_path_buf()),
        })
    }

    /// The shape the input arrived in.
    pub fn kind(&self) -> BackupInputKind {
        self.kind
    }

    /// The path the input was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The outer files, in entry order.
    pub fn files(&self) -> &[InputFile] {
        &self.files
    }

    /// SHA-256 of the ZIP's bytes, or of a directory's file listing
    /// (see [`BackupInput::open`]).
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// The note to record for this input, if any.
    pub fn note(&self) -> Option<&'static str> {
        match self.kind {
            BackupInputKind::Zip => None,
            BackupInputKind::Directory => Some(DIRECTORY_INPUT_NOTE),
        }
    }

    /// The bytes of the outer file at `entry_index`; a ZIP directory
    /// entry reads as no bytes.
    pub fn read(&mut self, entry_index: usize) -> Result<Vec<u8>, BackupStoreError> {
        let file = self
            .files
            .get(entry_index)
            .ok_or_else(|| BackupStoreError::NoSuchEntry { entry_index })?;
        match &mut self.source {
            Source::Zip(archive) => {
                let mut entry =
                    archive
                        .by_index(entry_index)
                        .map_err(|source| BackupStoreError::Zip {
                            archive: self.path.display().to_string(),
                            source,
                        })?;
                let mut bytes = Vec::with_capacity(usize::try_from(file.size).unwrap_or(0));
                entry
                    .read_to_end(&mut bytes)
                    .map_err(|source| BackupStoreError::Io {
                        path: self.path.join(&file.name),
                        source,
                    })?;
                Ok(bytes)
            }
            Source::Directory(dir) => {
                let path = dir.join(&file.name);
                fs::read(&path).map_err(|source| BackupStoreError::Io { path, source })
            }
        }
    }
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(digest: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing into a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// An Option Archive (`PlantData~2~*.zip`, `RefData~4~*.zip`) held in
/// memory, read entry by entry in central directory order.
pub struct OptionArchive {
    name: String,
    archive: ZipArchive<Cursor<Vec<u8>>>,
}

impl OptionArchive {
    /// Opens the archive `bytes`, naming it `name` in errors.
    pub fn open(name: &str, bytes: Vec<u8>) -> Result<Self, BackupStoreError> {
        let archive =
            ZipArchive::new(Cursor::new(bytes)).map_err(|source| BackupStoreError::Zip {
                archive: name.to_string(),
                source,
            })?;
        Ok(Self {
            name: name.to_string(),
            archive,
        })
    }

    /// Entries in the archive.
    pub fn len(&self) -> usize {
        self.archive.len()
    }

    /// Whether the archive has no entries.
    pub fn is_empty(&self) -> bool {
        self.archive.is_empty()
    }

    /// The entry at `index` and its bytes (none for a directory entry).
    pub fn read(&mut self, index: usize) -> Result<(ZipEntry, Vec<u8>), BackupStoreError> {
        let mut entry = self
            .archive
            .by_index(index)
            .map_err(|source| BackupStoreError::Zip {
                archive: self.name.clone(),
                source,
            })?;
        let listed = zip_entry_of(&entry);
        let mut content = Vec::with_capacity(usize::try_from(listed.size).unwrap_or(0));
        if !listed.is_dir {
            entry
                .read_to_end(&mut content)
                .map_err(|source| BackupStoreError::Io {
                    path: PathBuf::from(&self.name).join(&listed.name),
                    source,
                })?;
        }
        Ok((listed, content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_is_lowercase_and_64_chars() {
        assert_eq!(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            sha256_hex(b"")
        );
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_refused() {
        let dir = std::env::temp_dir().join(format!(
            "pid_parse_store_input_{}_{}",
            std::process::id(),
            line!()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("not-a-zip.bin");
        fs::write(&path, b"TAPE but not a zip").unwrap();

        let err = BackupInput::open(&path).err().expect("refused");
        assert!(
            matches!(err, BackupStoreError::UnrecognisedInput { .. }),
            "{err}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_lists_its_top_level_files_in_name_order_and_skips_subdirectories() {
        let dir = std::env::temp_dir().join(format!(
            "pid_parse_store_input_{}_{}",
            std::process::id(),
            line!()
        ));
        fs::create_dir_all(dir.join("extracted")).unwrap();
        fs::write(dir.join("b.txt"), b"bb").unwrap();
        fs::write(dir.join("A.txt"), b"a").unwrap();
        fs::write(dir.join("extracted").join("ignored.txt"), b"x").unwrap();

        let mut input = BackupInput::open(&dir).expect("open directory");
        assert_eq!(BackupInputKind::Directory, input.kind());
        assert_eq!(
            vec![("A.txt", 1u64), ("b.txt", 2u64)],
            input
                .files()
                .iter()
                .map(|file| (file.name.as_str(), file.size))
                .collect::<Vec<_>>()
        );
        assert_eq!(Some(DIRECTORY_INPUT_NOTE), input.note());
        assert_eq!(b"bb".to_vec(), input.read(1).unwrap());

        // The listing hash: "<sha256(a)>  A.txt\n<sha256(bb)>  b.txt\n".
        let listing = format!(
            "{}  A.txt\n{}  b.txt\n",
            sha256_hex(b"a"),
            sha256_hex(b"bb")
        );
        assert_eq!(sha256_hex(listing.as_bytes()), input.sha256());
        let _ = fs::remove_dir_all(&dir);
    }
}
