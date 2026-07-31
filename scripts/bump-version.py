#!/usr/bin/env python3
"""Bump the unified workspace version used by all tokmesh crates.

Usage:
  scripts/bump-version.py 0.2.0
  scripts/bump-version.py v0.2.0

Updates:
  - [workspace.package] version in root Cargo.toml
  - version fields on internal path deps (tokmesh-core / tokmesh-cli)
  - packaging/npm/**/package.json version (+ @tokmesh/* pins) if present
  - packaging/pypi/pyproject.toml version if present

Does not create git commits or tags. Review the diff, commit, then tag vX.Y.Z.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


def normalize(version: str) -> str:
    v = version.strip()
    if v.startswith(("v", "V")):
        v = v[1:]
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:[0-9A-Za-z\.-]+)?", v):
        raise SystemExit(f"invalid semver-ish version: {version!r}")
    return v


def patch_workspace_package_version(text: str, version: str) -> str:
    updated, n = re.subn(
        r'(?m)^(\[workspace\.package\]\n(?:(?!^\[).*\n)*?^version = ")([^"]+)(")',
        rf"\g<1>{version}\g<3>",
        text,
        count=1,
    )
    if n != 1:
        raise SystemExit("failed to patch [workspace.package] version")
    return updated


def patch_internal_dep_versions(text: str, version: str) -> str:
    out = text
    for name in ("tokmesh-core", "tokmesh-cli"):
        pattern = rf'(?m)^({re.escape(name)}\s*=\s*\{{[^\n]*?\bversion\s*=\s*")([^"]+)(")'
        out, n = re.subn(pattern, rf"\g<1>{version}\g<3>", out, count=1)
        if n != 1:
            raise SystemExit(f"failed to patch version on workspace dep {name}")
    return out


def patch_npm(version: str) -> list[str]:
    changed: list[str] = []
    npm_root = ROOT / "packaging" / "npm"
    if not npm_root.is_dir():
        return changed
    for path in sorted(npm_root.rglob("package.json")):
        text = path.read_text()
        new = re.sub(
            r'("version"\s*:\s*")([^"]+)(")',
            rf"\g<1>{version}\g<3>",
            text,
            count=1,
        )
        new = re.sub(
            r'("@tokmesh/[^"]+"\s*:\s*")(\d+\.\d+\.\d+[0-9A-Za-z\.-]*)(")',
            rf"\g<1>{version}\g<3>",
            new,
        )
        if new != text:
            path.write_text(new)
            changed.append(str(path.relative_to(ROOT)))
    return changed


def patch_pypi(version: str) -> list[str]:
    changed: list[str] = []
    path = ROOT / "packaging" / "pypi" / "pyproject.toml"
    if not path.is_file():
        return changed
    text = path.read_text()
    updated, n = re.subn(
        r'(?m)^(version\s*=\s*")([^"]+)(")',
        rf"\g<1>{version}\g<3>",
        text,
        count=1,
    )
    if n == 1 and updated != text:
        path.write_text(updated)
        changed.append(str(path.relative_to(ROOT)))
    return changed


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("version", help="New version (0.2.0) or tag (v0.2.0)")
    args = ap.parse_args()
    version = normalize(args.version)

    cargo = ROOT / "Cargo.toml"
    text = cargo.read_text()
    text = patch_workspace_package_version(text, version)
    text = patch_internal_dep_versions(text, version)
    cargo.write_text(text)

    changed = ["Cargo.toml", *patch_npm(version), *patch_pypi(version)]
    print(f"workspace version -> {version}")
    for c in changed:
        print(f"  updated {c}")
    print()
    print("Next (when you intend to release):")
    print("  1. review:  git diff")
    print(f"  2. commit:  git commit -am 'chore: release {version}'")
    print(f"  3. tag:     git tag -a v{version} -m 'v{version}'")
    print("  4. push:    git push origin main --tags")
    print("  5. wait for Release workflow (binaries) to succeed")
    print(f"  6. run Publish registries workflow with version={version}")


if __name__ == "__main__":
    main()
