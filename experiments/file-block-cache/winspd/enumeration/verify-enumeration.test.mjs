import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { verifyEnumeration } from './verify-enumeration.mjs';

const key = (name) => createHash('sha256').update(name).digest('hex');
const uses = { os: false, boot: false, pagefile: false, dump: false, dedicatedDump: false, hibernate: false };

function example() {
  const plan = { session: key('vm+boot+trial'), adapter: key('winspd'), os: key('os'), t1: key('t1'), unitSerial: key('fresh-guid') };
  const base = { adapter: key('nvme'), bytes: 8 * 2 ** 30, sector: 512, layout: 'RAW', partitions: 0, readOnly: false, uses };
  const os = { ...base, key: plan.os, serial: key('os-serial'), bytes: 200 * 2 ** 30, layout: 'GPT', partitions: 5,
    uses: { ...uses, os: true, boot: true, dump: true, pagefile: true } };
  const t1 = { ...base, key: plan.t1, serial: key('t1-serial') };
  const unit = { ...base, key: key('new-disk'), adapter: plan.adapter, serial: plan.unitSerial, bytes: 16 * 2 ** 20, readOnly: true };
  const snapshot = (phase, disks, volumes) => structuredClone({ phase, session: plan.session, complete: true, adapters: [plan.adapter], disks, volumes });
  const volumes = ['efi-volume', 'os-volume'].map(name => ({ key: key(name), disk: plan.os }));
  return { plan, before: snapshot('before', [os, t1], volumes), present: snapshot('present', [unit, t1, os], [...volumes].reverse()),
    after: snapshot('after', [t1, os], volumes) };
}

test('accepts the expected unit once, with reordered disk and volume inventories', () => {
  assert.doesNotThrow(() => verifyEnumeration(example()));
});

const failures = [
  ['another VM or boot', r => { r.after.session = key('different boot'); }],
  ['invalid phase', r => { r.after.phase = 'before'; }],
  ['incomplete volume or PnP query', r => { r.present.complete = false; }],
  ['missing adapter status', r => { r.present.adapters = []; }],
  ['two matching root adapters', r => { r.present.adapters.push(key('extra root')); }],
  ['expected identity on an existing disk', r => { r.plan.unitSerial = r.before.disks[1].serial; }],
  ['protected disk behind the proposed adapter', r => { r.before.disks[0].adapter = r.plan.adapter; }],
  ['T1 assumes a special role', r => { r.present.disks[1].uses.pagefile = true; }],
  ['existing disk changed on removal', r => { r.after.disks[0].serial = key('replacement'); }],
  ['OS or T1 replaced by a look-alike', r => { r.present.disks[2].key = key('impostor'); }],
  ['matching capacity but wrong unit serial', r => { r.present.disks[0].serial = key('wrong unit'); }],
  ['matching serial but wrong adapter', r => { r.present.disks[0].adapter = key('other parent'); }],
  ['extra disk', r => { r.present.disks.push({ ...r.present.disks[0], key: key('additional') }); }],
  ['unknown new disk write protection', r => { r.present.disks[0].readOnly = null; }],
  ['writable new disk', r => { r.present.disks[0].readOnly = false; }],
  ['partition created', r => { r.present.disks[0].partitions++; }],
  ['unexpected geometry', r => { r.present.disks[0].sector = 4096; }],
  ['unknown dump status', r => { delete r.present.disks[0].uses.dump; }],
  ['unit selected for hibernation', r => { r.present.disks[0].uses.hibernate = true; }],
  ['new volume', r => { r.present.volumes.push({ key: key('new volume'), disk: r.present.disks[0].key }); }],
  ['replaced volume with unchanged count', r => { r.after.volumes[1].key = key('replacement volume'); }],
  ['volume moved to another disk', r => { r.after.volumes[0].disk = r.plan.t1; }],
  ['volume parent unknown', r => { r.present.volumes[0].disk = null; }],
  ['unit still present after shutdown', r => { r.after.disks.push(r.present.disks[0]); }],
  ['duplicate disk identity', r => { r.present.disks[0].key = r.plan.t1; }],
  ['two disks report the same serial', r => { r.present.disks[1].serial = r.present.disks[0].serial; }],
  ['duplicated volume key', r => { r.before.volumes[1] = r.before.volumes[0]; }],
  ['unknown baseline protection', r => { r.before.disks[1].readOnly = null; }],
  ['empty baseline volume inventory', r => { r.before.volumes = []; }],
];
for (const [name, change] of failures) {
  test('rejects ' + name, () => { const record = example(); change(record); assert.throws(() => verifyEnumeration(record)); });
}

const cli = fileURLToPath(new URL('./verify-enumeration.mjs', import.meta.url));
test('stdin entry point returns a verdict without echoing private input', () => {
  const ok = spawnSync(process.execPath, [cli], { input: JSON.stringify(example()), encoding: 'utf8' });
  assert.equal(ok.status, 0);
  assert.match(ok.stdout, /PASS/);
  const bad = spawnSync(process.execPath, [cli], { input: 'PRIVATE_RAW_VALUE', encoding: 'utf8' });
  assert.equal(bad.status, 1);
  assert.match(bad.stderr, /NO-GO/);
  assert.doesNotMatch(bad.stderr + bad.stdout, /PRIVATE_RAW_VALUE/);
});
