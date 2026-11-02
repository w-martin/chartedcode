//! Finding the Python files to analyse.

use std::io;
use std::path::{Path, PathBuf};

use ignore::{DirEntry, WalkBuilder};

/// Directories that never hold project code, whatever the ignore files say.
const SKIPPED_DIRECTORIES: &[&str] = &["__pycache__", "node_modules", "site-packages"];

/// The Python files to analyse, and the directory their paths are reported relative to.
pub(crate) struct Sources {
    pub(crate) root_dir: PathBuf,
    pub(crate) files: Vec<PathBuf>,
}

fn is_skipped(entry: &DirEntry) -> bool {
    let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
    is_dir
        && (SKIPPED_DIRECTORIES.iter().any(|name| entry.file_name() == *name)
            // A virtual environment, wherever it is and whatever it is called.
            || entry.path().join("pyvenv.cfg").is_file())
}

/// The `.py` files under `root` (or `root` itself, if it is a file), in path order.
///
/// Hidden directories are skipped. Unless `respect_gitignore` is false, files matched by `.gitignore` and
/// `.ignore` files (including those in parent directories) are skipped; the user's global ignore file
/// is never read, so the same checkout gives the same result on every machine.
pub(crate) fn python_files(root: &Path, respect_gitignore: bool) -> io::Result<Sources> {
    let root = root.canonicalize()?;
    if root.is_file() {
        let root_dir = root
            .parent()
            .map_or_else(|| root.clone(), Path::to_path_buf);
        return Ok(Sources {
            root_dir,
            files: vec![root],
        });
    }
    let mut walk = WalkBuilder::new(&root);
    walk.hidden(true)
        .git_global(false)
        .git_exclude(false)
        .git_ignore(respect_gitignore)
        .ignore(respect_gitignore)
        .parents(respect_gitignore)
        .require_git(false)
        .filter_entry(|entry| !is_skipped(entry));
    let mut files: Vec<PathBuf> = walk
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter(|e| e.path().extension().is_some_and(|x| x == "py"))
        .map(DirEntry::into_path)
        .collect();
    files.sort();
    Ok(Sources {
        root_dir: root,
        files,
    })
}
