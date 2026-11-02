//! Which files are analysed: ignore files, hidden directories, virtual environments, a single file.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use chartedcode_core::{analyze, Error, Options};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch directory");
    dir
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("has a parent")).expect("create directories");
    fs::write(path, text).expect("write file");
}

fn project(name: &str) -> PathBuf {
    let root = scratch(name);
    write(&root, ".gitignore", "ignored.py\nbuild_out/\n");
    write(&root, "a.py", "def top():\n    pass\n");
    write(&root, "ignored.py", "def by_gitignore():\n    pass\n");
    write(&root, "build_out/x.py", "def in_ignored_dir():\n    pass\n");
    write(&root, ".hidden/h.py", "def in_hidden_dir():\n    pass\n");
    write(&root, "venv_like/pyvenv.cfg", "home = /usr/bin\n");
    write(
        &root,
        "venv_like/lib/v.py",
        "def in_virtualenv():\n    pass\n",
    );
    write(
        &root,
        "node_modules/n.py",
        "def in_node_modules():\n    pass\n",
    );
    write(&root, "__pycache__/c.py", "def in_pycache():\n    pass\n");
    write(&root, "sub/b.py", "def nested():\n    pass\n");
    root
}

fn function_names(root: &Path, respect_gitignore: bool) -> BTreeSet<String> {
    analyze(
        root,
        &Options {
            respect_gitignore,
            ..Options::default()
        },
    )
    .expect("project analyses")
    .functions
    .into_iter()
    .map(|f| f.name)
    .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(ToString::to_string).collect()
}

#[test]
fn gitignore_hidden_directories_and_virtualenvs_are_skipped() {
    let root = project("discover_gitignore");
    assert_eq!(function_names(&root, true), set(&["top", "nested"]));
}

#[test]
fn turning_gitignore_off_still_skips_hidden_directories_and_virtualenvs() {
    let root = project("discover_no_gitignore");
    assert_eq!(
        function_names(&root, false),
        set(&["top", "nested", "by_gitignore", "in_ignored_dir"])
    );
}

#[test]
fn a_single_file_is_analysed_on_its_own() {
    let root = project("discover_single_file");
    let analysis = analyze(&root.join("sub/b.py"), &Options::default()).expect("file analyses");
    assert_eq!(analysis.files, 1);
    assert_eq!(analysis.functions[0].file, "b");
}

#[test]
fn an_empty_directory_and_a_missing_path_are_errors() {
    let empty = scratch("discover_empty");
    assert!(matches!(
        analyze(&empty, &Options::default()),
        Err(Error::NoPythonFiles(_))
    ));
    let message = analyze(&empty.join("missing"), &Options::default())
        .expect_err("missing path")
        .to_string();
    assert!(message.starts_with("cannot read"), "{message}");
}

#[test]
fn files_that_are_not_valid_python_are_listed_not_fatal() {
    let root = scratch("discover_invalid");
    write(&root, "ok.py", "def fine():\n    pass\n");
    write(&root, "broken.py", "def broken(:\n");
    let analysis = analyze(&root, &Options::default()).expect("analyses");
    assert_eq!(analysis.parse_failures, ["broken"]);
    assert_eq!(analysis.functions.len(), 1);
}

#[test]
fn the_same_input_gives_byte_identical_json() {
    let root = project("discover_determinism");
    let first = serde_json::to_string(&analyze(&root, &Options::default()).expect("analyses"))
        .expect("serialises");
    for _ in 0..5 {
        let again = serde_json::to_string(&analyze(&root, &Options::default()).expect("analyses"))
            .expect("serialises");
        assert_eq!(first, again);
    }
}
