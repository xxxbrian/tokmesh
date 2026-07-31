"""Locate and exec the native tokmesh binary shipped in the wheel."""

from __future__ import annotations

import os
import sys
from pathlib import Path


def _candidate_paths() -> list[Path]:
    """Binary may live next to the package or in the scripts/ data dir."""
    here = Path(__file__).resolve().parent
    names = ["tokmesh.exe", "tokmesh"] if os.name == "nt" else ["tokmesh"]
    out: list[Path] = []
    # 1) packaged beside the Python module (our wheel layout)
    for name in names:
        out.append(here / name)
    # 2) same directory as this interpreter's scripts (console_scripts sibling)
    scripts = Path(sys.executable).resolve().parent
    for name in names:
        out.append(scripts / name)
    return out


def find_binary() -> Path:
    for path in _candidate_paths():
        if path.is_file():
            return path
    searched = "\n  ".join(str(p) for p in _candidate_paths())
    raise FileNotFoundError(
        "tokmesh native binary not found. Reinstall the platform wheel, or use:\n"
        "  cargo install tokmesh\n"
        "  mise use github:xxxbrian/tokmesh\n"
        f"Searched:\n  {searched}"
    )


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    binary = find_binary()
    if os.name != "nt":
        try:
            binary.chmod(binary.stat().st_mode | 0o111)
        except OSError:
            pass
    # Replace current process so signals/exit codes behave like a real CLI.
    if os.name == "nt":
        import subprocess

        completed = subprocess.run([str(binary), *argv])
        return int(completed.returncode)
    os.execv(str(binary), [str(binary), *argv])
    return 0  # pragma: no cover


if __name__ == "__main__":
    raise SystemExit(main())
