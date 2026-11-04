//! The set of files written by `write_bundle` and the `all` command, and the manifest that describes them.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chartedcode_core::cli::run_captured;
use chartedcode_core::{
    analyze, write_bundle, Analysis, Manifest, Options, RenderOptions, MANIFEST_VERSION,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const NAMES: [&str; 5] = [
    "analysis.json",
    "calls.html",
    "classes.html",
    "flow.html",
    "manifest.json",
];

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn analysis() -> Analysis {
    analyze(
        &fixture("calls"),
        &Options {
            respect_gitignore: false,
            control_flow: true,
            include_source: true,
        },
    )
    .expect("fixture analyses")
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn listing(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .expect("directory")
        .map(|e| e.expect("entry").path())
        .map(|p| {
            (
                p.file_name().expect("name").to_string_lossy().into_owned(),
                fs::read(&p).expect("read"),
            )
        })
        .collect()
}

fn read_manifest(dir: &Path) -> Manifest {
    serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("manifest"))
        .expect("valid manifest")
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
fn the_directory_holds_exactly_the_five_files_with_stable_names() {
    let dir = scratch("bundle_names");
    let manifest = write_bundle(&analysis(), &dir, &RenderOptions::default()).expect("writes");
    assert_eq!(
        listing(&dir).keys().map(String::as_str).collect::<Vec<_>>(),
        NAMES
    );
    let listed: Vec<&str> = manifest.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        listed,
        ["classes.html", "calls.html", "flow.html", "analysis.json"]
    );
    let kinds: Vec<&str> = manifest.files.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(kinds, ["classes", "calls", "flow", "analysis"]);
}

#[test]
fn every_size_and_hash_in_the_manifest_matches_the_file() {
    let dir = scratch("bundle_hashes");
    write_bundle(&analysis(), &dir, &RenderOptions::default()).expect("writes");
    let manifest = read_manifest(&dir);
    assert!(manifest.files.len() == 4 && manifest.files.iter().all(|f| f.name != "manifest.json"));
    for file in &manifest.files {
        let bytes = fs::read(dir.join(&file.name)).expect("file");
        assert_eq!(file.bytes, bytes.len(), "{}: size", file.name);
        assert_eq!(file.sha256, hash(&bytes), "{}: hash", file.name);
    }
}

#[test]
fn the_manifest_names_the_tool_versions_and_project() {
    let dir = scratch("bundle_meta");
    let a = analysis();
    write_bundle(&a, &dir, &RenderOptions::default()).expect("writes");
    let manifest = read_manifest(&dir);
    assert_eq!(manifest.schema_version, MANIFEST_VERSION);
    assert_eq!(manifest.tool, "chartedcode");
    assert_eq!(manifest.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest.analysis_schema_version, a.schema_version);
    assert_eq!(manifest.root, "calls");
}

#[test]
fn the_summary_agrees_with_the_analysis() {
    let dir = scratch("bundle_summary");
    let a = analysis();
    write_bundle(&a, &dir, &RenderOptions::default()).expect("writes");
    let s = read_manifest(&dir).summary;
    assert_eq!(s.files_analysed, a.files);
    assert_eq!(s.parse_failures, 0);
    assert_eq!(s.classes, a.nodes.iter().filter(|n| !n.external).count());
    assert_eq!(s.class_relationships, a.edges.len());
    assert_eq!(
        s.functions,
        a.functions.iter().filter(|f| !f.synthetic).count()
    );
    assert_eq!(s.call_edges, a.calls.len());
    assert_eq!(s.untraced_calls, a.call_stats.unresolved);
    assert_eq!(s.functions_with_flow, a.cfgs.len());
    assert_eq!(
        Some(s.max_complexity),
        a.cfgs.iter().map(|g| g.complexity).max()
    );
    assert!(s.classes > 0 && s.functions > 0 && s.call_edges > 0);
}

#[test]
fn two_runs_write_byte_identical_directories() {
    let (one, two) = (scratch("bundle_det_a"), scratch("bundle_det_b"));
    write_bundle(&analysis(), &one, &RenderOptions::default()).expect("writes");
    write_bundle(&analysis(), &two, &RenderOptions::default()).expect("writes");
    assert_eq!(listing(&one), listing(&two));
}

#[test]
fn the_analysis_file_is_the_whole_analysis_even_when_the_flow_page_is_filtered() {
    let dir = scratch("bundle_filter");
    let a = analysis();
    write_bundle(
        &a,
        &dir,
        &RenderOptions {
            min_complexity: 2,
            ..RenderOptions::default()
        },
    )
    .expect("writes");
    let saved: Analysis =
        serde_json::from_slice(&fs::read(dir.join("analysis.json")).expect("json")).expect("valid");
    assert_eq!(saved, a);
    let page = fs::read_to_string(dir.join("flow.html")).expect("page");
    let open = "<script type=\"application/json\" id=\"data\">";
    let start = page.find(open).expect("data") + open.len();
    let data: Value =
        serde_json::from_str(&page[start..start + page[start..].find("</script>").expect("end")])
            .expect("json");
    let drawn = data["cfgs"].as_array().expect("cfgs").len();
    assert!(drawn > 0 && drawn < a.cfgs.len());
}

#[test]
fn existing_files_are_replaced_and_others_left_alone() {
    let dir = scratch("bundle_replace");
    fs::create_dir_all(&dir).expect("create");
    fs::write(dir.join("calls.html"), "old").expect("write");
    fs::write(dir.join("notes.txt"), "mine").expect("write");
    write_bundle(&analysis(), &dir, &RenderOptions::default()).expect("writes");
    assert!(fs::read_to_string(dir.join("calls.html"))
        .expect("page")
        .starts_with("<!doctype html>"));
    assert_eq!(
        fs::read_to_string(dir.join("notes.txt")).expect("notes"),
        "mine"
    );
}

#[test]
fn an_output_path_that_is_a_file_is_an_error_naming_it() {
    let dir = scratch("bundle_file");
    fs::create_dir_all(&dir).expect("create");
    let blocker = dir.join("blocker");
    fs::write(&blocker, "x").expect("write");
    let message = write_bundle(&analysis(), &blocker.join("sub"), &RenderOptions::default())
        .expect_err("fails");
    assert!(message.starts_with("cannot create"), "{message}");
}

#[test]
fn the_all_command_writes_the_set_and_says_where() {
    let dir = scratch("bundle_cli");
    let (status, out, err) = run(&[
        "all",
        &fixture("calls").to_string_lossy(),
        "--no-gitignore",
        "-o",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(status, 0, "{err}");
    assert!(
        out.starts_with("wrote 4 files and manifest.json to "),
        "{out}"
    );
    assert_eq!(
        listing(&dir).keys().map(String::as_str).collect::<Vec<_>>(),
        NAMES
    );
}

#[test]
fn the_all_options_reach_the_files() {
    let dir = scratch("bundle_cli_options");
    let (status, _, err) = run(&[
        "all",
        &fixture("flow").to_string_lossy(),
        "--no-gitignore",
        "--no-source",
        "--include-private",
        "--min-complexity",
        "3",
        "-o",
        &dir.to_string_lossy(),
    ]);
    assert_eq!(status, 0, "{err}");
    let saved: Analysis =
        serde_json::from_slice(&fs::read(dir.join("analysis.json")).expect("json")).expect("valid");
    assert!(
        saved
            .cfgs
            .iter()
            .flat_map(|g| &g.blocks)
            .flat_map(|b| &b.lines)
            .all(|l| l.text.is_empty()),
        "--no-source"
    );
    assert!(
        fs::read_to_string(dir.join("calls.html"))
            .expect("page")
            .contains("\"showPrivate\":true"),
        "--include-private"
    );
}

#[test]
fn the_all_command_reports_failures_with_status_1() {
    let (status, _, err) = run(&["all", "/definitely/not/here", "-o", "unused"]);
    assert_eq!(status, 1);
    assert!(err.starts_with("chartedcode: cannot read"), "{err}");
}
