// A bounded structural build check, NOT a COFF verifier, loader or safety proof.
// Reference: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
import { closeSync, constants, fstatSync, lstatSync, openSync, readSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const MAX_BYTES = 1024 * 1024;
const REQUIRED = new Set(['win_cache_p02_snapshot_submit', 'win_cache_p02_snapshot_complete']);
const UNDEFINED = new Set(['__security_cookie', '__security_check_cookie', '__chkstk', 'memcpy', 'memset']);
const HEADER_DATA = 'ExPoolZeroingNativelySupported';
const invalid = () => { throw new Error('OBJECT_INVALID'); };

export function checkObject(bytes) {
  if (!Buffer.isBuffer(bytes) || bytes.length < 20 || bytes.length > MAX_BYTES) invalid();
  const region = (offset, length) => {
    if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(length) ||
        offset < 0 || length < 0 || offset > bytes.length || length > bytes.length - offset) invalid();
  };
  if (bytes.readUInt16LE(0) !== 0x8664 || bytes.readUInt16LE(16) !== 0) invalid();
  const sections = bytes.readUInt16LE(2);
  if (sections < 1 || sections > 96) invalid();
  const headerEnd = 20 + sections * 40;
  region(0, headerEnd);
  const symbolStart = bytes.readUInt32LE(8);
  const count = bytes.readUInt32LE(12);
  if (symbolStart < headerEnd || count < 2) invalid();
  region(symbolStart, count * 18 + 4);
  const stringStart = symbolStart + count * 18;
  const stringSize = bytes.readUInt32LE(stringStart);
  if (stringSize < 4) invalid();
  region(stringStart, stringSize);
  const stringEnd = stringStart + stringSize;
  const spans = [[0, headerEnd], [symbolStart, stringEnd]];
  const raw = [];
  const addSpan = (offset, size) => {
    if (!size) return;
    if (offset < headerEnd) invalid();
    region(offset, size); spans.push([offset, offset + size]);
  };
  for (let i = 0; i < sections; ++i) {
    const p = 20 + i * 40;
    const size = bytes.readUInt32LE(p + 16);
    const offset = bytes.readUInt32LE(p + 20);
    const characteristics = bytes.readUInt32LE(p + 36);
    const uninitialized = offset === 0 && (characteristics & 0x80) !== 0;
    if (!uninitialized) addSpan(offset, size);
    raw.push({ p, size, offset, uninitialized });
    addSpan(bytes.readUInt32LE(p + 24), bytes.readUInt16LE(p + 32) * 10);
    addSpan(bytes.readUInt32LE(p + 28), bytes.readUInt16LE(p + 34) * 6);
  }
  spans.sort((a, b) => a[0] - b[0]);
  for (let i = 1; i < spans.length; ++i) if (spans[i][0] < spans[i - 1][1]) invalid();

  const nameAt = p => {
    if (bytes.readUInt32LE(p) !== 0) {
      const short = bytes.subarray(p, p + 8);
      const end = short.indexOf(0);
      return short.subarray(0, end < 0 ? 8 : end).toString('utf8');
    }
    const offset = bytes.readUInt32LE(p + 4);
    if (offset < 4 || offset >= stringSize) invalid();
    const start = stringStart + offset;
    const end = bytes.indexOf(0, start);
    if (end < start || end >= stringEnd || end - start > 128) invalid();
    return bytes.subarray(start, end).toString('utf8');
  };

  const defined = new Set();
  const undefinedNames = new Set();
  let headerData = null;
  for (let index = 0; index < count;) {
    const p = symbolStart + index * 18;
    const storage = bytes[p + 16];
    const auxiliary = bytes[p + 17];
    if (auxiliary > count - index - 1) invalid();
    // Weak/alternate external forms cannot bypass the normal-external allowlist.
    if (storage === 105 || storage === 5) invalid();
    if (storage === 2) {
      const name = nameAt(p);
      const section = bytes.readInt16LE(p + 12);
      const value = bytes.readUInt32LE(p + 8);
      const type = bytes.readUInt16LE(p + 14);
      if (defined.has(name) || undefinedNames.has(name) || (headerData && name === HEADER_DATA)) invalid();
      if (section > 0) {
        if (section > sections) invalid();
        if (name === HEADER_DATA) {
          if (type !== 0 || value !== 0 || auxiliary !== 0) invalid();
          headerData = { section, p };
        } else {
          if (!REQUIRED.has(name) || type !== 0x20) invalid();
          const data = raw[section - 1];
          if (data.uninitialized || data.offset === 0 || value >= data.size) invalid();
          defined.add(name);
        }
      } else {
        if (section !== 0 || value !== 0 || auxiliary !== 0 ||
            !UNDEFINED.has(name) || (type !== 0 && type !== 0x20)) invalid();
        undefinedNames.add(name);
      }
    }
    // Unknown non-external auxiliary metadata is bounded, not interpreted.
    index += 1 + auxiliary;
  }
  if (defined.size !== REQUIRED.size) invalid();

  if (headerData) {
    // One reviewed SDK data role, not a general COMDAT or external exception.
    const { p } = raw[headerData.section - 1];
    if (!bytes.subarray(p, p + 8).equals(Buffer.from('.bss\0\0\0\0')) ||
        bytes.readUInt32LE(p + 8) !== 0 || bytes.readUInt32LE(p + 12) !== 0 ||
        bytes.readUInt32LE(p + 16) !== 1 || bytes.readUInt32LE(p + 20) !== 0 ||
        bytes.readUInt32LE(p + 24) !== 0 || bytes.readUInt32LE(p + 28) !== 0 ||
        bytes.readUInt16LE(p + 32) !== 0 || bytes.readUInt16LE(p + 34) !== 0 ||
        bytes.readUInt32LE(p + 36) !== 0xc0101080) invalid();

    let sectionSymbols = 0;
    let definitions = 0;
    // The first pass bounded every primary/aux record. Inspect only this
    // section's role/order; unrelated static compiler metadata stays opaque.
    for (let index = 0; index < count;) {
      const s = symbolStart + index * 18;
      const auxiliary = bytes[s + 17];
      if (bytes.readInt16LE(s + 12) === headerData.section) {
        const storage = bytes[s + 16];
        if (sectionSymbols === 0 && storage !== 3) invalid();
        if (sectionSymbols === 1 && s !== headerData.p) invalid();
        if (storage === 3) {
          if (++definitions !== 1 || nameAt(s) !== '.bss' || auxiliary !== 1 ||
              bytes.readUInt32LE(s + 8) !== 0 || bytes.readUInt16LE(s + 14) !== 0) invalid();
          const a = s + 18;
          if (bytes.readUInt32LE(a) !== 1 || bytes.readUInt16LE(a + 4) !== 0 ||
              bytes.readUInt16LE(a + 6) !== 0 || bytes.readUInt32LE(a + 8) !== 0 ||
              bytes[a + 14] !== 2) invalid();
          // Number (12..13) is meaningful for associative selection5, not2;
          // 15..17 are unused. They remain bounded but uninterpreted.
        }
        ++sectionSymbols;
      }
      index += 1 + auxiliary;
    }
    if (definitions !== 1 || sectionSymbols < 2) invalid();
  }
  return {
    schema_version: 1, verdict: 'expected_snapshot_object', machine: 'amd64',
    sections, symbol_entries: count,
    defined: [...defined].sort(), header_data: headerData ? [HEADER_DATA] : [],
    undefined: [...undefinedNames].sort(),
    image_linked: false, driver_loaded: false, runtime_verified: false,
  };
}

// Only the trusted, ephemeral build artifact is expected here. The file check is
// not a sandbox or a guarantee against an attacker racing a pathname.
function readObject(path) {
  const before = lstatSync(path);
  if (!before.isFile() || before.size > MAX_BYTES) invalid();
  const fd = openSync(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0));
  try {
    const stat = fstatSync(fd);
    if (!stat.isFile() || stat.size > MAX_BYTES) invalid();
    const bytes = Buffer.alloc(MAX_BYTES + 1);
    let length = 0;
    while (length < bytes.length) {
      const read = readSync(fd, bytes, length, bytes.length - length, null);
      if (read === 0) break;
      length += read;
    }
    if (length > MAX_BYTES) invalid();
    return bytes.subarray(0, length);
  } finally {
    closeSync(fd);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    if (process.argv.length !== 3) invalid();
    process.stdout.write(JSON.stringify(checkObject(readObject(process.argv[2]))) + '\n');
  } catch {
    process.stderr.write('error: OBJECT_INVALID\n');
    process.exitCode = 1;
  }
}
