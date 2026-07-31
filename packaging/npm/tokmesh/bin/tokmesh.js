#!/usr/bin/env node
'use strict';

const { spawn } = require('child_process');
const fs = require('fs');

const PLATFORMS = {
  'darwin-arm64': 'tokmesh-darwin-arm64',
  'darwin-x64': 'tokmesh-darwin-x64',
  'linux-arm64': 'tokmesh-linux-arm64',
  'linux-x64': 'tokmesh-linux-x64',
  'win32-arm64': 'tokmesh-windows-arm64',
  'win32-x64': 'tokmesh-windows-x64',
};

const key = `${process.platform}-${process.arch}`;
const pkgName = PLATFORMS[key];

if (!pkgName) {
  console.error(`tokmesh: unsupported platform ${key}`);
  console.error(`Supported: ${Object.keys(PLATFORMS).join(', ')}`);
  console.error('Or install via: cargo install tokmesh / mise use github:xxxbrian/tokmesh');
  process.exit(1);
}

let binary;
try {
  binary = require(pkgName);
} catch (err) {
  console.error(`tokmesh: failed to load optional package "${pkgName}".`);
  console.error('The platform package may not have been installed.');
  console.error(err && err.message ? err.message : err);
  process.exit(1);
}

if (!fs.existsSync(binary)) {
  console.error(`tokmesh: binary missing at ${binary}`);
  process.exit(1);
}

if (process.platform !== 'win32') {
  try {
    fs.chmodSync(binary, 0o755);
  } catch {
    /* ignore */
  }
}

const child = spawn(binary, process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: false,
});

child.on('error', (err) => {
  console.error('tokmesh: failed to start binary:', err.message);
  process.exit(1);
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code ?? 1);
  }
});
