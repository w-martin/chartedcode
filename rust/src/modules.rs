//! Module names, import roots and relative imports.

use std::path::{Path, PathBuf};

pub(crate) fn join(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else if b.is_empty() {
        a.to_string()
    } else {
        format!("{a}.{b}")
    }
}

pub(crate) fn relative_base(pkg: &str, level: u32, module: Option<&str>) -> Option<String> {
    if level == 0 {
        return module.map(str::to_string);
    }
    let segs: Vec<&str> = pkg.split('.').filter(|s| !s.is_empty()).collect();
    let up = (level - 1) as usize;
    if up > segs.len() {
        return None;
    }
    let base = segs[..segs.len() - up].join(".");
    Some(join(&base, module.unwrap_or("")))
}

// Import root = first ancestor directory that is not a package.
pub(crate) fn module_of(path: &Path) -> (PathBuf, String, bool) {
    let stem = path
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let is_init = stem == "__init__";
    let mut parts: Vec<String> = if is_init { vec![] } else { vec![stem] };
    let mut dir = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    while dir.join("__init__.py").exists() {
        parts.push(
            dir.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        );
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }
    parts.reverse();
    (dir, parts.join("."), is_init)
}
