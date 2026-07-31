#!/usr/bin/env bash
# Verify that GitHub Release v$VERSION has all expected binary assets.
# Usage: scripts/verify-github-release.sh 0.1.0
#        scripts/verify-github-release.sh v0.1.0
set -euo pipefail

RAW="${1:-}"
if [[ -z "$RAW" ]]; then
  echo "usage: $0 <version|vversion>" >&2
  exit 2
fi
VERSION="${RAW#v}"
TAG="v${VERSION}"

TARGETS=(
  x86_64-unknown-linux-gnu
  x86_64-unknown-linux-musl
  aarch64-unknown-linux-gnu
  aarch64-unknown-linux-musl
  x86_64-apple-darwin
  aarch64-apple-darwin
  x86_64-pc-windows-msvc
  aarch64-pc-windows-msvc
)

echo "Checking GitHub Release ${TAG} …"
if ! gh release view "$TAG" >/dev/null 2>&1; then
  echo "ERROR: release ${TAG} not found (binaries must be published first)" >&2
  exit 1
fi

mapfile -t ASSETS < <(gh release view "$TAG" --json assets -q '.assets[].name' | sort)
echo "Found ${#ASSETS[@]} assets"

missing=0
for t in "${TARGETS[@]}"; do
  if [[ "$t" == *windows* ]]; then
    base="tokmesh-${VERSION}-${t}.zip"
  else
    base="tokmesh-${VERSION}-${t}.tar.gz"
  fi
  for name in "$base" "${base}.sha256"; do
    if printf '%s\n' "${ASSETS[@]}" | grep -Fxq "$name"; then
      echo "  ok  $name"
    else
      echo "  MISSING $name" >&2
      missing=1
    fi
  done
done

if [[ "$missing" -ne 0 ]]; then
  echo "ERROR: release ${TAG} is incomplete — refuse to publish registries" >&2
  exit 1
fi

# Refuse draft releases
DRAFT=$(gh release view "$TAG" --json isDraft -q .isDraft)
if [[ "$DRAFT" == "true" ]]; then
  echo "ERROR: release ${TAG} is still a draft" >&2
  exit 1
fi

echo "Release ${TAG} looks complete (${#TARGETS[@]} platforms + checksums)."
