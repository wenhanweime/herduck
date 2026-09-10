'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { createHash } = require('node:crypto');
const { spawn } = require('node:child_process');
const { once } = require('node:events');
const { ensureBinary, platformEntry } = require('../lib/install.cjs');

const bytes = Buffer.from('#!/bin/sh\nprintf "%s\\n" "$@"\n');
function manifest(content = bytes) {
  return { version: '0.1.0-alpha.2', platforms: { [`${process.platform}-${process.arch}`]: {
    url: 'https://github.com/wenhanweime/herduck/releases/download/v0.1.0-alpha.2/herduck-test',
    sha256: createHash('sha256').update(content).digest('hex'),
  } } };
}
async function fixture(t) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'herduck-npm-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  let downloads = 0;
  const options = { installRoot: directory, log() {}, fetchBinary: async (_, dest) => {
    downloads++;
    await fs.writeFile(dest, bytes, { flag: 'wx' });
  } };
  return { directory, options, downloads: () => downloads };
}

test('first download is verified, executable and reused offline', async t => {
  const f = await fixture(t);
  const executable = await ensureBinary(manifest(), f.options);
  assert.equal((await fs.stat(executable)).mode & 0o777, 0o755);
  assert.deepEqual(await fs.readFile(executable), bytes);
  assert.equal(await ensureBinary(manifest(), f.options), executable);
  assert.equal(f.downloads(), 1);
});

test('tampering creates a fresh executable; failed verification never installs bytes', async t => {
  const f = await fixture(t);
  const executable = await ensureBinary(manifest(), f.options);
  await fs.writeFile(executable, 'corrupt');
  const repaired = await ensureBinary(manifest(), f.options);
  assert.equal(f.downloads(), 2);
  assert.notEqual(repaired, executable);
  assert.deepEqual(await fs.readFile(repaired), bytes);
  assert.equal(await fs.readFile(executable, 'utf8'), 'corrupt');
  await fs.writeFile(repaired, 'corrupt');
  await assert.rejects(ensureBinary(manifest(), { ...f.options,
    fetchBinary: (_, dest) => fs.writeFile(dest, 'wrong download') }), /checksum mismatch/);
  assert.equal(await fs.readFile(repaired, 'utf8'), 'corrupt');
  await assertCleanCache([executable, repaired]);
});

async function assertCleanCache(executables) {
  const directories = [...new Set(executables.map(executable => path.dirname(executable)))];
  const cache = path.dirname(directories[0]);
  assert.deepEqual((await fs.readdir(cache)).sort(), directories.map(directory => path.basename(directory)).sort());
  for (const directory of directories) assert.deepEqual(await fs.readdir(directory), ['herduck']);
}

test('concurrent installs publish complete programs and clean temporary files', async t => {
  const f = await fixture(t);
  const results = await Promise.all(Array.from({ length: 8 }, () => ensureBinary(manifest(), f.options)));
  for (const executable of new Set(results)) assert.deepEqual(await fs.readFile(executable), bytes);
  await assertCleanCache(results);
});

for (const repair of [false, true]) {
  test(`concurrent ${repair ? 'repairs' : 'first installs'} preserve an already returned executable inode`,
    { timeout: 10000 }, async t => {
      const f = await fixture(t);
      const previous = repair ? await ensureBinary(manifest(), f.options) : undefined;
      if (previous) await fs.writeFile(previous, 'corrupt');
      let releasePublishers, releaseSecond;
      const bothPublishing = new Promise(resolve => { releasePublishers = resolve; });
      const firstReturned = new Promise(resolve => { releaseSecond = resolve; });
      t.after(() => { releasePublishers(); releaseSecond(); });
      const rename = fs.rename;
      let publications = 0;
      t.mock.method(fs, 'rename', async (...args) => {
        const publication = ++publications;
        if (publication === 2) releasePublishers();
        // Both installers have chosen to publish, but neither file is visible yet.
        await bothPublishing;
        if (publication === 2) await firstReturned;
        return rename(...args);
      });
      const installs = [ensureBinary(manifest(), f.options), ensureBinary(manifest(), f.options)];
      const first = await Promise.race(installs);
      const before = await fs.stat(first);
      // Force the second publication to happen after the first caller could launch its binary.
      releaseSecond();
      const results = await Promise.all(installs);
      const after = await fs.stat(first);
      assert.deepEqual([after.dev, after.ino], [before.dev, before.ino]);
      assert.deepEqual(await fs.readFile(first), bytes);
      const downloaded = f.downloads();
      assert.ok(results.includes(await ensureBinary(manifest(), f.options)));
      assert.equal(f.downloads(), downloaded);
      await assertCleanCache(previous ? [...results, previous] : results);
    });
}

test('interrupted download is cleaned and can be retried', async t => {
  const f = await fixture(t);
  await assert.rejects(ensureBinary(manifest(), { ...f.options, fetchBinary: async (_, dest) => {
    await fs.writeFile(dest, 'partial');
    throw new Error('network interrupted');
  } }), /network interrupted/);
  const executable = await ensureBinary(manifest(), f.options);
  await assertCleanCache([executable]);
});

