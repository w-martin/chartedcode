//! The HTML pages: data, layout engine, escaping and determinism.

use std::fs;
use std::path::{Path, PathBuf};

use chartedcode_core::{analyze, render_html, Analysis, Options, RenderOptions, View};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn manifest(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn analysis_of(path: &Path, control_flow: bool) -> Analysis {
    analyze(
        path,
        &Options {
            respect_gitignore: false,
            control_flow,
            include_source: true,
        },
    )
    .expect("analyses")
}

/// The JSON a page embeds under `id`.
fn embedded(page: &str, id: &str) -> Value {
    let open = format!("<script type=\"application/json\" id=\"{id}\">");
    let start = page.find(&open).expect("script block") + open.len();
    let end = page[start..].find("</script>").expect("end of block") + start;
    serde_json::from_str(&page[start..end]).expect("valid JSON")
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch directory");
    dir
}

#[test]
fn each_page_embeds_its_data_and_the_layout_engine() {
    let a = analysis_of(&manifest("tests/fixtures/flow"), true);
    let elk = fs::read_to_string(manifest("assets/elk.bundled.js")).expect("elk");
    for view in [View::Classes, View::Calls, View::Flow] {
        let page = render_html(view, &a, &RenderOptions::default()).expect("renders");
        assert!(page.starts_with("<!doctype html>"), "{view:?}");
        assert!(
            page.contains(&elk),
            "{view:?}: the layout engine is embedded whole"
        );
        assert!(
            !page.contains("@@TITLE@@")
                && !page.contains("@@DATA@@")
                && !page.contains("@@CONFIG@@")
        );
        let data = embedded(&page, "data");
        assert_eq!(data["root"], a.root.as_str());
        match view {
            View::Classes => {
                assert_eq!(data["nodes"].as_array().map(Vec::len), Some(a.nodes.len()))
            }
            View::Calls => {
                assert_eq!(
                    data["functions"].as_array().map(Vec::len),
                    Some(a.functions.len())
                );
                assert_eq!(data["calls"].as_array().map(Vec::len), Some(a.calls.len()));
            }
            View::Flow => assert_eq!(data["cfgs"].as_array().map(Vec::len), Some(a.cfgs.len())),
        }
    }
}

#[test]
fn rendering_twice_gives_the_same_bytes() {
    let a = analysis_of(&manifest("tests/fixtures/calls"), true);
    for view in [View::Classes, View::Calls, View::Flow] {
        let first = render_html(view, &a, &RenderOptions::default()).expect("renders");
        let again = render_html(view, &a, &RenderOptions::default()).expect("renders");
        assert_eq!(first, again, "{view:?}");
    }
}

#[test]
fn source_text_cannot_break_out_of_the_page_or_fill_a_slot() {
    let root = scratch("render_hostile");
    fs::write(
        root.join("m.py"),
        "def f():\n    x = \"</script><b>@@ELK@@ @@TITLE@@ <!-- \"\n    return x\n",
    )
    .expect("write");
    let a = analysis_of(&root, true);
    let template = fs::read_to_string(manifest("assets/flow.html")).expect("template");
    let page = render_html(View::Flow, &a, &RenderOptions::default()).expect("renders");
    assert_eq!(
        page.matches("</script>").count(),
        template.matches("</script>").count(),
        "the page has the template's script blocks and no more"
    );
    assert_eq!(
        page.matches("@@ELK@@").count(),
        1,
        "the literal in the source stays text; the slot is filled"
    );
    let data = embedded(&page, "data");
    let lines: Vec<&str> = data["cfgs"][0]["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .flat_map(|b| b["lines"].as_array().expect("lines"))
        .filter_map(|l| l["text"].as_str())
        .collect();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("</script><b>@@ELK@@ @@TITLE@@ <!--")),
        "{lines:?}"
    );
}

#[test]
fn the_title_is_escaped() {
    let root = scratch("render_a&b");
    fs::write(root.join("m.py"), "def f():\n    pass\n").expect("write");
    let page = render_html(
        View::Calls,
        &analysis_of(&root, false),
        &RenderOptions::default(),
    )
    .expect("renders");
    assert!(
        page.contains("<title>render_a&amp;b — call graph</title>"),
        "{}",
        &page[..300]
    );
}

#[test]
fn the_flow_page_can_leave_out_simple_functions() {
    let a = analysis_of(&manifest("tests/fixtures/flow"), true);
    let all = render_html(View::Flow, &a, &RenderOptions::default()).expect("renders");
    let complex = render_html(
        View::Flow,
        &a,
        &RenderOptions {
            min_complexity: 3,
            ..RenderOptions::default()
        },
    )
    .expect("renders");
    let count = |page: &str| {
        embedded(page, "data")["cfgs"]
            .as_array()
            .map_or(0, Vec::len)
    };
    assert!(count(&complex) > 0 && count(&complex) < count(&all));
    let data = embedded(&complex, "data");
    assert!(data["cfgs"]
        .as_array()
        .expect("cfgs")
        .iter()
        .all(|g| g["complexity"].as_u64() >= Some(3)));
}

#[test]
fn the_private_names_option_reaches_the_page() {
    let a = analysis_of(&manifest("tests/fixtures/calls"), false);
    let shown = render_html(
        View::Calls,
        &a,
        &RenderOptions {
            show_private: true,
            ..RenderOptions::default()
        },
    )
    .expect("renders");
    let hidden = render_html(View::Calls, &a, &RenderOptions::default()).expect("renders");
    assert_eq!(embedded(&shown, "config")["showPrivate"], true);
    assert_eq!(embedded(&hidden, "config")["showPrivate"], false);
}

#[test]
fn the_layout_engine_is_the_pinned_unmodified_file() {
    let bytes = fs::read(manifest("assets/elk.bundled.js")).expect("elk");
    let recorded = fs::read_to_string(manifest("assets/elk.bundled.js.sha256")).expect("checksum");
    let expected = recorded.split_whitespace().next().expect("hash");
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        actual, expected,
        "assets/elk.bundled.js changed; it must stay the unmodified elkjs 0.11.0"
    );
    let text = String::from_utf8(bytes).expect("utf-8");
    assert!(
        !text.contains("</script"),
        "the bundle must be safe to inline in a script element"
    );
    assert!(fs::read_to_string(manifest("assets/elkjs-LICENSE.md"))
        .expect("licence")
        .contains("Eclipse Public License"));
}
