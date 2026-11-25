import test from 'node:test';
import assert from 'node:assert/strict';
import { inspectImports, verifyUserLink } from './user-link-proof.mjs';

// Fixed PE32+ layout. Required symbols go in the thunk table; text in the
// section without a referring thunk must never count as an import.
function tinyImage(symbols) {
  const image = Buffer.alloc(0x3000);
  const pe = 0x80, opt = pe + 24, section = opt + 240, raw = 0x200, rva = 0x1000;
  image.write('MZ'); image.writeUInt32LE(pe, 0x3c);
  image.write('PE\0\0', pe); image.writeUInt16LE(0x8664, pe + 4);
  image.writeUInt16LE(1, pe + 6); image.writeUInt16LE(240, pe + 20);
  image.writeUInt16LE(0x20b, opt); image.writeUInt32LE(rva + 512, opt + 16);
  image.writeUInt16LE(3, opt + 68); image.writeUInt32LE(16, opt + 108);
  image.writeUInt32LE(rva, opt + 120); image.writeUInt32LE(60, opt + 124);
  image.writeUInt32LE(rva, section + 12); image.writeUInt32LE(0x2800, section + 16);
  image.writeUInt32LE(raw, section + 20);
  let start = 0x900;
  for (const [group, library] of ['kernel32.dll', 'bcrypt.dll'].entries()) {
    const desc = raw + group * 20, names = 0x70 + group * 24, thunks = 0x200 + group * 0x180;
    image.writeUInt32LE(rva + thunks, desc); image.writeUInt32LE(rva + names, desc + 12);
    image.writeUInt32LE(rva + thunks, desc + 16); image.write(library + '\0', raw + names);
    const imports = symbols.filter(name => name.startsWith('BCrypt') === (group === 1));
    for (const [i, name] of imports.entries()) {
      image.writeBigUInt64LE(BigInt(rva + start), raw + thunks + i * 8);
      image.write(name + '\0', raw + start + 2); start += name.length + 3;
    }
  }
  return image;
}
const launcher = ['CreateEventW', 'DuplicateHandle', 'SetEvent', 'CreatePipe',
  'CreateProcessW', 'UpdateProcThreadAttribute', 'WaitForSingleObject',
  'GetFileInformationByHandleEx', 'BCryptGenRandom'];
const listing = '8664 machine (x64)\n20B magic # (PE32+)\n3 subsystem (Windows CUI)\nSection contains the following imports:\n'
  + launcher.map(symbol => '  C1 ' + symbol).join('\n') + '\n';

test('user link checks the actual import thunks as well as the separate inspection', () => {
  const bytes = tinyImage(launcher);
  assert.deepEqual([...inspectImports(bytes).keys()].sort(), [...launcher].sort());
  const ok = verifyUserLink('launcher', bytes, listing);
  assert.equal(ok.executed, false); assert.match(ok.sha256, /^[0-9a-f]{64}$/);
  const decoy = tinyImage(launcher.filter(n => n !== 'SetEvent'));
  decoy.write('SetEvent\0', 0x2200);
  assert.throws(() => verifyUserLink('launcher', decoy, listing));
  assert.throws(() => verifyUserLink('launcher', bytes, listing.replace('C1 SetEvent','C1 NotSetEvent')));
  assert.throws(() => verifyUserLink('collector', bytes, listing));
  const impostor=Buffer.from(bytes); impostor.write('otherdll.dll',0x270);
  assert.throws(() => verifyUserLink('launcher', impostor, listing));
});
test('static proof rejects corrupted tables and the wrong architecture/subsystem', () => {
  for (const change of [b=>b.writeUInt16LE(0x14c,0x84), b=>b.writeUInt16LE(1,0x98+68),
    b=>b.writeUInt32LE(0xffffff00,0x200+12), b=>b.writeBigUInt64LE(0xfffffff0n,0x400)]) {
    const bytes=tinyImage(launcher);change(bytes);assert.throws(()=>inspectImports(bytes));
  }
});
