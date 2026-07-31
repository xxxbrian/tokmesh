#!/usr/bin/env node
'use strict';

const { spawn } = require('child_process');
const fs = require('fs');
const { install, binaryPath } = require('../lib/install');

async function main() {
  let bin = binaryPath();
  if (!fs.existsSync(bin)) {
    bin = await install({ force: true });
  }
  if (process.platform !== 'win32') {
    try {
      fs.chmodSync(bin, 0o755);
    } catch {
      /* ignore */
    }
  }
  const child = spawn(bin, process.argv.slice(2), {
    stdio: 'inherit',
    windowsHide: false,
  });
  child.on('error', (err) => {
    console.error('tokmesh: failed to start binary:', err.message);
    process.exit(1);
  });
  child.on('exit', (code, signal) => {
    if (signal) process.kill(process.pid, signal);
    else process.exit(code ?? 1);
  });
}

main().catch((err) => {
  console.error(String(err && err.message ? err.message : err));
  process.exit(1);
});
