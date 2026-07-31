'use strict';

/**
 * Download the native tokmesh binary for this platform from GitHub Releases
 * into vendor/ next to this package. Invoked as postinstall; also callable
 * from the CLI wrapper if the binary is missing.
 */

const fs = require('fs');
const path = require('path');
const https = require('https');
const http = require('http');
const { execFileSync } = require('child_process');
const { resolveTarget, releaseUrl } = require('./platform');

const ROOT = path.join(__dirname, '..');
const VENDOR = path.join(ROOT, 'vendor');
const PKG = require('../package.json');

function binaryPath() {
  const { binary } = resolveTarget();
  return path.join(VENDOR, binary);
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith('https') ? https : http;
    const req = client.get(
      url,
      {
        headers: {
          'User-Agent': 'tokmesh-npm-install',
          Accept: 'application/octet-stream',
        },
      },
      (res) => {
        // Follow redirects (GitHub release assets)
        if (
          res.statusCode >= 300 &&
          res.statusCode < 400 &&
          res.headers.location
        ) {
          res.resume();
          download(res.headers.location, dest).then(resolve, reject);
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(
            new Error(
              `download failed HTTP ${res.statusCode} for ${url}\n` +
                'Is the GitHub repo public and does the release asset exist?',
            ),
          );
          return;
        }
        const out = fs.createWriteStream(dest);
        res.pipe(out);
        out.on('finish', () => out.close(() => resolve(dest)));
        out.on('error', reject);
      },
    );
    req.on('error', reject);
  });
}

function extractArchive(archivePath, ext, outDir) {
  ensureDir(outDir);
  if (ext === 'zip') {
    // Prefer system unzip; fall back to PowerShell on Windows.
    try {
      execFileSync('unzip', ['-o', archivePath, '-d', outDir], {
        stdio: 'ignore',
      });
      return;
    } catch {
      /* try powershell */
    }
    execFileSync(
      'powershell.exe',
      [
        '-NoProfile',
        '-Command',
        `Expand-Archive -Path '${archivePath.replace(/'/g, "''")}' -DestinationPath '${outDir.replace(/'/g, "''")}' -Force`,
      ],
      { stdio: 'inherit' },
    );
    return;
  }
  execFileSync('tar', ['-xzf', archivePath, '-C', outDir], { stdio: 'ignore' });
}

async function install({ force = false } = {}) {
  const version = PKG.version;
  const target = resolveTarget();
  const destBin = path.join(VENDOR, target.binary);

  if (!force && fs.existsSync(destBin)) {
    return destBin;
  }

  ensureDir(VENDOR);
  const url = releaseUrl(version, target.rustTarget, target.ext);
  const archive = path.join(
    VENDOR,
    `tokmesh-${version}-${target.rustTarget}.${target.ext}`,
  );

  process.stderr.write(`tokmesh: downloading ${url}\n`);
  await download(url, archive);
  extractArchive(archive, target.ext, VENDOR);

  // Normalize nested extracts
  if (!fs.existsSync(destBin)) {
    const found = walkFind(VENDOR, target.binary);
    if (found && found !== destBin) {
      fs.renameSync(found, destBin);
    }
  }
  if (!fs.existsSync(destBin)) {
    throw new Error(`tokmesh: binary ${target.binary} missing after extract`);
  }
  if (process.platform !== 'win32') {
    fs.chmodSync(destBin, 0o755);
  }
  try {
    fs.unlinkSync(archive);
  } catch {
    /* ignore */
  }
  process.stderr.write(`tokmesh: installed native binary → ${destBin}\n`);
  return destBin;
}

function walkFind(dir, name) {
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name);
    if (ent.isFile() && ent.name === name) return p;
    if (ent.isDirectory()) {
      const hit = walkFind(p, name);
      if (hit) return hit;
    }
  }
  return null;
}

module.exports = { install, binaryPath, VENDOR };

if (require.main === module) {
  install().catch((err) => {
    // postinstall should not hard-fail the whole npm install in every case,
    // but for a CLI the binary is required — warn and exit 0 so offline
    // installs still succeed; first `tokmesh` run will retry.
    console.error(String(err && err.stack ? err.stack : err));
    console.error(
      'tokmesh: postinstall could not fetch the binary; `tokmesh` will retry on first run.',
    );
    process.exit(0);
  });
}