test('failed publication cleans its generation and download before retrying', async t => {
  const f = await fixture(t);
  const rename = fs.rename;
  let fail = true;
  t.mock.method(fs, 'rename', async (...args) => {
    if (fail) {
      fail = false;
      throw new Error('publication interrupted');
    }
    return rename(...args);
  });
  await assert.rejects(ensureBinary(manifest(), f.options), /publication interrupted/);
  const executable = await ensureBinary(manifest(), f.options);
  assert.deepEqual(await fs.readFile(executable), bytes);
  await assertCleanCache([executable]);
});

test('abandoned downloads and empty generations cannot block another install', async t => {
  const f = await fixture(t);
  const m = manifest();
  const entry = platformEntry(m, process.platform, process.arch);
  const cache = path.join(f.directory, m.version, `${entry.key}-${entry.sha256}`);
  const abandoned = path.join(cache, 'install-abandoned');
  const partial = path.join(cache, '.download-abandoned');
  await fs.mkdir(abandoned, { recursive: true });
  await fs.writeFile(partial, 'partial');
  const executable = await ensureBinary(m, f.options);
  assert.deepEqual(await fs.readFile(executable), bytes);
  assert.equal(await ensureBinary(m, f.options), executable);
  assert.equal(f.downloads(), 1);
  // Another process may still own these paths; installation must not delete them.
  assert.deepEqual(await fs.readdir(abandoned), []);
  assert.equal(await fs.readFile(partial, 'utf8'), 'partial');
});

test('a manifest entry cannot override the validated platform key or escape the install root', async t => {
  const f = await fixture(t);
  const m = manifest();
  const key = `${process.platform}-${process.arch}`;
  m.platforms[key].key = '../../outside';
  assert.deepEqual(platformEntry(m, process.platform, process.arch), {
    key, url: m.platforms[key].url, sha256: m.platforms[key].sha256,
  });
  const executable = await ensureBinary(m, f.options);
  const relative = path.relative(f.directory, executable);
  assert.ok(!path.isAbsolute(relative) && relative.split(path.sep)[0] !== '..');
});

test('unsupported platforms, absent assets and malformed manifests give useful errors', () => {
  assert.throws(() => platformEntry(manifest(), 'win32', 'x64'), /Unsupported platform/);
  assert.throws(() => platformEntry({ platforms: {} }, 'linux', 'x64'), /No binary/);
  const m = manifest();
  m.version = '../../escape';
  assert.throws(() => platformEntry(m, process.platform, process.arch), /Invalid release manifest/);
  const bad = manifest();
  bad.platforms[`${process.platform}-${process.arch}`].url = 'https://example.com/program';
  assert.throws(() => platformEntry(bad, process.platform, process.arch), /Invalid release download URL/);
});

async function launcher(t, content) {
  const f = await fixture(t);
  const m = manifest(Buffer.from(content));
  await fs.cp(path.join(__dirname, '..', 'lib'), path.join(f.directory, 'package/lib'), { recursive: true });
  await fs.cp(path.join(__dirname, '..', 'bin'), path.join(f.directory, 'package/bin'), { recursive: true });
  await fs.writeFile(path.join(f.directory, 'package/manifest.json'), JSON.stringify(m));
  await ensureBinary(m, { ...f.options, installRoot: path.join(f.directory, 'herduck/runtime'),
    fetchBinary: (_, dest) => fs.writeFile(dest, content) });
  return { ...f, spawn: args => spawn(process.execPath, [path.join(f.directory, 'package/bin/herduck.cjs'), ...args],
    { env: { ...process.env, XDG_DATA_HOME: f.directory }, stdio: ['pipe', 'pipe', 'pipe'] }) };
}

test('launcher preserves literal arguments, stdin and native exit code', async t => {
  const f = await launcher(t, '#!/bin/sh\nprintf "%s\\n" "$@"\ncat\nexit 23\n');
  const child = f.spawn(['with spaces', '$(literal)', '--flag']);
  let output = '';
  child.stdout.on('data', data => { output += data; });
  child.stdin.end('input\n');
  const [code, signal] = await once(child, 'close');
  assert.equal(code, 23);
  assert.equal(signal, null);
  assert.equal(output, 'with spaces\n$(literal)\n--flag\ninput\n');
});

test('launcher forwards termination and does not orphan the child', { timeout: 10000 }, async t => {
  const f = await launcher(t, '#!/bin/sh\ntrap "exit 42" TERM\nprintf "ready\\n"\nwhile :; do sleep 0.1; done\n');
  const child = f.spawn([]);
  t.after(() => child.kill('SIGKILL'));
  const done = once(child, 'close');
  await once(child.stdout, 'data');
  child.kill('SIGTERM');
  const [code] = await done;
  assert.equal(code, 42);
});
