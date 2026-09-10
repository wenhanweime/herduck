'use strict';

const fs = require('node:fs/promises');
const { createReadStream, createWriteStream } = require('node:fs');
const { createHash, randomUUID } = require('node:crypto');
const { homedir } = require('node:os');
const path = require('node:path');
const { Readable, Transform } = require('node:stream');
const { pipeline } = require('node:stream/promises');

async function sha256(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}

function platformEntry(manifest, platform, arch) {
  const key = `${platform}-${arch}`;
  if (!['darwin-x64', 'darwin-arm64', 'linux-x64', 'linux-arm64'].includes(key)) {
    throw new Error(`Unsupported platform ${key}. Herduck supports macOS and Linux on x64 or arm64.`);
  }
  const entry = manifest.platforms?.[key];
  if (!entry) throw new Error(`No binary for ${key} in this package. Install the .tgz from https://github.com/wenhanweime/herduck/releases.`);
  if (!/^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$/.test(manifest.version)
      || !/^[a-f0-9]{64}$/.test(entry.sha256)) throw new Error('Invalid release manifest.');
  const url = new URL(entry.url);
  if (url.protocol !== 'https:' || url.hostname !== 'github.com'
      || !url.pathname.startsWith(`/wenhanweime/herduck/releases/download/v${manifest.version}/`)
      || url.username || url.password || url.port || url.search || url.hash) {
    throw new Error('Invalid release download URL.');
  }
  return { key, url: url.href, sha256: entry.sha256 };
}

async function download(url, destination) {
  const signal = AbortSignal.timeout(120_000);
  const response = await fetch(url, { signal, redirect: 'follow' });
  if (!response.ok || !response.body) throw new Error(`Download failed (HTTP ${response.status}). Try again when GitHub is reachable.`);
  let size = 0;
  const limit = new Transform({
    transform(chunk, encoding, callback) {
      size += chunk.length;
      callback(size > 256 * 1024 * 1024 ? new Error('Release binary exceeds the download limit.') : null, chunk);
    },
  });
  await pipeline(Readable.fromWeb(response.body), limit,
    createWriteStream(destination, { flags: 'wx', mode: 0o600 }), { signal });
}

async function validBinary(file, expected) {
  try {
    const stat = await fs.lstat(file);
    return stat.isFile() && !stat.isSymbolicLink() && await sha256(file) === expected;
  } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
}

async function cachedBinary(directory, expected) {
  const entries = await fs.readdir(directory, { withFileTypes: true });
  // Accept the original flat cache layout too, but never replace an existing executable.
  const candidates = [path.join(directory, 'herduck'), ...entries
    .filter(entry => entry.isDirectory() && entry.name.startsWith('install-'))
    .map(entry => path.join(directory, entry.name, 'herduck'))];
  for (const executable of candidates) {
    if (await validBinary(executable, expected)) {
      await fs.chmod(executable, 0o755);
      return executable;
    }
  }
}

async function ensureBinary(manifest, options = {}) {
  const { platform = process.platform, arch = process.arch,
    installRoot = path.join(process.env.XDG_DATA_HOME || path.join(homedir(), '.local', 'share'), 'herduck', 'runtime'),
    fetchBinary = download, log = message => console.error(message) } = options;
  const entry = platformEntry(manifest, platform, arch);
  // Keep the actual executable outside npm's disposable cache: the server can restart
  // and hand off live terminals after npm/npx has cleaned up the launcher package.
  const directory = path.resolve(installRoot, manifest.version, `${entry.key}-${entry.sha256}`);
  await fs.mkdir(directory, { recursive: true, mode: 0o700 });
  const cached = await cachedBinary(directory, entry.sha256);
  if (cached) return cached;
  const temporary = path.join(directory, `.download-${process.pid}-${randomUUID()}`);
  try {
    log(`herduck: downloading ${manifest.version} for ${entry.key}…`);
    await fetchBinary(entry.url, temporary);
    if (!await validBinary(temporary, entry.sha256)) throw new Error('Release checksum mismatch; downloaded file was discarded. Retry or report the release asset.');
    await fs.chmod(temporary, 0o755);
    const concurrent = await cachedBinary(directory, entry.sha256);
    if (concurrent) return concurrent;
    // An exclusive directory makes this path immutable, including during corruption repair.
    // Renaming over another install could invalidate a running Linux process's current_exe().
    // Concurrent publishers may retain duplicates; neither locks nor crash recovery are needed.
    const generation = await fs.mkdtemp(path.join(directory, 'install-'));
    const executable = path.join(generation, 'herduck');
    try {
      await fs.rename(temporary, executable);
    } catch (error) {
      await fs.rm(generation, { recursive: true, force: true });
      throw error;
    }
    return executable;
  } finally {
    await fs.rm(temporary, { force: true });
  }
}

module.exports = { ensureBinary, platformEntry, sha256 };
