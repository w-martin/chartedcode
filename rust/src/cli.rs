//! The command line.

use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{analyze, render_html, write_bundle, Options, RenderOptions, View};

#[derive(Parser)]
#[command(
    name = "chartedcode",
    version,
    about = "Diagrams for Python projects: classes, calls and control flow, drawn as self-contained HTML"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Class diagram: inheritance, Protocol conformance and field types
    Classes(Common),
    /// Call graph, with call order and an execution-flow view
    Calls(Common),
    /// Control-flow graph of each function: branches, loops and handlers, with the calls in each block
    Flow(FlowArgs),
    /// Every diagram and the analysis as JSON, in one directory with a manifest of what was written
    All(AllArgs),
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Html,
    Json,
}

#[derive(Args)]
struct Common {
    /// A Python file or a directory of Python files
    path: PathBuf,
    /// Where to write the result (default: `<name>-<view>.chartedcode.html` in the current directory for html,
    /// standard output for json)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format
    #[arg(long, value_enum, default_value = "html")]
    format: Format,
    /// Start with private names (a single leading underscore) shown; json output always includes them
    #[arg(long)]
    include_private: bool,
    /// Do not skip files matched by .gitignore
    #[arg(long)]
    no_gitignore: bool,
}

#[derive(Args)]
struct FlowArgs {
    #[command(flatten)]
    common: Common,
    /// Leave the source text out of the diagram (blocks show only line numbers)
    #[arg(long)]
    no_source: bool,
    /// Only draw functions of at least this complexity (decision points plus one)
    #[arg(long, default_value_t = 0)]
    min_complexity: usize,
}

#[derive(Args)]
struct AllArgs {
    /// A Python file or a directory of Python files
    path: PathBuf,
    /// The directory to write into, created if needed (default: `<name>-chartedcode` in the current directory)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Start the pages with private names (a single leading underscore) shown
    #[arg(long)]
    include_private: bool,
    /// Do not skip files matched by .gitignore
    #[arg(long)]
    no_gitignore: bool,
    /// Leave the source text out of the flow diagrams and the JSON (blocks show only line numbers)
    #[arg(long)]
    no_source: bool,
    /// Only draw functions of at least this complexity in the flow page; the JSON keeps them all
    #[arg(long, default_value_t = 0)]
    min_complexity: usize,
}

/// The settings a view needs beyond the common ones.
struct Request {
    view: View,
    common: Common,
    no_source: bool,
    min_complexity: usize,
}

/// Run the command line with `argv` (the program name first), writing to `out` and `err`; returns the exit status.
pub fn execute(argv: Vec<String>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let cli = match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(e) => {
            let target: &mut dyn Write = if e.use_stderr() { err } else { out };
            let _ = write!(target, "{e}");
            return u8::try_from(e.exit_code()).unwrap_or(2);
        }
    };
    let request = match cli.command {
        Command::All(args) => {
            return match run_all(&args, out) {
                Ok(()) => 0,
                Err(message) => {
                    let _ = writeln!(err, "chartedcode: {message}");
                    1
                }
            };
        }
        Command::Classes(common) => Request {
            view: View::Classes,
            common,
            no_source: false,
            min_complexity: 0,
        },
        Command::Calls(common) => Request {
            view: View::Calls,
            common,
            no_source: false,
            min_complexity: 0,
        },
        Command::Flow(args) => Request {
            view: View::Flow,
            common: args.common,
            no_source: args.no_source,
            min_complexity: args.min_complexity,
        },
    };
    match run_view(&request, out) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(err, "chartedcode: {message}");
            1
        }
    }
}

fn run_all(args: &AllArgs, out: &mut dyn Write) -> Result<(), String> {
    let analysis = analyze(
        &args.path,
        &Options {
            respect_gitignore: !args.no_gitignore,
            control_flow: true,
            include_source: !args.no_source,
        },
    )
    .map_err(|e| e.to_string())?;
    let dir = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("{}-chartedcode", analysis.root)));
    let manifest = write_bundle(
        &analysis,
        &dir,
        &RenderOptions {
            show_private: args.include_private,
            min_complexity: args.min_complexity,
        },
    )?;
    writeln!(
        out,
        "wrote {} files and manifest.json to {}",
        manifest.files.len(),
        dir.display()
    )
    .map_err(|e| e.to_string())
}

fn run_view(request: &Request, out: &mut dyn Write) -> Result<(), String> {
    let Request {
        view,
        common,
        no_source,
        min_complexity,
    } = request;
    let analysis = analyze(
        &common.path,
        &Options {
            respect_gitignore: !common.no_gitignore,
            control_flow: *view == View::Flow,
            include_source: !no_source,
        },
    )
    .map_err(|e| e.to_string())?;
    let (text, default_name) = match common.format {
        Format::Json => (
            serde_json::to_string(&analysis).map_err(|e| e.to_string())?,
            None,
        ),
        Format::Html => (
            render_html(
                *view,
                &analysis,
                &RenderOptions {
                    show_private: common.include_private,
                    min_complexity: *min_complexity,
                },
            )
            .map_err(|e| e.to_string())?,
            Some(format!(
                "{}-{}.chartedcode.html",
                analysis.root,
                view.name()
            )),
        ),
    };
    match (&common.output, default_name) {
        (Some(path), _) => write_file(path, &text),
        (None, Some(name)) => {
            let path = PathBuf::from(name);
            write_file(&path, &text)?;
            writeln!(out, "wrote {}", path.display()).map_err(|e| e.to_string())
        }
        (None, None) => writeln!(out, "{text}").map_err(|e| e.to_string()),
    }
}

fn write_file(path: &std::path::Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Run the command line with `argv` against the process's standard streams.
#[must_use]
pub fn run(argv: Vec<String>) -> u8 {
    execute(argv, &mut io::stdout(), &mut io::stderr())
}

/// Like [`run`], but returns the status, standard output and standard error instead of writing them.
#[must_use]
pub fn run_captured(argv: Vec<String>) -> (u8, Vec<u8>, Vec<u8>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = execute(argv, &mut out, &mut err);
    (status, out, err)
}
