#!/usr/bin/env python3
"""Build platform PyPI wheels that embed the native tokmesh binary.

Usage:
  scripts/build-pypi-wheels.py 0.1.0
  scripts/build-pypi-wheels.py 0.1.0 --out dist/pypi

Requires: gh, tar, Python 3.9+, pip package `build` (installed automatically).
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
PYPI_SRC = ROOT / "packaging" / "pypi"

# rust_target, pep425_tag, binary_name, archive_ext
# musl static Linux binaries run on typical glibc hosts and Alpine.
WHEEL_TARGETS = [
    ("x86_64-unknown-linux-musl", "py3-none-manylinux_2_17_x86_64", "tokmesh", "tar.gz"),
    ("aarch64-unknown-linux-musl", "py3-none-manylinux_2_17_aarch64", "tokmesh", "tar.gz"),
    ("x86_64-apple-darwin", "py3-none-macosx_11_0_x86_64", "tokmesh", "tar.gz"),
    ("aarch64-apple-darwin", "py3-none-macosx_11_0_arm64", "tokmesh", "tar.gz"),
    ("x86_64-pc-windows-msvc", "py3-none-win_amd64", "tokmesh.exe", "zip"),
    ("aarch64-pc-windows-msvc", "py3-none-win_arm64", "tokmesh.exe", "zip"),
]


def normalize(version: str) -> str:
    return version[1:] if version.startswith(("v", "V")) else version


def run(cmd: list[str], **kw) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.check_call(cmd, **kw)


def record_hash(data: bytes) -> str:
    digest = hashlib.sha256(data).digest()
    b64 = base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")
    return f"sha256={b64}"


def wheel_meta_files(name: str, version: str, tag: str) -> dict[str, bytes]:
    readme = ""
    rm = PYPI_SRC / "README.md"
    if rm.is_file():
        readme = rm.read_text(encoding="utf-8")
    metadata = "\n".join(
        [
            "Metadata-Version: 2.1",
            f"Name: {name}",
            f"Version: {version}",
            "Summary: Local AI coding token analytics CLI and TUI",
            "Home-page: https://github.com/xxxbrian/tokmesh",
            "Author: Tokmesh contributors",
            "License: MIT",
            "Classifier: License :: OSI Approved :: MIT License",
            "Classifier: Programming Language :: Python :: 3",
            "Requires-Python: >=3.9",
            "Description-Content-Type: text/markdown",
            "",
            readme,
        ]
    )
    wheel = "\n".join(
        [
            "Wheel-Version: 1.0",
            "Generator: tokmesh-build-pypi-wheels",
            "Root-Is-Purelib: false",
            f"Tag: {tag}",
            "",
        ]
    )
    entry_points = "\n".join(
        [
            "[console_scripts]",
            "tokmesh = tokmesh._cli:main",
            "",
        ]
    )
    dist_info = f"{name}-{version}.dist-info"
    return {
        f"{dist_info}/METADATA": metadata.encode("utf-8"),
        f"{dist_info}/WHEEL": wheel.encode("utf-8"),
        f"{dist_info}/entry_points.txt": entry_points.encode("utf-8"),
        f"{dist_info}/top_level.txt": b"tokmesh\n",
    }


def build_wheel(
    version: str,
    tag: str,
    binary_path: pathlib.Path,
    binary_name: str,
    out_dir: pathlib.Path,
) -> pathlib.Path:
    name = "tokmesh"
    dist_info = f"{name}-{version}.dist-info"
    wheel_path = out_dir / f"{name}-{version}-{tag}.whl"

    files: list[tuple[str, bytes, int]] = []  # arcname, data, mode

    pkg_root = PYPI_SRC / "src" / "tokmesh"
    for path in sorted(pkg_root.rglob("*")):
        if not path.is_file():
            continue
        if path.suffix not in {".py", ".typed"}:
            continue
        rel = path.relative_to(pkg_root.parent).as_posix()
        if path.name == "__init__.py":
            data = (
                '"""tokmesh — thin Python entry for the native CLI binary."""\n\n'
                f'__version__ = "{version}"\n'
            ).encode("utf-8")
        else:
            data = path.read_bytes()
        files.append((rel, data, 0o644))

    files.append((f"tokmesh/{binary_name}", binary_path.read_bytes(), 0o755))

    for arcname, data in wheel_meta_files(name, version, tag).items():
        files.append((arcname, data, 0o644))

    record_lines: list[str] = []
    for arcname, data, _mode in files:
        record_lines.append(f"{arcname},{record_hash(data)},{len(data)}")
    record_lines.append(f"{dist_info}/RECORD,,")
    record_bytes = ("\n".join(record_lines) + "\n").encode("utf-8")
    files.append((f"{dist_info}/RECORD", record_bytes, 0o644))

    out_dir.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(wheel_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for arcname, data, mode in files:
            info = zipfile.ZipInfo(arcname)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3  # Unix
            info.external_attr = (mode & 0xFFFF) << 16
            zf.writestr(info, data)

    print(f"built {wheel_path.name} ({wheel_path.stat().st_size} bytes)")
    return wheel_path


def build_sdist(version: str, out_dir: pathlib.Path) -> None:
    with tempfile.TemporaryDirectory(prefix="tokmesh-sdist-") as sdir:
        sdist_path = pathlib.Path(sdir) / "pypi"
        shutil.copytree(PYPI_SRC, sdist_path)
        pyproject = (sdist_path / "pyproject.toml").read_text(encoding="utf-8")
        pyproject = re.sub(
            r'(?m)^version = "[^"]+"',
            f'version = "{version}"',
            pyproject,
            count=1,
        )
        (sdist_path / "pyproject.toml").write_text(pyproject, encoding="utf-8")
        (sdist_path / "src" / "tokmesh" / "__init__.py").write_text(
            '"""tokmesh — thin Python entry for the native CLI binary."""\n\n'
            f'__version__ = "{version}"\n',
            encoding="utf-8",
        )
        run([sys.executable, "-m", "pip", "install", "-q", "build"])
        run(
            [sys.executable, "-m", "build", "--sdist", "--outdir", str(out_dir.resolve())],
            cwd=str(sdist_path),
        )


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("version")
    ap.add_argument("--repo", default="xxxbrian/tokmesh")
    ap.add_argument("--out", default=str(ROOT / "dist" / "pypi"))
    ap.add_argument("--skip-sdist", action="store_true")
    args = ap.parse_args()
    version = normalize(args.version)
    rel_tag = f"v{version}"
    out_dir = pathlib.Path(args.out)
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)

    with tempfile.TemporaryDirectory(prefix="tokmesh-pypi-") as tmp:
        tmp_path = pathlib.Path(tmp)
        for rust_target, wheel_tag, binary_name, ext in WHEEL_TARGETS:
            asset = f"tokmesh-{version}-{rust_target}.{ext}"
            run(
                [
                    "gh",
                    "release",
                    "download",
                    rel_tag,
                    "-R",
                    args.repo,
                    "-p",
                    asset,
                    "-D",
                    str(tmp_path),
                ]
            )
            extract_dir = tmp_path / f"ex-{rust_target}"
            extract_dir.mkdir(exist_ok=True)
            asset_path = tmp_path / asset
            if ext == "zip":
                with zipfile.ZipFile(asset_path) as zf:
                    zf.extractall(extract_dir)
            else:
                run(["tar", "-xzf", str(asset_path), "-C", str(extract_dir)])
            matches = list(extract_dir.rglob(binary_name))
            if not matches:
                raise SystemExit(f"missing {binary_name} in {asset}")
            build_wheel(version, wheel_tag, matches[0], binary_name, out_dir)

    if not args.skip_sdist:
        build_sdist(version, out_dir)

    print("artifacts in", out_dir)
    for p in sorted(out_dir.iterdir()):
        print(f"  {p.name}  {p.stat().st_size}")


if __name__ == "__main__":
    main()
