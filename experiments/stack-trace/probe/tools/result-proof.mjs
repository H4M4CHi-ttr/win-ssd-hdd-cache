import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Counts actual named results, not an intended test list. Duplicate names in
// different Rust targets are separate executions and must not be deduplicated.
export function verifyRustProof(text) {
  const rows = [...text.matchAll(/^test (?!result:)(.+) \.\.\. (.+)\r?$/gm)];
  if (!rows.length || rows.some(row => row[2].trim() !== 'ok')) throw new Error('P04_RUST_NAMED_PROOF_MISSING');
  const summaries = text.split(/\r?\n/).filter(row => row.startsWith('test result:'));
  if (!summaries.length) throw new Error('P04_RUST_SUMMARY_MISSING');
  let total = 0;
  for (const summary of summaries) {
    const match = /^test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in .+$/.exec(summary);
    if (!match) throw new Error('P04_RUST_NOT_ALL_PASSED');
    const count = Number(match[1]);
    if (!Number.isSafeInteger(count) || !Number.isSafeInteger(total + count)) throw new Error('P04_RUST_COUNT_RANGE');
    total += count;
  }
  if (total === 0 || total !== rows.length) throw new Error('P04_RUST_COUNT_MISMATCH');
  return total;
}

export function verifyTapProof(text) {
  const names = ['tests', 'pass', 'fail', 'cancelled', 'skipped', 'todo'];
  const counts = Object.create(null);
  for (const match of text.matchAll(/^# (tests|pass|fail|cancelled|skipped|todo) ([0-9]+)\s*$/gm)) {
    const count = Number(match[2]);
    if (match[1] in counts || !Number.isSafeInteger(count)) throw new Error('P04_TAP_COUNT_INVALID');
    counts[match[1]] = count;
  }
  if (names.some(name => !(name in counts)) || counts.pass === 0 || counts.pass !== counts.tests ||
      ['fail', 'cancelled', 'skipped', 'todo'].some(name => counts[name] !== 0) || /^\s*not ok\b/m.test(text)) {
    throw new Error('P04_TAP_NOT_ALL_PASSED');
  }
  return counts.pass;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.length !== 2 || !['rust', 'tap'].includes(args[0])) throw new Error('P04_RESULT_PROOF_USAGE');
  const input = readFileSync(args[1], 'utf8');
  const count = args[0] === 'rust' ? verifyRustProof(input) : verifyTapProof(input);
  console.log(`P04_RESULT_PROOF kind=${args[0]} passed=${count}`);
}
