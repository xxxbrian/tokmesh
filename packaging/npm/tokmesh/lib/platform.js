'use strict';

/** Map node (process.platform, process.arch) → GitHub release asset target. */
function resolveTarget(platform = process.platform, arch = process.arch) {
  // Prefer musl static builds on Linux (run on glibc + Alpine).
  const table = {
    'linux-x64': {
      rustTarget: 'x86_64-unknown-linux-musl',
      binary: 'tokmesh',
      ext: 'tar.gz',
    },
    'linux-arm64': {
      rustTarget: 'aarch64-unknown-linux-musl',
      binary: 'tokmesh',
      ext: 'tar.gz',
    },
    'darwin-x64': {
      rustTarget: 'x86_64-apple-darwin',
      binary: 'tokmesh',
      ext: 'tar.gz',
    },
    'darwin-arm64': {
      rustTarget: 'aarch64-apple-darwin',
      binary: 'tokmesh',
      ext: 'tar.gz',
    },
    'win32-x64': {
      rustTarget: 'x86_64-pc-windows-msvc',
      binary: 'tokmesh.exe',
      ext: 'zip',
    },
    'win32-arm64': {
      rustTarget: 'aarch64-pc-windows-msvc',
      binary: 'tokmesh.exe',
      ext: 'zip',
    },
  };
  const key = `${platform}-${arch}`;
  const hit = table[key];
  if (!hit) {
    const supported = Object.keys(table).join(', ');
    throw new Error(
      `tokmesh: unsupported platform ${key}. Supported: ${supported}\n` +
        'Or install via: cargo install tokmesh / pipx install tokmesh / mise use github:xxxbrian/tokmesh',
    );
  }
  return { key, ...hit };
}

function assetName(version, rustTarget, ext) {
  return `tokmesh-${version}-${rustTarget}.${ext}`;
}

function releaseUrl(version, rustTarget, ext, repo = 'xxxbrian/tokmesh') {
  const tag = version.startsWith('v') ? version : `v${version}`;
  const ver = tag.slice(1);
  const name = assetName(ver, rustTarget, ext);
  return `https://github.com/${repo}/releases/download/${tag}/${name}`;
}

module.exports = { resolveTarget, assetName, releaseUrl };
