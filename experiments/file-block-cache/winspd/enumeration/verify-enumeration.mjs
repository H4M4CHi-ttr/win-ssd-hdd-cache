// Offline checker for observations of one disposable WinSpd unit.
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const roles = ['os', 'boot', 'pagefile', 'dump', 'dedicatedDump', 'hibernate'];
const diskFields = ['key', 'serial', 'adapter', 'bytes', 'sector', 'partitions', 'layout', 'readOnly'];
const token = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const distinct = values => new Set(values).size === values.length;
const noneUsed = disk => roles.every(role => disk.uses[role] === false);

class EvidenceError extends Error {}
function requireFact(condition, message) {
  if (!condition) throw new EvidenceError(message);
}

function shape(value, fields) {
  requireFact(value !== null && typeof value === 'object' && !Array.isArray(value) &&
    Object.keys(value).length === fields.length && fields.every(key => Object.hasOwn(value, key)),
    'Missing or unexpected evidence field.');
}

function snapshot(value, phase, plan) {
  shape(value, ['phase', 'session', 'complete', 'adapters', 'disks', 'volumes']);
  requireFact(value.complete === true && value.phase === phase && value.session === plan.session,
    'Incomplete inventory, wrong phase, or session mismatch.');
  requireFact(Array.isArray(value.adapters) && value.adapters.length === 1 && value.adapters[0] === plan.adapter,
    'The expected WinSpd adapter was not uniquely observed.');
  requireFact(Array.isArray(value.volumes) && value.volumes.length > 0, 'Missing volume inventory.');
  for (const volume of value.volumes) {
    shape(volume, ['key', 'disk']);
    requireFact(token(volume.key) && token(volume.disk), 'Invalid volume identity or disk association.');
  }
  requireFact(distinct(value.volumes.map(v => v.key)), 'Volume identity is ambiguous.');
  requireFact(Array.isArray(value.disks) && value.disks.length === (phase === 'present' ? 3 : 2),
    'Unexpected disk count.');
  for (const disk of value.disks) {
    shape(disk, [...diskFields, 'uses']);
    shape(disk.uses, roles);
    requireFact([disk.key, disk.serial, disk.adapter].every(token), 'Invalid disk identity evidence.');
    requireFact(Number.isSafeInteger(disk.bytes) && disk.bytes > 0 &&
      Number.isSafeInteger(disk.sector) && disk.sector > 0 && disk.bytes % disk.sector === 0 &&
      Number.isSafeInteger(disk.partitions) && disk.partitions >= 0 && ['GPT', 'RAW'].includes(disk.layout),
      'Invalid disk geometry or layout.');
    requireFact(typeof disk.readOnly === 'boolean' && roles.every(role => typeof disk.uses[role] === 'boolean'),
      'Disk protection or special-use information is unknown.');
  }
  requireFact(distinct(value.disks.map(d => d.key)) && distinct(value.disks.map(d => d.serial)),
    'Disk identity or serial is ambiguous.');
  const disks = new Map(value.disks.map(d => [d.key, d]));
  requireFact(value.volumes.every(v => disks.has(v.disk)), 'Unknown backing disk for a volume.');
  return disks;
}

function unchanged(a, b) {
  return b && diskFields.every(field => a[field] === b[field]) && roles.every(role => a.uses[role] === b.uses[role]);
}

export function verifyEnumeration(record) {
  shape(record, ['plan', 'before', 'present', 'after']);
  const { plan, before, present, after } = record;
  shape(plan, ['session', 'os', 't1', 'adapter', 'unitSerial']);
  requireFact(Object.values(plan).every(token) && plan.os !== plan.t1, 'Invalid or ambiguous plan.');
  const initial = snapshot(before, 'before', plan);
  const live = snapshot(present, 'present', plan);
  const final = snapshot(after, 'after', plan);

  const os = initial.get(plan.os), t1 = initial.get(plan.t1);
  requireFact(os?.layout === 'GPT' && os.partitions > 0 && os.uses.os && os.uses.boot &&
    t1?.layout === 'RAW' && t1.partitions === 0 && t1.bytes === 8 * 2 ** 30 && noneUsed(t1),
    'The protected OS disk and T1 do not match the baseline.');
  for (const disk of initial.values()) {
    requireFact(disk.adapter !== plan.adapter && disk.serial !== plan.unitSerial, 'Planned unit overlaps the baseline.');
    requireFact(unchanged(disk, live.get(disk.key)) && unchanged(disk, final.get(disk.key)),
      'An existing disk changed or disappeared.');
  }
  const unit = [...live.values()].find(disk => !initial.has(disk.key));
  requireFact(unit && unit.adapter === plan.adapter && unit.serial === plan.unitSerial &&
    unit.bytes === 16 * 2 ** 20 && unit.sector === 512 && unit.layout === 'RAW' && unit.partitions === 0 &&
    unit.readOnly && noneUsed(unit), 'New unit identity, protection, geometry, or usage is incorrect.');
  requireFact(before.volumes.every(v => v.disk === plan.os), 'Unexpected baseline volume owner.');
  const baselineVolumes = new Map(before.volumes.map(v => [v.key, v.disk]));
  for (const phase of [present, after]) {
    requireFact(phase.volumes.length === baselineVolumes.size && phase.volumes.every(v => baselineVolumes.get(v.key) === v.disk),
      'Volume inventory changed.');
  }
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    verifyEnumeration(JSON.parse(readFileSync(0, 'utf8')));
    console.log('PASS: original disks and volumes preserved; one read-only unit appeared and disappeared.');
  } catch (error) {
    console.error('NO-GO: ' + (error instanceof EvidenceError ? error.message : 'Unreadable or invalid JSON input.'));
    process.exitCode = 1;
  }
}
