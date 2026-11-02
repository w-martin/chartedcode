"""Driving a headless Chrome over a generated page."""

import html
import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any

import pytest

_MACOS_CHROME = Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
_COMMANDS = ("google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "chrome")
_TIMEOUT_SECONDS = 300


def find_chrome() -> str | None:
    """The Chrome or Chromium to drive: `$CHROME`, the macOS application, or one on PATH."""
    from_environment = os.environ.get("CHROME")
    if from_environment and Path(from_environment).exists():
        return from_environment
    if _MACOS_CHROME.exists():
        return str(_MACOS_CHROME)
    for command in _COMMANDS:
        found = shutil.which(command)
        if found:
            return found
    return None


CHROME = find_chrome()
requires_chrome = pytest.mark.skipif(CHROME is None, reason="no Chrome installed (set CHROME to its path)")


def drive(page: Path, script: str, fragment: str = "", budget_ms: int = 120_000) -> Any:  # noqa: ANN401
    """Open `page` in a headless Chrome, run `script` in it, and return the JSON the script leaves in `#log`.

    Args:
        page: A generated HTML page.
        script: JavaScript that ends by appending `<pre id="log">` holding a JSON document.
        fragment: The URL fragment to open the page with (it sets the initial state).
        budget_ms: How much virtual time the page may use.

    Returns:
        The parsed JSON.
    """
    driven = page.with_name(f"_driven_{page.name}")
    driven.write_text(
        page.read_text(encoding="utf-8").replace("</body>", f"<script>{script}</script></body>"), encoding="utf-8"
    )
    try:
        completed = subprocess.run(
            [
                str(CHROME),
                "--headless=new",
                "--disable-gpu",
                "--window-size=1800,1000",
                f"--virtual-time-budget={budget_ms}",
                "--dump-dom",
                f"file://{driven}#{fragment}",
            ],
            capture_output=True,
            text=True,
            timeout=_TIMEOUT_SECONDS,
            check=False,
        )
    finally:
        driven.unlink(missing_ok=True)
    found = re.search(r'<pre id="log">(.*?)</pre>', completed.stdout, re.DOTALL)
    if found is None:
        pytest.fail(f"the page script did not finish; Chrome said: {completed.stderr[-500:]}")
    return json.loads(html.unescape(found.group(1)))
