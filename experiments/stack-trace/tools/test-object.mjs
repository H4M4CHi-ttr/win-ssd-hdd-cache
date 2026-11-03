import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { checkObject } from './check-object.mjs';

const helpers = ['win_cache_p02_snapshot_submit', 'win_cache_p02_snapshot_complete'];
const allowed = ['__security_cookie', '__security_check_cookie', '__chkstk', 'memcpy', 'memset'];

// Original literal COFF fixture, not compiler output or a driver image.
function fixture(extra = [], replace = null) {
  const symbols = replace ?? [
    { name: helpers[0], section: 1, value: 0, type: 0x20, storage: 2 },
    { name: helpers[1], section: 1, value: 4, type: 0x20, storage: 2 },
    ...extra,
  ];
  const strings = [Buffer.alloc(4)];
  const records = [];
  let stringLength = 4;
  for (const symbol of symbols) {
    const record = Buffer.alloc(18);
    if (symbol.name.length <= 8) record.write(symbol.name);
    else {
      record.writeUInt32LE(stringLength, 4);
      const name = Buffer.from(symbol.name + '\0');
      strings.push(name); stringLength += name.length;
    }
    record.writeUInt32LE(symbol.value ?? 0, 8);
    record.writeInt16LE(symbol.section ?? 0, 12);
    record.writeUInt16LE(symbol.type ?? 0, 14);
    record[16] = symbol.storage ?? 2;
    record[17] = symbol.aux?.length ?? 0;
    records.push(record, ...(symbol.aux ?? []));
  }
  strings[0].writeUInt32LE(stringLength);
  const header = Buffer.alloc(68);
  header.writeUInt16LE(0x8664, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt32LE(68, 8);
  header.writeUInt32LE(records.length, 12);
  header.write('.text', 20);
  header.writeUInt32LE(8, 36);
  header.writeUInt32LE(60, 40);
  header.writeUInt32LE(0x60000020, 56);
  header[60] = 0xc3; header[64] = 0xc3;
  return Buffer.concat([header, ...records, ...strings]);
}

function reject(buffer) {
  assert.throws(() => checkObject(buffer), { message: 'OBJECT_INVALID' });
}

test('two literal functions form a bounded object, never a runtime verdict', () => {
  assert.deepEqual(checkObject(fixture()), {
    schema_version: 1, verdict: 'expected_snapshot_object', machine: 'amd64',
    sections: 1, symbol_entries: 2,
    defined: [...helpers].sort(), undefined: [],
    image_linked: false, driver_loaded: false, runtime_verified: false,
  });
});

test('only the five compiler runtime references are permitted', () => {
  const b = fixture(allowed.map(name => ({ name })));
  assert.deepEqual(checkObject(b).undefined, [...allowed].sort());
});

test('driver lifecycle and I/O externals are not accepted by name', () => {
  for (const name of ['DriverEntry', 'GsDriverEntry', 'IoCallDriver', 'IofCallDriver', 'IoCompleteRequest', 'IoAttachDeviceToDeviceStack', 'IoCreateDevice', 'ExAllocatePool2', 'surprise']) {
    reject(fixture([{ name }]));
    reject(fixture([{ name, section: 1, type: 0x20 }]));
  }
});

test('PE and bigobj prefixes and foreign machine are rejected', () => {
  for (const machine of [0x5a4d, 0, 0x14c, 0xaa64]) {
    const b = fixture(); b.writeUInt16LE(machine); reject(b);
  }
});

test('optional headers and zero or excessive section counts are rejected', () => {
  for (const [offset, value] of [[16, 1], [2, 0], [2, 97]]) {
    const b = fixture(); b.writeUInt16LE(value, offset); reject(b);
  }
});

test('the byte cap is exact and truncated headers cannot be read', () => {
  const exact = Buffer.alloc(1024 * 1024); fixture().copy(exact);
  assert.equal(checkObject(exact).symbol_entries, 2);
  reject(Buffer.alloc(1024 * 1024 + 1));
  for (const length of [0, 1, 19, 59, 67]) reject(fixture().subarray(0, length));
});

test('symbol table pointers and counts cannot overflow or overlap headers', () => {
  for (const [offset, value] of [[8, 0], [8, 20], [8, 0xffffffff], [12, 0], [12, 0xffffffff]]) {
    const b = fixture(); b.writeUInt32LE(value, offset); reject(b);
  }
});

test('string table length offset and terminator are bounded', () => {
  for (const size of [0, 3, 0xffffffff]) {
    const b = fixture(); b.writeUInt32LE(size, 104); reject(b);
  }
  for (const offset of [0, 3, 0xffffffff]) {
    const b = fixture(); b.writeUInt32LE(offset, 72); reject(b);
  }
  const unterminated = fixture(); unterminated.fill(0x41, 108); reject(unterminated);
});

test('weak alternate-external and common symbols are rejected', () => {
  reject(fixture([{ name: 'memcpy', storage: 105 }]));
  reject(fixture([{ name: 'memcpy', storage: 5 }]));
  reject(fixture([{ name: 'memcpy', value: 1 }]));
  reject(fixture([{ name: 'memcpy', aux: [Buffer.alloc(18)] }]));
  reject(fixture([{ name: 'memcpy', section: -1 }]));
});

test('required definitions must be functions in actual bounded sections', () => {
  for (const [offset, value, width] of [[80, 0, 2], [80, 2, 2], [82, 0, 2], [76, 8, 4]]) {
    const b = fixture();
    if (width === 2) b.writeUInt16LE(value, offset); else b.writeUInt32LE(value, offset);
    reject(b);
  }
});

test('missing or duplicate helpers cannot satisfy the symbol allowlist', () => {
  reject(fixture([], [{ name: helpers[0], section: 1, type: 0x20 }]));
  reject(fixture([{ name: helpers[0], section: 1, type: 0x20 }]));
  reject(fixture([{ name: 'memcpy', section: 1, type: 0x20 }]));
});

test('auxiliary records are skipped but bounded and not mistaken for externals', () => {
  const aux = Buffer.alloc(18); aux.write('anything'); aux[16] = 105;
  const b = fixture([{ name: '.file', storage: 103, section: -2, aux: [aux] }]);
  assert.equal(checkObject(b).symbol_entries, 4);
  const truncated = fixture(); truncated[85] = 255; reject(truncated);
});

test('section raw data and relocation or line tables are bounded', () => {
  for (const offset of [40, 44, 48]) {
    const b = fixture(); b.writeUInt32LE(0xffffffff, offset);
    if (offset === 44) b.writeUInt16LE(1, 52);
    if (offset === 48) b.writeUInt16LE(1, 54);
    reject(b);
  }
  const overlap = fixture(); overlap.writeUInt32LE(68, 40); reject(overlap);
});

test('CLI handles only a bounded ordinary object file and fixed errors', () => {
  const directory = mkdtempSync(join(tmpdir(), 'p02-coff-'));
  const path = join(directory, 'synthetic.obj'); writeFileSync(path, fixture());
  const cli = fileURLToPath(new URL('./check-object.mjs', import.meta.url));
  const good = spawnSync(process.execPath, [cli, path], { encoding: 'utf8' });
  assert.equal(good.status, 0); assert.equal(good.stderr, '');
  assert.deepEqual(JSON.parse(good.stdout), checkObject(fixture()));
  for (const args of [[], [path, 'extra'], [directory], [join(directory, 'absent')]]) {
    const bad = spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' });
    assert.equal(bad.status, 1); assert.equal(bad.stdout, '');
    assert.equal(bad.stderr, 'error: OBJECT_INVALID\n');
  }
});
