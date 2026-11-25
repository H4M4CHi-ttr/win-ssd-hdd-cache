// Non-executing proof for linked user-mode P04 binaries. The WDK image has
// a different subsystem and its own existing checker.
import { createHash } from 'node:crypto';
import { readFileSync, statSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const common = ['CreateFileW', 'DeviceIoControl', 'BCryptGenRandom'];
const required = {
  collector: [...common, 'GetFileType', 'GetProcAddress', 'SetHandleInformation',
    'WaitForSingleObject', 'PssCaptureSnapshot', 'PssWalkSnapshot', 'PssFreeSnapshot'],
  launcher: ['CreateEventW', 'DuplicateHandle', 'SetEvent', 'CreatePipe', 'CreateProcessW',
    'UpdateProcThreadAttribute', 'WaitForSingleObject', 'GetFileInformationByHandleEx', 'BCryptGenRandom'],
  registration: ['SetupDiCreateDeviceInfoList', 'SetupDiOpenDeviceInfoW', 'SetupDiGetDeviceInstanceIdW',
    'SetupDiGetDeviceRegistryPropertyW', 'SetupDiSetDeviceRegistryPropertyW',
    'SetupDiGetDevicePropertyW', 'SetupDiDestroyDeviceInfoList', 'SetupCopyOEMInfW',
    'SetupGetInfDriverStoreLocationW', 'SetupOpenInfFileW', 'SetupCloseInfFile', 'SetupInstallServicesFromInfSectionW',
    'OpenSCManagerW', 'OpenServiceW', 'QueryServiceConfigW', 'QueryServiceStatusEx', 'CloseServiceHandle',
    'RegOpenKeyExW', 'RegCreateKeyExW', 'RegSetValueExW', 'RegQueryValueExW', 'RegCloseKey',
    'CompareStringOrdinal', 'GetLastError', 'SetLastError'],
};
const maxImage = 64 * 1024 * 1024;
function reject(reason) { throw new Error('P04_USER_LINK_' + reason); }

export function inspectImports(image) {
  if (!Buffer.isBuffer(image) || image.length < 64 || image.length > maxImage) reject('SIZE');
  function bounds(pos, size) {
    if (!Number.isSafeInteger(pos) || pos < 0 || size < 0 || size > image.length - pos) reject('RANGE');
    return pos;
  }
  const u16 = pos => image.readUInt16LE(bounds(pos, 2));
  const u32 = pos => image.readUInt32LE(bounds(pos, 4));
  const pe = u32(60), opt = pe + 24;
  if (u16(0) !== 0x5a4d || u32(pe) !== 0x4550 || u16(pe + 4) !== 0x8664) reject('MACHINE');
  const count = u16(pe + 6), size = u16(pe + 20);
  if (count < 1 || count > 96 || size !== 240 || u16(opt) !== 0x20b ||
      u16(opt + 68) !== 3 || !u32(opt + 16) || u32(opt + 108) < 2) reject('HEADERS');
  const sections = [];
  for (let i = 0; i < count; i++) {
    const pos = opt + size + 40 * i;
    const base = u32(pos + 12), length = u32(pos + 16), file = u32(pos + 20);
    bounds(file, length);
    sections.push({ base, length, file });
  }
  function map(rva, n) {
    const matches = sections.filter(s => rva >= s.base && rva - s.base <= s.length - n);
    if (matches.length !== 1) reject('RVA');
    return matches[0].file + rva - matches[0].base;
  }
  function name(rva) {
    let value = '';
    for (let i = 0; i <= 512; i++) {
      const c = image[map(rva + i, 1)];
      if (!c && value) return value;
      if (c < 33 || c > 126) reject('NAME');
      value += String.fromCharCode(c);
    }
    reject('NAME_TOO_LONG');
  }
  const start = u32(opt + 120), length = u32(opt + 124), imports = new Map();
  if (!start || length < 40 || length > 65536) reject('DIRECTORY');
  for (let offset = 0; offset + 20 <= length; offset += 20) {
    const p = map(start + offset, 20);
    if ([0,4,8,12,16].every(n => !u32(p + n))) return imports;
    const dll = name(u32(p + 12)).toLowerCase();
    const thunks = u32(p) || u32(p + 16);
    if (!thunks) reject('THUNKS');
    for (let i = 0; ; i++) {
      if (i >= 8192) reject('THUNK_LIMIT');
      const value = image.readBigUInt64LE(map(thunks + i * 8, 8));
      if (value === 0n) break;
      if (value & (1n << 63n)) continue; // PE32+ import by ordinal.
      if (value > 0xffffffffn) reject('NAME_RVA');
      const symbol = name(Number(value) + 2);
      if (!imports.has(symbol)) imports.set(symbol, new Set());
      imports.get(symbol).add(dll);
    }
  }
  reject('UNTERMINATED_DIRECTORY');
}

function expectedLibrary(symbol, library) {
  if (symbol.startsWith('Setup')) return library === 'setupapi.dll';
  if (symbol.startsWith('BCrypt')) return library === 'bcrypt.dll';
  if (/^(Reg|OpenSCManager|OpenService|QueryService|CloseService)/.test(symbol)) {
    return library === 'advapi32.dll' || /^api-ms-win-(?:core-registry|service-)/.test(library);
  }
  return ['kernel32.dll', 'kernelbase.dll'].includes(library) || library.startsWith('api-ms-win-core-');
}

export function verifyUserLink(kind, image, listing) {
  if (!Object.hasOwn(required, kind)) reject('KIND');
  const imports = inspectImports(image);
  if (!/\b8664 machine \(x64\)/i.test(listing) || !/\b20B magic # \(PE32\+\)/i.test(listing) ||
      !/3 subsystem \(Windows CUI\)/i.test(listing)) reject('INSPECTION_HEADERS');
  const checked = {};
  const table = listing.split(/Section contains the following imports:/i).slice(1).join('\n');
  for (const symbol of required[kind]) {
    const libraries = [...(imports.get(symbol) ?? [])].filter(dll => expectedLibrary(symbol, dll));
    if (!libraries.length || !new RegExp('^\\s*[0-9a-fA-F]+\\s+' + symbol + '\\s*$','m').test(table)) {
      reject('MISSING_' + symbol);
    }
    checked[symbol] = libraries.sort();
  }
  return { kind, machine: 'AMD64', subsystem: 'Windows CUI', bytes: image.length,
    sha256: createHash('sha256').update(image).digest('hex'), verifiedImports: checked, executed: false };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [kind, exe, listing, ...extra] = process.argv.slice(2);
  if (!listing || extra.length || statSync(exe).size > maxImage || statSync(listing).size > maxImage) reject('INPUT');
  console.log(JSON.stringify(verifyUserLink(kind, readFileSync(exe), readFileSync(listing, 'utf8'))));
}
