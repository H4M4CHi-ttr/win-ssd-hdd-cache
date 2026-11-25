import { readFileSync, readdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyRustProof } from './result-proof.mjs';

const tools = path.dirname(fileURLToPath(import.meta.url));
const repository = path.resolve(tools, '../../../..');
const manifestBytes = readFileSync(path.join(tools, 'matrix-cases.json'));
export const matrix = JSON.parse(manifestBytes.toString('utf8').replace(/^\uFEFF/, ''));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const fail = reason => { throw new Error(`P04_MATRIX_${reason}`); };

function groupsFor(suite, platform) {
  if (!['portable', 'ipc'].includes(suite) || !['linux', 'windows'].includes(platform)) fail('USAGE');
  const groups = matrix.groups.filter(g => g.suites.includes(suite) && g.platforms.includes(platform));
  if (!groups.length) fail('PLATFORM_NOT_APPLICABLE');
  return groups;
}
export function requiredNames(suite, platform) {
  const names = groupsFor(suite, platform).flatMap(g => g.tests.map(t => t.name));
  if (!names.length || new Set(names).size !== names.length) fail('INVALID_REQUIRED_MANIFEST');
  return names;
}
function prefix(point) {
  switch (point) {
    case 'open': return ['Open'];
    case 'begin': return ['Open', 'B1'];
    case 'drain': case 'popped': return ['Open', 'B1', 'D2'];
    case 'stop': return ['Open', 'B1', 'D2', 'S3'];
    case 'recovery-drain': return ['Open', 'B1', 'D2', 'S3', 'D4'];
    case 'status': return ['Open', 'B1', ...Array.from({ length: 256 }, (_, i) => `D${i + 2}`), 'S258', 'T259'];
    case 'write': case 'flush': return ['Open', 'B1', 'D2', 'S3', 'Close'];
    default: fail('UNKNOWN_HANG_POINT');
  }
}
export function expectedHangTrace(point, release) {
  const trace = prefix(point);
  switch (point) {
    case 'open': if (release === 'ok') trace.push('Close'); break;
    case 'begin': if (release === 'ok') trace.push('S2'); trace.push('Close'); break;
    case 'drain': case 'popped': trace.push('S3', 'Close'); break;
    case 'stop': if (release === 'error') trace.push('S4'); trace.push('Close'); break;
    case 'recovery-drain': if (release === 'error') trace.push('S5'); trace.push('Close'); break;
    case 'status': if (release === 'error') trace.push('S260'); trace.push('Close'); break;
    case 'write': case 'flush': break;
    default: fail('UNKNOWN_HANG_POINT');
  }
  return trace;
}
export function verifyHangVariants(text) {
  const rows = text.split(/\r?\n/).filter(line => line.includes('"schema":"p04-hang-case-v1"')).map(line => {
    try { return JSON.parse(line); } catch { fail('MALFORMED_HANG_JSON'); }
  });
  const required = matrix.hangPoints.flatMap(point => matrix.hangReleases.map(release => `${point}/${release}`));
  if (rows.length !== required.length) fail('HANG_VARIANT_COUNT');
  const seen = new Set();
  for (const row of rows) {
    const id = `${row.point}/${row.release}`;
    if (!required.includes(id) || seen.has(id)) fail('HANG_VARIANT_DUPLICATE_OR_UNKNOWN');
    seen.add(id);
    const result = row.point === 'open' ? (row.release === 'ok' ? 'abort-before-begin' : 'device-unavailable') :
      (row.point === 'begin' && row.release === 'error') ? 'device-unavailable' :
      (['write','flush'].includes(row.point) && row.release === 'error') ? 'output-error' : 'archive-emitted';
    if (row.entered !== true || row.deadlineObservedAt !== 15000 || row.deadlineSticky !== true || row.lateProcessExitAt !== 15001 ||
        !Number.isSafeInteger(row.receivedBytes) || row.receivedBytes < 0 || !/^[a-f0-9]{64}$/.test(row.receivedSha256) ||
        row.result !== result || row.report?.schema !== 'p04-controlled-v1' ||
        ['send_result','host_deadline','process_exit','received_bytes','received_hash','saved'].some(key => row.report[key] !== null) ||
        JSON.stringify(row.prefix) !== JSON.stringify(prefix(row.point)) || JSON.stringify(row.actual) !== JSON.stringify(expectedHangTrace(row.point, row.release)) ||
        ['realDeviceAccess','driverInstalledOrLoaded','registeredOrAttached','runtimeQualified','windowsClientQualified'].some(key => row[key] !== false)) fail('HANG_VARIANT_EVIDENCE');
  }
  return rows;
}
export function verifyMatrixProof(suite, platform, text) {
  const passed = verifyRustProof(text);
  const required = requiredNames(suite, platform);
  const actual = [...text.matchAll(/^test (?!result:)(.+) \.\.\. ok\r?$/gm)].map(m => m[1]);
  const count = name => actual.filter(value => value === name).length;
  const mismatches = required.filter(name => count(name) !== 1);
  if (mismatches.length) fail(`REQUIRED_CASE_MISSING_OR_DUPLICATE:${mismatches.map(name => `${name}=${count(name)}`).join(',')}`);
  const groups = groupsFor(suite, platform).map(g => ({ id:g.id, requiredPassed:g.tests.length,
    cases:g.tests.map(t => ({ id:t.caseId, name:t.name, fixtureVariants:t.variants, sources:g.sources })),
    helpersObserved:(g.helpers ?? []).filter(name => count(name) > 0) }));
  const hangVariants = suite === 'portable' ? verifyHangVariants(text) : [];
  return { schema:'p04-matrix-proof-v1', suite, platform, passed, requiredPassed:required.length, groups,
    hangVariantCount:hangVariants.length, hangVariants,
    notApplicable:matrix.groups.filter(g => g.suites.includes(suite) && !g.platforms.includes(platform)).map(g => g.id),
    logSha256:sha(text), coverageManifestSha256:sha(manifestBytes),
    driverInstalledOrLoaded:false, registeredOrAttached:false, observedLiveRequests:false,
    realDeviceAccess:false, runtimeQualified:false, windowsClientQualified:false };
}
function sourceHashes() {
  const files = new Set(matrix.sourceFiles);
  function walk(relative) {
    for (const entry of readdirSync(path.join(repository, relative), { withFileTypes:true })) {
      if (entry.isSymbolicLink()) fail('SOURCE_SYMLINK');
      const name = `${relative}/${entry.name}`;
      if (entry.isDirectory()) { if (!['target','.git'].includes(entry.name)) walk(name); }
      else if (/\.(rs|toml|inf)$/.test(entry.name)) files.add(name);
    }
  }
  for (const root of matrix.sourceRoots) walk(root);
  return [...files].sort().map(relative => {
    if (typeof relative !== 'string' || path.isAbsolute(relative) || relative.includes('\\') || relative.split('/').some(part => !part || part === '.' || part === '..')) fail('INVALID_SOURCE_PATH');
    return { path:relative, sha256:sha(readFileSync(path.join(repository, relative))) };
  });
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.length !== 3) fail('USAGE');
  const proof = verifyMatrixProof(args[0], args[1], readFileSync(args[2], 'utf8'));
  console.log(JSON.stringify({ ...proof, sourceHashes:sourceHashes() }, null, 2));
}
