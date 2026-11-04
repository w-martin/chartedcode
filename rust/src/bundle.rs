//! Writing every diagram of a project as a set of files, with a manifest that describes them.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::Analysis;
use crate::render::{render_html, RenderOptions, View};

/// The version of the manifest layout; it changes when a field is renamed, removed or reinterpreted.
pub const MANIFEST_VERSION: u32 = 1;

/// What was written, and enough about each file to verify and index it.
///
/// There are no timestamps: the same input gives the same bytes, manifest included.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Manifest {
    pub schema_version: u32,
    pub tool: String,
    pub version: String,
    /// The `schema_version` of `analysis.json`.
    pub analysis_schema_version: u32,
    pub root: String,
    pub summary: Summary,
    pub files: Vec<ManifestFile>,
}

/// Totals over the analysed project.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Summary {
    pub files_analysed: usize,
    pub parse_failures: usize,
    pub classes: usize,
    pub class_relationships: usize,
    pub functions: usize,
    pub call_edges: usize,
    pub untraced_calls: usize,
    pub functions_with_flow: usize,
    pub max_complexity: usize,
}

/// One file of the set: its name, what it holds (`classes`, `calls`, `flow` or `analysis`), size and SHA-256.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ManifestFile {
    pub name: String,
    pub kind: String,
    pub bytes: usize,
    pub sha256: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn summary(analysis: &Analysis) -> Summary {
    Summary {
        files_analysed: analysis.files,
        parse_failures: analysis.parse_failures.len(),
        classes: analysis.nodes.iter().filter(|n| !n.external).count(),
        class_relationships: analysis.edges.len(),
        functions: analysis.functions.iter().filter(|f| !f.synthetic).count(),
        call_edges: analysis.calls.len(),
        untraced_calls: analysis.call_stats.unresolved,
        functions_with_flow: analysis.cfgs.len(),
        max_complexity: analysis
            .cfgs
            .iter()
            .map(|g| g.complexity)
            .max()
            .unwrap_or(0),
    }
}

/// Write `classes.html`, `calls.html`, `flow.html`, `analysis.json` and `manifest.json` into `dir`, creating it
/// if needed. Other files already in `dir` are left alone; these five are replaced.
///
/// `analysis` should have been made with control flow, or the flow page and `analysis.json` will have none.
///
/// # Errors
///
/// A message naming the file that could not be written, or saying that the analysis could not be written as JSON.
pub fn write_bundle(
    analysis: &Analysis,
    dir: &Path,
    options: &RenderOptions,
) -> Result<Manifest, String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let json = serde_json::to_string(analysis).map_err(|e| e.to_string())?;
    let mut pages = vec![];
    for (name, kind, view) in [
        ("classes.html", "classes", View::Classes),
        ("calls.html", "calls", View::Calls),
        ("flow.html", "flow", View::Flow),
    ] {
        let page = render_html(view, analysis, options).map_err(|e| e.to_string())?;
        pages.push((name, kind, page));
    }
    pages.push(("analysis.json", "analysis", json));
    let mut files = vec![];
    for (name, kind, text) in &pages {
        let path = dir.join(name);
        fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        files.push(ManifestFile {
            name: (*name).into(),
            kind: (*kind).into(),
            bytes: text.len(),
            sha256: sha256_hex(text.as_bytes()),
        });
    }
    let manifest = Manifest {
        schema_version: MANIFEST_VERSION,
        tool: "chartedcode".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        analysis_schema_version: analysis.schema_version,
        root: analysis.root.clone(),
        summary: summary(analysis),
        files,
    };
    let mut text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    text.push('\n');
    let path = dir.join("manifest.json");
    fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(manifest)
}
