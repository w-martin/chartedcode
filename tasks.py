"""Invoke tasks for chartedcode development."""

from pathlib import Path
from shutil import rmtree
from sysconfig import get_config_var, get_path
from zipfile import ZipFile

from invoke import Context, task

RUST_DIR = Path("rust")
PACKAGE_DIR = Path("src") / "chartedcode"
EXTENSION_BUILD_DIR = RUST_DIR / "target" / "extension"
EXTENSION_SUFFIXES = {".so", ".pyd"}


def _installed_binary() -> Path | None:
    """The `chartedcode` executable `maturin develop` installed into this environment, if any."""
    candidate = Path(get_path("scripts")) / f"chartedcode{get_config_var('EXE') or ''}"
    return candidate if candidate.is_file() else None


def _staged_extensions() -> list[Path]:
    """The compiled `chartedcode._rust` extension module staged in the package directory, if any (not its stub)."""
    return [path for path in PACKAGE_DIR.glob("_rust.*") if path.suffix in EXTENSION_SUFFIXES]


def _stage_extension(ctx: Context, *, release: bool) -> None:
    """Build the extension module and copy it into the package directory, where the wheel picks it up.

    maturin builds it, not cargo, so the cross-compiling toolchains and the macOS link flags are the ones
    the wheels use. The package's own wheel is built with `bindings = "bin"`, which cannot build it.
    """
    rmtree(EXTENSION_BUILD_DIR, ignore_errors=True)
    profile = "--release" if release else ""
    # From the crate's own directory: maturin would otherwise apply the `bindings = "bin"` of the root pyproject.toml.
    with ctx.cd(RUST_DIR / "python"):
        ctx.run(f'maturin build {profile} -o "{EXTENSION_BUILD_DIR.resolve()}"')
    for stale in _staged_extensions():
        stale.unlink()
    with ZipFile(next(EXTENSION_BUILD_DIR.glob("*.whl"))) as wheel:
        for member in wheel.namelist():
            if Path(member).name.startswith("_rust.") and Path(member).suffix in EXTENSION_SUFFIXES:
                (PACKAGE_DIR / Path(member).name).write_bytes(wheel.read(member))


def _needs_build() -> bool:
    """Whether the installed executable or the staged extension is older than the Rust sources.

    It must look at the executable `maturin develop` installs into the environment, not the one
    `cargo build` leaves in `rust/target/debug`: the two are built independently.
    """
    binary = _installed_binary()
    extensions = _staged_extensions()
    if binary is None or not extensions:
        return True
    built = min(binary.stat().st_mtime, *(extension.stat().st_mtime for extension in extensions))
    sources = [
        *(RUST_DIR / "src").rglob("*.rs"),
        *(RUST_DIR / "assets").rglob("*"),
        *(RUST_DIR / "python").glob("src/*.rs"),
        RUST_DIR / "python" / "build.rs",
        RUST_DIR / "Cargo.toml",
        RUST_DIR / "python" / "Cargo.toml",
        RUST_DIR / "Cargo.lock",
    ]
    return any(source.is_file() and source.stat().st_mtime > built for source in sources)


@task
def build(ctx: Context, *, force: bool = False) -> None:
    """Build the `chartedcode` executable and extension module, and install them with the package, if needed."""
    if force or _needs_build():
        print("Building chartedcode...")
        _stage_extension(ctx, release=False)
        ctx.run("maturin develop --manifest-path rust/Cargo.toml")
    else:
        print("chartedcode is up to date.")


@task(name="format-check")
def check_if_code_needs_formatting(ctx: Context) -> None:
    """Check formatting of the Python and Rust code."""
    ctx.run("ruff format --check")
    ctx.run(f"cd {RUST_DIR} && cargo fmt --all -- --check")


@task(name="format")
def format_code(ctx: Context) -> None:
    """Format the Python and Rust code."""
    ctx.run("ruff format .")
    ctx.run(f"cd {RUST_DIR} && cargo fmt --all")


@task
def lint(ctx: Context) -> None:
    """Run all linters: ruff check, ty check, bandit, cargo clippy."""
    ctx.run("ruff check .")
    ctx.run("ty check .")
    ctx.run("bandit -r src/ -c pyproject.toml")
    print("Running Rust clippy...")
    ctx.run(f"cd {RUST_DIR} && cargo clippy --workspace --all-targets -- -D warnings")


@task
def lint_fix(ctx: Context) -> None:
    """Run ruff check with --fix and ruff format."""
    ctx.run("ruff check --fix .")
    ctx.run("ruff format .")


@task(pre=[build])
def test(ctx: Context) -> None:
    """Run pytest with branch coverage and the Rust tests. Builds the executable if needed."""
    print("Running Python tests...")
    ctx.run("python -m pytest tests/")
    print("Running Rust tests...")
    ctx.run(f"cd {RUST_DIR} && cargo test --workspace")


@task
def verify_licences(ctx: Context) -> None:
    """Verify dependency licenses against the policy in pyproject.toml."""
    ctx.run("trustedlicenses --quiet")


@task(name="all", pre=[build])
def all_checks(ctx: Context) -> None:
    """Run all checks: format, lint, test, verify-licences."""
    format_code(ctx)
    lint(ctx)
    test(ctx)
    verify_licences(ctx)
