import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { requiredCases, verifySuite } from './suite-proof.mjs';
const tools = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(tools, '../../../..');
const read = name => readFileSync(path.join(repo,name),'utf8');
const workflow = () => JSON.parse(read('.github/workflows/probe-build-proof.yml'));
const build = () => read('experiments/stack-trace/probe/tools/build.ps1');
const output = (suite,names) => names.map(n=>'P04_CASE_PASS '+n).join('\n')+`\nP04_SUITE_PASS suite=${suite} cases=${names.length}\n`;

test('all three suites require their independently named fixed schedules',()=>{
  assert.equal(requiredCases.wire.length,7); assert.equal(requiredCases.recorder.length,15); assert.equal(requiredCases.driver.length,42);
  for(const [suite,names] of Object.entries(requiredCases)) assert.equal(verifySuite(suite,output(suite,names)),names.length);
});
test('a self-consistent smaller suite cannot omit a mandatory case',()=>{
  for(const [suite,names] of Object.entries(requiredCases)) for(let i=0;i<names.length;i++) assert.throws(()=>verifySuite(suite,output(suite,names.filter((_,j)=>j!==i))));
});
test('zero duplicate mismatched summary and unknown suite are rejected',()=>{
  assert.throws(()=>verifySuite('wire',output('wire',[])));
  assert.throws(()=>verifySuite('wire',output('wire',[...requiredCases.wire,requiredCases.wire[0]])));
  assert.throws(()=>verifySuite('wire',output('wire',requiredCases.wire)+'P04_SUITE_PASS suite=wire cases=7\n'));
  assert.throws(()=>verifySuite('wire',output('recorder',requiredCases.wire)));
  assert.throws(()=>verifySuite('other',output('other',['x'])));
});
test('an additional distinct regression is accepted without weakening minimums',()=>{
  assert.equal(verifySuite('wire',output('wire',[...requiredCases.wire,'extra_regression'])),8);
});
test('workflow uses only least-privilege ordinary hosted jobs',()=>{
  const w=workflow(); assert.deepEqual(w.permissions,{contents:'read'});
  assert.deepEqual(Object.keys(w.jobs).sort(),['host-linux','wdk-source-proof']);
  assert.equal(w.jobs['host-linux']['runs-on'],'ubuntu-24.04');
  assert.equal(w.jobs['wdk-source-proof']['runs-on'],'windows-2022');
  assert.deepEqual(Object.keys(w.on).sort(),['pull_request','push']);
  assert.deepEqual(w.on.push.branches,['main']);
  for(const j of Object.values(w.jobs)) { assert.ok(j['timeout-minutes']>0 && j['timeout-minutes']<=35); assert.equal(j.permissions,undefined); }
});
test('both event filters include the direct Rust scalar-decoder dependency',()=>{
  const w=workflow();
  for(const event of ['pull_request','push']) assert.ok(w.on[event].paths.includes('crates/cache-capture-core/**'));
});
test('only pinned checkout and text JSON evidence upload actions are used',()=>{
  const w=workflow(); let uploads=0;
  for(const j of Object.values(w.jobs)) for(const s of j.steps) if(s.uses) {
    if(s.uses==='actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1') assert.equal(s.with['persist-credentials'],false);
    else { assert.equal(s.uses,'actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02'); uploads++;
      assert.equal(s.with.path,'experiments/stack-trace/probe/out/evidence/*.txt\nexperiments/stack-trace/probe/out/evidence/*.json');
      assert.equal(s.with['if-no-files-found'],'error'); assert.equal(s.with['retention-days'],14); }
  }
  assert.equal(uploads,1);
});
test('workflow has no live device invocation or credential-bearing feature',()=>{
  const text=JSON.stringify(workflow());
  assert.doesNotMatch(text,/self-hosted|pull_request_target|secrets\.|environment"|continue-on-error|always\(\).*collect/);
  assert.doesNotMatch(text,/cache-probe-collector[^\n]*collect --|pnputil|testsigning|bcdedit|sc\.exe/);
  assert.match(text,/test-host\.mjs --suite all/); assert.match(text,/test-build-contract\.mjs/);
});
test('kernel build refuses a non-Actions or non-Windows host',()=>{
  assert.match(build(),/\$env:GITHUB_ACTIONS -ne 'true' -or \$env:RUNNER_OS -ne 'Windows'/);
  assert.match(build(),/P04_BUILD_REQUIRES_EPHEMERAL_WINDOWS_ACTIONS_RUNNER/);
  assert.match(build(),/P04_CHECKOUT_SHA_MISMATCH/);
});
test('same actual driver and snapshot TUs feed host and WDK evidence',()=>{
  const b=build(); for(const s of ['wdk/driver.c','common/wire.c','common/recorder.c','../wdk/upper_snapshot.c']) assert.ok(b.includes("'"+s+"'"));
  assert.match(b,/P04_HOST_WDK_SOURCE_MISMATCH/);
  assert.match(read('experiments/stack-trace/probe/host/driver_schedules.c'),/#include "\.\.\/wdk\/driver\.c"/);
  assert.doesNotMatch(read('experiments/stack-trace/probe/wdk/driver.c'),/#\s*(if|ifdef|elif)[^\n]*(HOST|FAKE|TEST)/);
});
test('only exact kit and two public include directories are accepted',()=>{
  const p=read('experiments/stack-trace/probe/tools/header-paths.ps1');
  assert.match(p,/\$KitRoot/); assert.match(p,/\$SnapshotIncludeRoot/); assert.match(p,/\$ProbeIncludeRoot/);
  assert.match(p,/GetFullPath\(\$ReportedPath, \$CompilerBase\)/);
  assert.doesNotMatch(p,/Root = \$CompilerBase/);
  assert.match(build(),/Resolve-P04KernelHeader/);
  assert.match(read('experiments/stack-trace/probe/tools/test-header-paths.ps1'),/host\\fake-wdk\\ntddk.h/);
});
test('package digest signatures collision and compiler evidence are mandatory',()=>{
  const b=build(); for(const phrase of ['pins.json',"'nuget' @('verify', '-All'",'DOWNLOAD_DIGEST_MISMATCH','KIT_COLLISION','clSha256','linkSha256','libraries','sourceHashes','headerCount']) assert.ok(b.includes(phrase),phrase);
  assert.doesNotMatch(b,/LLVM-.*exe|Start-Process|Install-Module/);
});
test('kernel link retains GS native W^X flags and original PE checker',()=>{
  const b=build(); for(const flag of ["'/kernel'","'/GS'","'/Zl'","'/W4'","'/WX'","'/ENTRY:GsDriverEntry'","'/NXCOMPAT'","'/DYNAMICBASE'","'/INTEGRITYCHECK'","'/NODEFAULTLIB'","'Wdmsec.lib'","'BufferOverflowK.lib'"]) assert.ok(b.includes(flag),flag);
  assert.match(b,/rust-wdk\/tools\/pe-check\.mjs/); assert.match(b,/D09 PE precheck: 22 nonzero cases passed/);
});
test('evidence never promotes build execution into runtime qualification',()=>{
  const b=build(); for(const key of ['driverInstalledOrLoaded','registeredOrAttached','observedLiveRequests','realDeviceAccess','runtimeQualified','windowsClientQualified']) assert.match(b,new RegExp(key+' = \\$false'));
  assert.match(b,/kernelImageLinked = \$true/); assert.match(b,/simulatedDdiScheduling = \$true/);
  assert.doesNotMatch(b,/& \$imagePath|Invoke-Recorded \$imagePath/);
});
test('Windows live feature receives real compilation and link but no invocation',()=>{
  const b=build(); for(const verb of ['check','build']) assert.ok(b.includes(`@('${verb}', '-p', 'cache-probe-collector', '--features', 'experimental-live', '--locked')`));
  assert.doesNotMatch(b,/@\('collect'|cache-probe-collector\.exe/);
});
test('Rust proof rejects zero ignored failed or incomplete summaries',async()=>{
  const {verifyRustProof}=await import('./result-proof.mjs');
  const pass='test example ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n';
  assert.equal(verifyRustProof(pass),1);
  for(const bad of ['',pass.replace('1 passed','0 passed'),pass.replace('0 ignored','1 ignored'),pass.replace('0 failed','1 failed'),pass.replace('test result: ok.','test result: FAILED.')]) assert.throws(()=>verifyRustProof(bad));
});
test('Rust proof totals real named passes across targets without deduplicating names',async()=>{
  const {verifyRustProof}=await import('./result-proof.mjs');
  const one='test repeated_name ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n';
  const empty='test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n';
  assert.equal(verifyRustProof(one+one+empty),2);
  for(const bad of [empty,one+one.split('\n')[0],one+empty.replace('0 filtered out','1 filtered out'),one.replace('repeated_name ... ok','repeated_name ... ignored'),one.replace('repeated_name ... ok','repeated_name ... FAILED')]) assert.throws(()=>verifyRustProof(bad));
});
test('TAP proof requires complete unique nonzero all-pass summary',async()=>{
  const {verifyTapProof}=await import('./result-proof.mjs');
  const pass='TAP version 13\nok 1 - example\n1..1\n# tests 1\n# suites 0\n# pass 1\n# fail 0\n# cancelled 0\n# skipped 0\n# todo 0\n';
  assert.equal(verifyTapProof(pass),1);
  for(const bad of ['',pass.replace('# pass 1','# pass 0'),pass.replace('# skipped 0','# skipped 1'),pass+'# tests 1\n',pass.replace('# todo 0',''),pass.replace('ok 1 - example','not ok 1 - example')]) assert.throws(()=>verifyTapProof(bad));
});
