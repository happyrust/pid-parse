//! A directory a test may fill, gone when the test lets go of it.
//!
//! A test that needs files of its own — a corpus with nothing in it, a
//! baseline to rewrite, a damaged template — makes them under the temporary
//! directory. Left there, they pile up run after run on whichever drive holds
//! it, and a directory with a fixed name is shared by every test and every
//! checkout that uses the name. [`ScratchDir`] is a directory of one test's
//! own that is removed when the guard is dropped: at the end of the test, or
//! while a failed assertion unwinds through it.
//!
//! This file is compiled into the library's unit tests and, by path, into the
//! integration tests that need a directory, so there is one such guard.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A directory under the temporary directory that is removed with this guard.
///
/// It stands where a path is taken: `&dir` is the directory, and
/// `dir.join(..)` a path inside it.
pub(crate) struct ScratchDir(PathBuf);

impl ScratchDir {
    /// An empty directory named `pid-plot-<name>-<process id>-<count>`.
    ///
    /// The process id and the count of directories this process has made keep
    /// it the caller's alone: neither another run of the tests sharing the
    /// temporary directory nor another test of this run reads or replaces
    /// what is in it. `name` only says whose it is.
    pub(crate) fn new(name: &str) -> Self {
        static MADE: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "pid-plot-{name}-{}-{}",
            std::process::id(),
            MADE.fetch_add(1, Ordering::Relaxed)
        ));
        // A process id comes round again, and a run that was killed left its
        // directories behind.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)
            .unwrap_or_else(|error| panic!("mkdir {}: {error}", dir.display()));
        Self(dir)
    }
}

impl std::ops::Deref for ScratchDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for ScratchDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // Nothing rests on the removal: a directory a locked file keeps
        // behind is litter in the temporary directory, not a failed test.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
