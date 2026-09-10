#!/usr/bin/env node
'use strict';

const { spawn } = require('node:child_process');
const { ensureBinary } = require('../lib/install.cjs');
const manifest = require('../manifest.json');

async function main() {
  const executable = await ensureBinary(manifest);
  const child = spawn(executable, process.argv.slice(2), { stdio: 'inherit' });
  const handlers = new Map();
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    const handler = () => child.kill(signal);
    handlers.set(signal, handler);
    process.on(signal, handler);
  }
  const cleanup = () => {
    for (const [signal, handler] of handlers) process.removeListener(signal, handler);
  };
  child.once('error', error => {
    cleanup();
    console.error(`herduck: could not start the native program: ${error.message}`);
    process.exitCode = 1;
  });
  child.once('exit', (code, signal) => {
    cleanup();
    if (signal) process.kill(process.pid, signal);
    else process.exitCode = code ?? 1;
  });
}

main().catch(error => {
  console.error(`herduck: ${error.message}`);
  process.exitCode = 1;
});
