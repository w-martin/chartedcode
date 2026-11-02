# chartedcode

Diagrams for Python projects: **class**, **call** and **control-flow** diagrams, drawn as self-contained HTML by a small, deterministic Rust program.

Point it at a file or a directory. It reads the source (it never runs it), works out how the code fits together, and writes one HTML file per diagram. Each file holds its data, the layout engine and the viewer, so it opens in any browser, works offline, and needs no Graphviz or other renderer installed.

```console
$ chartedcode classes src/        # class diagram
$ chartedcode calls src/          # call graph
$ chartedcode flow src/           # control-flow graph of each function
wrote src-flow.chartedcode.html
```

## Install

```console
$ pip install chartedcode        # or: uvx chartedcode calls src/
```

The wheel carries the compiled program; there is nothing else to install. Python 3.11 or later.

## The diagrams

Every relationship says how it was found, and anything the tool could not follow is counted and listed, not guessed at or silently dropped.

### `classes`

Inheritance, Protocol conformance and field types.

- **inherits**: a base class, resolved through imports (including relative imports, re-exports through `__init__.py`, star imports and `TYPE_CHECKING` imports).
- **implements**: a class has every method name of a `Protocol`. This compares names only, not signatures, so it is shown as weak evidence.
- **has-a**: a field whose annotation names a class of the project (class-body annotations, and `self.x` assigned from an annotated parameter in `__init__`).

### `calls`

Which function calls which, and in what order.

- **calls** and **constructs** (`Foo()` goes to `Foo.__init__`, or to an implicit constructor when the class defines none).
- **passes**: a function or method handed to another function as a callback (`runner.run("train", self._trainer.fit)`), which is assumed to be called.
- **dispatches**: a call to a Protocol method may run any implementer's method of that name.
- **Call order**: each call is numbered within its caller, in evaluation order (a call's arguments before the call). Numbers appear on the arrows, in a panel listing the focused function's calls, and in an **execution flow** view that chains the calls in order and enters each callee's own calls. Recursion is marked, not drawn, so the execution flow is always acyclic.
- **Acyclic view**: functions that call each other in a cycle are collapsed into one node. When there are no cycles the page says it is identical to the call graph.
- Views at function, class or module granularity; focus on one node with its callers and callees to a chosen depth.

The colour of an edge says how sure the tool is: resolved by name, import, `self` or `super`; inferred from a declared type; method names only; external.

### `flow`

The control-flow graph of each function (and of a module's top-level code): basic blocks with their source lines, edges labelled with when they are taken (`true`, `false`, `next item`, `loop`, `break`, `exception`, `return`, ...), unreachable code marked, and a complexity number (decision points plus one). The calls in each block carry the same step numbers as the call graph, and a call that has a flow of its own can be opened with two clicks.

Approximations, so the diagram can be read correctly: any statement in a `try` body may raise (one `exception` edge from the body to each handler), context managers do not branch, and boolean operators, conditional expressions and comprehensions do not split a block.

## What it does not follow

Static analysis of a dynamic language has limits. The tool does not infer return types, so a call on a variable whose type is not declared (`model.evaluate()` where `model` is whatever a function returned) is **untraced**: counted per function, listed in the page, never drawn as an edge. It also does not follow dispatch through subclass overrides (only Protocols dispatch), property access that runs code, decorators, `getattr` and other dynamic calls, function-local imports or `__all__`. Branches and loops are flattened in the call numbering: a number means "the order written", not a guaranteed run order.

## Options

| Option | Effect |
| --- | --- |
| `-o, --output FILE` | where to write the result; by default `<name>-<view>.chartedcode.html` in the current directory, or standard output for JSON |
| `--format html\|json` | the page, or the analysis as JSON (`schema_version` 1; always includes private names) |
| `--include-private` | start with private names (a single leading underscore) shown; the page hides them by default |
| `--no-gitignore` | also analyse files matched by `.gitignore` (by default they and hidden directories are skipped; the global ignore file is never read, so a checkout gives the same result everywhere) |
| `--no-source` | `flow` only: leave source text out of the diagram, so blocks show only line numbers |
| `--min-complexity N` | `flow` only: only draw functions of at least this complexity |

The same input always gives byte-identical output.

**A `flow` page contains the source text of your functions.** Use `--no-source` before sharing one if that matters.

## From Python

```console
$ python -m chartedcode calls src/
$ chartedcode-py calls src/            # always inside this process
$ CHARTEDCODE_INPROC=1 python -m chartedcode calls src/
```

`python -m chartedcode` starts the executable that came with the package. For environments where the interpreter may run code but no other program may be started, `chartedcode-py` (or `CHARTEDCODE_INPROC=1`) runs the same program inside the interpreter, through an extension module; the output is identical. `chartedcode._binary.run_binary(["calls", "src/", "--format", "json"], capture=True)` runs it from code.

## Development

```console
$ uv sync
$ uv run inv build          # build the executable and the extension, and install them
$ uv run inv all            # format, lint, test (Rust and Python), verify licences
```

- `uv run inv test` builds first. Run bare `pytest` as `uv run --no-sync pytest` after `uv run inv build`: a plain `uv run` re-syncs the project and can put an older cached build over the one just installed. The test session refuses to run against an executable older than the Rust sources.
- The browser tests drive a headless Chrome and are skipped when none is found (set `CHROME` to its path).
- Build wheels from the working tree after staging the extension (`uv run inv build` does that), with `maturin build --profile cli`. `uv build` builds the wheel from the source distribution, which does not contain the compiled extension.

## Licence

MIT (see `LICENSE`). Every page embeds [elkjs](https://github.com/kieler/elkjs) 0.11.0, unmodified, which is licensed under the Eclipse Public License 2.0; see `NOTICE` and `rust/assets/elkjs-LICENSE.md`.
