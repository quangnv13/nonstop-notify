import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const allowedRustLicenses = new Set([
  '0BSD', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'CC0-1.0', 'ISC',
  'MIT', 'MIT-0', 'MPL-2.0', 'Unicode-3.0', 'Unlicense', 'Zlib',
]);
const knownDisallowedRustLicenses = new Set(['LGPL-2.1-or-later']);
const allowedRustExceptions = new Set(['LLVM-exception']);
const allowedRustSources = new Set(['registry+https://github.com/rust-lang/crates.io-index']);
const allowedNpmLicenses = new Set(['0BSD', 'Apache-2.0', 'BSD-3-Clause', 'ISC', 'MIT', 'MPL-2.0']);

function evaluateRustLicense(expression) {
  const normalized = expression.replaceAll('/', ' OR ');
  const tokens = normalized.match(/\(|\)|\bAND\b|\bOR\b|\bWITH\b|[A-Za-z0-9.+-]+/g) ?? [];
  const residue = normalized.replace(/\(|\)|\bAND\b|\bOR\b|\bWITH\b|[A-Za-z0-9.+-]+|\s+/g, '');
  assert.equal(residue, '', `unsupported license syntax: ${expression}`);
  let index = 0;
  const unknown = new Set();

  function parsePrimary() {
    if (tokens[index] === '(') {
      index += 1;
      const value = parseOr();
      assert.equal(tokens[index], ')', `unclosed license group: ${expression}`);
      index += 1;
      return value;
    }
    const license = tokens[index++];
    assert(license, `empty license expression: ${expression}`);
    if (!allowedRustLicenses.has(license) && !knownDisallowedRustLicenses.has(license)) unknown.add(license);
    return allowedRustLicenses.has(license);
  }

  function parseWith() {
    let value = parsePrimary();
    if (tokens[index] === 'WITH') {
      index += 1;
      const exception = tokens[index++];
      assert(exception, `missing license exception: ${expression}`);
      if (!allowedRustExceptions.has(exception)) unknown.add(exception);
      value &&= allowedRustExceptions.has(exception);
    }
    return value;
  }

  function parseAnd() {
    let value = parseWith();
    while (tokens[index] === 'AND') {
      index += 1;
      value = parseWith() && value;
    }
    return value;
  }

  function parseOr() {
    let value = parseAnd();
    while (tokens[index] === 'OR') {
      index += 1;
      value = parseAnd() || value;
    }
    return value;
  }

  const allowed = parseOr();
  assert.equal(index, tokens.length, `unexpected license token: ${tokens[index]}`);
  return { allowed, unknown: [...unknown] };
}

function selfCheck() {
  assert.deepEqual(evaluateRustLicense('MIT OR Apache-2.0'), { allowed: true, unknown: [] });
  assert.deepEqual(evaluateRustLicense('(Apache-2.0 OR MIT) AND BSD-3-Clause'), { allowed: true, unknown: [] });
  assert.deepEqual(evaluateRustLicense('Apache-2.0 WITH LLVM-exception'), { allowed: true, unknown: [] });
  assert.deepEqual(evaluateRustLicense('MIT OR LGPL-2.1-or-later'), { allowed: true, unknown: [] });
  assert.deepEqual(evaluateRustLicense('Unknown-License OR MIT'), { allowed: true, unknown: ['Unknown-License'] });
  assert.equal(evaluateRustLicense('GPL-3.0-only').allowed, false);
  assert.equal(auditRust({
    workspace_members: ['local'],
    packages: [
      { id: 'local', name: 'local', version: '1.0.0', license: 'MIT', source: null },
      { id: 'remote', name: 'remote', version: '1.0.0', license: null, source: 'git+https://example.com/repo' },
    ],
  }).issues.length, 2);
  assert.equal(auditNpm({
    packages: {
      '': {},
      'node_modules/ok': { license: 'MIT' },
      'node_modules/missing': {},
      'node_modules/gpl': { license: 'GPL-3.0-only' },
    },
  }).issues.length, 2);
  console.log('License audit self-check passed (8 assertions).');
}

function loadCargoMetadata() {
  const result = spawnSync('cargo', ['metadata', '--locked', '--format-version', '1'], {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.status !== 0) {
    process.stderr.write(result.stderr || result.error?.message || 'cargo metadata failed\n');
    process.exit(1);
  }
  return JSON.parse(result.stdout);
}

function auditRust(metadata) {
  const issues = [];
  const workspaceMembers = new Set(metadata.workspace_members);
  for (const packageInfo of metadata.packages) {
    const label = `${packageInfo.name}@${packageInfo.version}`;
    if (!packageInfo.license) issues.push(`${label}: missing license`);
    else if (/\b(?:AGPL|GPL)-/i.test(packageInfo.license)) issues.push(`${label}: forbidden license ${packageInfo.license}`);
    else {
      try {
        const result = evaluateRustLicense(packageInfo.license);
        if (result.unknown.length) issues.push(`${label}: unknown license ${result.unknown.join(', ')}`);
        else if (!result.allowed) issues.push(`${label}: disallowed license ${packageInfo.license}`);
      } catch (error) {
        issues.push(`${label}: ${error.message}`);
      }
    }
    if (packageInfo.source === null) {
      if (!workspaceMembers.has(packageInfo.id)) issues.push(`${label}: unknown local source`);
    } else if (!allowedRustSources.has(packageInfo.source)) {
      issues.push(`${label}: disallowed source ${packageInfo.source}`);
    }
  }
  return { count: metadata.packages.length, issues };
}

function auditNpm(lock = JSON.parse(readFileSync(join(repoRoot, 'ui-react', 'package-lock.json'), 'utf8'))) {
  const packages = Object.entries(lock.packages ?? {}).filter(([path]) => path !== '');
  const issues = [];
  for (const [path, packageInfo] of packages) {
    if (!packageInfo.license) issues.push(`${path}: missing license`);
    else if (!allowedNpmLicenses.has(packageInfo.license)) issues.push(`${path}: disallowed license ${packageInfo.license}`);
  }
  return { count: packages.length, issues };
}

if (process.argv.includes('--self-check')) {
  selfCheck();
  process.exit(0);
}

const rust = auditRust(loadCargoMetadata());
const npm = auditNpm();
const issues = [...rust.issues, ...npm.issues];
console.log(`License audit: ${rust.count} Rust packages, ${npm.count} npm packages, ${issues.length} issues.`);
if (issues.length) {
  for (const issue of issues) console.error(`- ${issue}`);
  process.exitCode = 1;
}
