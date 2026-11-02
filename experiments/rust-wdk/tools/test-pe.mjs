import assert from 'node:assert/strict';
import { inspectPe } from './pe-check.mjs';

function fixture() {
  const b = Buffer.alloc(1024);
  b.write('MZ'); b.writeUInt32LE(128, 60); b.write('PE\0\0',128);
  b.writeUInt16LE(0x8664,132); b.writeUInt16LE(1,134); b.writeUInt16LE(240,148);
  b.writeUInt16LE(0x20b,152); b.writeUInt32LE(0x1000,168);
  b.writeUInt32LE(4096,184); b.writeUInt16LE(1,220); b.writeUInt16LE(0x1c0,222);
  b.writeUInt32LE(16,260);
  b.write('.text',392); b.writeUInt32LE(512,400); b.writeUInt32LE(0x1000,404);
  b.writeUInt32LE(512,408); b.writeUInt32LE(512,412); b.writeUInt32LE(0x60000020,428);
  return b;
}
let count=0;
assert.equal(inspectPe(fixture()).machine,'amd64'); ++count;
for(const mutate of [
  b=>b.writeUInt16LE(0x14c,132), b=>b.writeUInt16LE(3,220),
  b=>b.writeUInt16LE(0,222), b=>b.writeUInt32LE(0xe0000020,428),
  b=>b.writeUInt32LE(512,184), b=>b.writeUInt32LE(0,168),
  b=>b.writeUInt32LE(0xfffffff0,60), b=>b.writeUInt16LE(65535,134),
  b=>b.writeUInt32LE(512,296), b=>b.writeUInt32LE(0x1100,272),
]) { const b=fixture(); mutate(b); assert.throws(()=>inspectPe(b)); ++count; }
for (const length of [0,63,151,391,431,600]) { assert.throws(()=>inspectPe(fixture().subarray(0,length))); ++count; }
// Real import directory: IMAGE_IMPORT_DESCRIPTOR name RVA at +12, then terminator.
const b=fixture(); b.writeUInt32LE(0x1000,272); b.writeUInt32LE(40,276);
b.writeUInt32LE(0x1040,524); b.write('ntoskrnl.exe\0',576);
assert.deepEqual(inspectPe(b).imports,['ntoskrnl.exe']); ++count;
b.write('kernel32.dll\0',576); assert.throws(()=>inspectPe(b)); ++count;
console.log(`D09 PE precheck: ${count} nonzero cases passed`);
