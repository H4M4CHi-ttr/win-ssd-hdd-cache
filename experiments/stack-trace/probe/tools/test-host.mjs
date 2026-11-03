import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { verifySuite } from './suite-proof.mjs';

const probe = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const parent = path.dirname(probe);
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--suite' || !['wire','recorder','driver','all'].includes(args[1])) {
  throw new Error('P04_HOST_USAGE');
}
const temporary = mkdtempSync(path.join(tmpdir(), 'p04-host-'));
const windows = process.platform === 'win32';
const compiler = windows ? 'cl.exe' : 'cc';
function run(exe, argv) {
  const result = spawnSync(exe, argv, { cwd: probe, encoding: 'utf8', timeout: 120000, maxBuffer: 8 * 1024 * 1024 });
  process.stdout.write(result.stdout ?? ''); process.stderr.write(result.stderr ?? '');
  if (result.error || result.status !== 0) throw new Error(`P04_HOST_COMMAND_FAILED ${exe}`);
  return result.stdout;
}
if (!windows) run(compiler, ['--version']);
for (const suite of args[1] === 'all' ? ['wire','recorder','driver'] : [args[1]]) {
  const sources = suite === 'wire' ? ['host/wire_vectors.c','common/wire.c']
    : suite === 'recorder' ? ['host/recorder_vectors.c','common/recorder.c']
      : ['host/driver_schedules.c','host/ddi_fake.c','host/platform.c','common/wire.c','common/recorder.c','../wdk/upper_snapshot.c'];
  const includes = ['include', path.join(parent, 'include')];
  if (suite === 'driver') includes.unshift('host/fake-wdk', 'host');
  const executable = path.join(temporary, suite + (windows ? '.exe' : ''));
  const flags = windows ? ['/nologo','/std:c17','/W4','/WX','/MT','/GS','/O2',...includes.map(i=>'/I'+i),`/Fe:${executable}`,`/Fo:${temporary}${path.sep}`]
    : ['-std=c17','-Wall','-Wextra','-Werror','-O2',...includes.map(i=>'-I'+i),'-o',executable];
  if (suite === 'driver') flags.push(...(windows ? ['/D_KERNEL_MODE','/D_AMD64_'] : ['-D_KERNEL_MODE','-D_AMD64_','-fshort-wchar','-pthread']));
  run(compiler, [...flags,...sources]);
  const output = run(executable, []);
  verifySuite(suite, output);
  if (suite === 'driver') {
    // driver_schedules includes the actual TU; it is not separately linked twice.
    if (!readFileSync(path.join(probe,'host/driver_schedules.c'),'utf8').includes('#include "../wdk/driver.c"')) throw new Error('P04_ACTUAL_SOURCE_LINK_MISSING');
    for (const source of ['wdk/driver.c','common/wire.c','common/recorder.c','../wdk/upper_snapshot.c']) {
      const digest = createHash('sha256').update(readFileSync(path.resolve(probe,source))).digest('hex');
      console.log(`P04_HOST_SOURCE ${source} sha256=${digest}`);
    }
  }
}
