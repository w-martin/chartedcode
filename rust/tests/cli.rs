//! The command line, run in-process.

use std::fs;
use std::path::PathBuf;

use chartedcode_core::cli::run_captured;
use serde_json::Value;

fn manifest(relative: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

fn scratch_file(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let path = dir.join(name);
    let _ = fs::remove_file(&path);
    path
}

fn run(args: &[&str]) -> (u8, String, String) {
    let argv = std::iter::once("chartedcode")
        .chain(args.iter().copied())
        .map(String::from)
        .collect();
    let (status, out, err) = run_captured(argv);
    (
        status,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

#[test]
fn version_is_the_crates_version() {
    let (status, out, _) = run(&["--version"]);
    assert_eq!(status, 0);
    assert_eq!(
        out.trim(),
        format!("chartedcode {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn a_usage_error_exits_2_and_says_what_is_wrong() {
    let (status, out, err) = run(&["nonsense"]);
    assert_eq!(status, 2);
    assert!(out.is_empty());
    assert!(err.contains("nonsense"), "{err}");
}

#[test]
fn help_goes_to_standard_output() {
    let (status, out, err) = run(&["flow", "--help"]);
    assert_eq!(status, 0);
    assert!(err.is_empty());
    assert!(
        out.contains("--no-source") && out.contains("--min-complexity"),
        "{out}"
    );
}

#[test]
fn a_missing_path_exits_1_with_a_message() {
    let (status, out, err) = run(&["calls", "/definitely/not/here", "--format", "json"]);
    assert_eq!(status, 1);
    assert!(out.is_empty());
    assert!(
        err.starts_with("chartedcode: cannot read /definitely/not/here"),
        "{err}"
    );
}

#[test]
fn a_directory_without_python_exits_1() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli_empty");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create");
    let (status, _, err) = run(&["classes", &dir.to_string_lossy(), "--format", "json"]);
    assert_eq!(status, 1);
    assert!(err.contains("no Python files found"), "{err}");
}

#[test]
fn json_goes_to_standard_output_by_default() {
    let (status, out, err) = run(&[
        "calls",
        &manifest("tests/fixtures/calls"),
        "--format",
        "json",
        "--no-gitignore",
    ]);
    assert_eq!(status, 0, "{err}");
    let data: Value = serde_json::from_str(&out).expect("one JSON document");
    assert_eq!(data["schema_version"], 1);
    assert!(data["calls"].as_array().is_some_and(|c| !c.is_empty()));
    assert!(
        data["cfgs"].as_array().is_some_and(Vec::is_empty),
        "graphs only for the flow view"
    );
}

#[test]
fn output_goes_to_the_named_file_and_nothing_to_standard_output() {
    let path = scratch_file("cli_out.json");
    let (status, out, _) = run(&[
        "classes",
        &manifest("tests/fixtures/imports"),
        "--format",
        "json",
        "-o",
        &path.to_string_lossy(),
    ]);
    assert_eq!(status, 0);
    assert!(out.is_empty());
    let data: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("file written")).expect("JSON");
    assert!(data["edges"].as_array().is_some_and(|e| !e.is_empty()));
}

#[test]
fn html_is_written_to_the_named_file() {
    let path = scratch_file("cli_out.html");
    let (status, out, err) = run(&[
        "calls",
        &manifest("tests/fixtures/calls"),
        "-o",
        &path.to_string_lossy(),
    ]);
    assert_eq!(status, 0, "{err}");
    assert!(out.is_empty());
    assert!(fs::read_to_string(&path)
        .expect("file written")
        .starts_with("<!doctype html>"));
}

#[test]
fn flow_options_reach_the_page() {
    let path = scratch_file("cli_flow.html");
    let (status, _, err) = run(&[
        "flow",
        &manifest("tests/fixtures/flow"),
        "--no-source",
        "--min-complexity",
        "3",
        "-o",
        &path.to_string_lossy(),
    ]);
    assert_eq!(status, 0, "{err}");
    let page = fs::read_to_string(&path).expect("file written");
    let open = "<script type=\"application/json\" id=\"data\">";
    let start = page.find(open).expect("data") + open.len();
    let data: Value =
        serde_json::from_str(&page[start..start + page[start..].find("</script>").expect("end")])
            .expect("JSON");
    let cfgs = data["cfgs"].as_array().expect("cfgs");
    assert!(!cfgs.is_empty() && cfgs.iter().all(|g| g["complexity"].as_u64() >= Some(3)));
    let texts_empty = cfgs
        .iter()
        .flat_map(|g| g["blocks"].as_array().expect("blocks"))
        .flat_map(|b| b["lines"].as_array().expect("lines"))
        .all(|l| l["text"] == "");
    assert!(texts_empty, "--no-source leaves the text out");
}

#[test]
fn private_names_can_start_shown() {
    let path = scratch_file("cli_private.html");
    let (status, _, _) = run(&[
        "calls",
        &manifest("tests/fixtures/calls"),
        "--include-private",
        "-o",
        &path.to_string_lossy(),
    ]);
    assert_eq!(status, 0);
    assert!(fs::read_to_string(&path)
        .expect("file written")
        .contains("\"showPrivate\":true"));
}

#[test]
fn an_unwritable_output_path_exits_1() {
    let (status, _, err) = run(&[
        "calls",
        &manifest("tests/fixtures/calls"),
        "-o",
        "/definitely/not/here/out.html",
    ]);
    assert_eq!(status, 1);
    assert!(err.contains("cannot write"), "{err}");
}
