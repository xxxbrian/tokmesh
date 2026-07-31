#!/usr/bin/env python3
"""Download GitHub Release assets into packaging/npm platform packages.

Usage:
  scripts/prepare-npm-binaries.py 0.1.0
  scripts/prepare-npm-binaries.py 0.1.0 --repo xxxbrian/tokmesh

Requires: gh (authenticated for private repos), tar; unzip or Python zipfile.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
NPM = ROOT / "packaging" / "npm"


def run(cmd: list[str], **kw) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.check_call(cmd, **kw)


def normalize(version: str) -> str:
    return version[1:] if version.startswith(("v", "V")) else version


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("version")
    ap.add_argument("--repo", default="xxxbrian/tokmesh")
    args = ap.parse_args()
    version = normalize(args.version)
    tag = f"v{version}"
    platforms = json.loads((NPM / "platforms.json").read_text())["platforms"]

    with tempfile.TemporaryDirectory(prefix="tokmesh-npm-") as tmp:
        tmp_path = pathlib.Path(tmp)
        for p in platforms:
            triple = p["rust_target"]
            ext = p["ext"]
            asset = f"tokmesh-{version}-{triple}.{ext}"
            dest_dir = NPM / p["dir"]
            dest_dir.mkdir(parents=True, exist_ok=True)
            # clear old binary
            for stale in dest_dir.glob("tokmesh*"):
                if stale.name in ("index.js", "package.json") or stale.suffix == ".md":
                    continue
                if stale.is_file() and stale.name.startswith("tokmesh"):
                    stale.unlink()

            out_asset = tmp_path / asset
            run(
                [
                    "gh",
                    "release",
                    "download",
                    tag,
                    "-R",
                    args.repo,
                    "-p",
                    asset,
                    "-D",
                    str(tmp_path),
                ]
            )
            extract_dir = tmp_path / f"ex-{triple}"
            extract_dir.mkdir()
            if ext == "zip":
                with zipfile.ZipFile(out_asset) as zf:
                    zf.extractall(extract_dir)
            else:
                run(["tar", "-xzf", str(out_asset), "-C", str(extract_dir)])

            binary_name = p["binary"]
            src = extract_dir / binary_name
            if not src.is_file():
                # sometimes nested
                matches = list(extract_dir.rglob(binary_name))
                if not matches:
                    raise SystemExit(f"binary {binary_name} not found in {asset}")
                src = matches[0]
            dest = dest_dir / binary_name
            shutil.copy2(src, dest)
            dest.chmod(0o755)
            print(f"staged {dest} ({dest.stat().st_size} bytes)")

    print("npm binaries ready")


if __name__ == "__main__":
    main()
