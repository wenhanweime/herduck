#!/usr/bin/env node
// Thin launcher: resolves the platform package that ships the real binary and execs it.
"use strict";
const { spawnSync } = require("node:child_process");
const path = require("node:path");

const platformPackage = `herduck-${process.platform}-${process.arch}`;

let binary;
try {
  binary = path.join(path.dirname(require.resolve(`${platformPackage}/package.json`)), "herduck");
} catch {
  process.stderr.write(
    `herduck: no prebuilt binary for ${process.platform}-${process.arch}.\n` +
      `Expected optional dependency ${platformPackage}. Reinstall with optional dependencies enabled,\n` +
      `or build from source: https://github.com/wenhanweime/herduck#install-from-source\n`,
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  process.stderr.write(`herduck: failed to start ${binary}: ${result.error.message}\n`);
  process.exit(1);
}
process.exit(result.status ?? 1);
