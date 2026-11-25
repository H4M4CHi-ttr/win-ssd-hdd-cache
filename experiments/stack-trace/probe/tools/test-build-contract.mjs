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
test('all three actual AMD64 user boundaries receive check release link static proof and no invocation',()=>{
  const b=build();
  for(const row of ["kind = 'registration'; package = 'cache-probe-registration-win'; binary = 'registration-link-proof'; features = @()", "kind = 'collector'; package = 'cache-probe-collector'; binary = 'cache-probe-collector'; features = @('experimental-live')", "kind = 'launcher'; package = 'cache-probe-launcher'; binary = 'cache-probe-launcher'; features = @('experimental-live')"]) assert.ok(b.includes(row));
  assert.ok(b.includes("'--target', 'x86_64-pc-windows-msvc'"));
  assert.ok(b.includes("'cargo' (@('check') + $targetArguments)"));
  assert.ok(b.includes("'cargo' (@('build', '--release') + $targetArguments)"));
  assert.ok(b.includes("'dumpbin.exe' @('/headers', '/imports', $userImage)"));
  assert.match(b,/user-link-proof\.mjs/); assert.match(b,/P04_RUST_AMD64_HOST_REQUIRED/); assert.match(b,/P04_USER_LINK_BINDING_MISMATCH/);
  for(const proof of ['imageSha256','inspectionSha256','staticProof','userBoundaryLinks','namedMatrixProof','syntheticIpcProof']) assert.ok(b.includes(proof));
  assert.doesNotMatch(b,/@\('collect'|&\s+\$userImage|Invoke-Recorded\s+\$userImage|cargo[^\n]*@\('run'/);
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

test('matrix gate requires every fixed platform-applicable name once',async()=>{
  const {verifyMatrixProof, requiredNames}=await import('./matrix-proof.mjs');
  const names=requiredNames('ipc','windows');
  const make=values=>values.map(n=>`test ${n} ... ok`).join('\n')+`\ntest result: ok. ${values.length} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n`;
  const log=make(names);
  assert.equal(verifyMatrixProof('ipc','windows',log).requiredPassed,names.length);
  for(const bad of ['',log.replace(`test ${names[0]} ... ok\n`,''),log.replace('... ok','... ignored'),log+log]) assert.throws(()=>verifyMatrixProof('ipc','windows',bad));
  assert.throws(()=>verifyMatrixProof('ipc','linux',log));
  for(let index=0;index<names.length;index++)assert.throws(()=>verifyMatrixProof('ipc','windows',make(names.filter((_,i)=>i!==index))));
});

test('both event filters cover every new crate and the proof helper directory',()=>{
  for(const event of ['pull_request','push']) for(const entry of ['crates/cache-probe-registration/**','crates/cache-probe-registration-win/**','crates/cache-probe-launcher/**','crates/cache-probe-collector/**','experiments/stack-trace/**']) assert.ok(workflow().on[event].paths.includes(entry),`${event}:${entry}`);
});

test('portable jobs retain default features and Windows executes only the synthetic IPC target',()=>{
  const linux=workflow().jobs['host-linux'].steps.map(s=>s.run??'').join('\n'); const b=build();
  for(const packageName of ['cache-probe-registration','cache-probe-registration-win','cache-probe-launcher']) assert.ok(linux.includes(`-p ${packageName}`));
  assert.match(linux,/matrix-proof\.mjs portable linux/); assert.match(b,/matrix-proof\.mjs', 'portable', 'windows'/);
  assert.match(linux,/--show-output --test-threads=1/); assert.match(b,/'--show-output', '--test-threads=1'/);
  assert.match(linux,/cargo fmt --all -- --check/); assert.match(linux,/cargo clippy --workspace --all-targets --locked -- -D warnings/);
  assert.ok(b.includes("@('test', '-p', 'cache-probe-launcher', '--features', 'synthetic-ipc', '--lib', '--test', 'windows_ipc', '--release', '--locked')"));
  assert.doesNotMatch(linux,/--features|windows_ipc/);
  assert.match(linux,/test-user-link-proof\.mjs/); assert.ok(b.includes("'tools/test-user-link-proof.mjs'"));
  assert.match(b,/userSdk =/); assert.match(b,/selectedLibraryHashes = \$userSdkHashes/);
});

test('fixed coverage includes all R13 stage variants and helpers never satisfy H gates',async()=>{
  const {matrix,requiredNames}=await import('./matrix-proof.mjs'); const names=requiredNames('portable','windows');
  for(const stage of ['p','s','k','f']) {
    assert.ok(names.includes(`tests::r13a_${stage}`));
    for(const timing of ['before','after']) { assert.ok(names.includes(`tests::r13b_${stage}_${timing}`)); assert.ok(names.includes(`tests::crash_tests::r13b_${stage}_${timing}_process_kill`)); }
    for(const effect of ['effect','no_effect']) assert.ok(names.includes(`tests::r13c_${stage}_${effect}`));
    for(const fault of ['denied','type','generation','content','journal']) assert.ok(names.includes(`tests::r13d_${stage}_${fault}`));
  }
  for(const name of ['h04_stop_success_and_failure_after_release','h04_recovery_drain_success_and_failure','h04_budget_status_success_and_failure']) assert.ok(names.includes(name));
  for(const name of ['r13_readback_decider_uses_snapshot_and_call_status_only','r13_same_error_and_returned_observation_ignore_hidden_store_effect','r13_success_with_before_readback_is_contradictory_not_confirmed','r13_readback_decider_requires_fresh_protection_rights_and_exclusion']) assert.ok(names.includes(`tests::${name}`));
  for(const name of ['r03_t1_capacity_other_known_values_refused','r03_t1_capacity_unknown_refused','r03_capacity_cannot_replace_strong_identity','r04_non_ascii_protected_or_observed_ids_refused','r03_non_ascii_case_alias_cannot_hide_collision','r04_inadmissible_pnp_characters_refused','r04_pnp_admission_199_and_200_separate_from_codec','r05_empty_expected_class_or_stack_refused','r05_empty_or_unbound_function_service_refused','r09_invalid_admission_fresh_observation_stops_before_p']) assert.ok(names.includes(`tests::${name}`));
  assert.equal(names.includes('synthetic_hang_child'),false);
  assert.deepEqual(matrix.hangPoints,['open','begin','drain','popped','stop','recovery-drain','status','write','flush']);
  assert.deepEqual(matrix.hangReleases,['ok','error']);
  assert.ok(matrix.groups.find(g=>g.id==='windows-synthetic-ipc').platforms.length===1);
});

function hangFixture(){
  const prefixes={open:['Open'],begin:['Open','B1'],drain:['Open','B1','D2'],popped:['Open','B1','D2'],stop:['Open','B1','D2','S3'],'recovery-drain':['Open','B1','D2','S3','D4'],status:['Open','B1',...Array.from({length:256},(_,i)=>`D${i+2}`),'S258','T259'],write:['Open','B1','D2','S3','Close'],flush:['Open','B1','D2','S3','Close']};
  return Object.entries(prefixes).flatMap(([point,prefix])=>['ok','error'].map(release=>{
    const actual=[...prefix];
    if(point==='open'){if(release==='ok')actual.push('Close');}
    else if(point==='begin'){if(release==='ok')actual.push('S2');actual.push('Close');}
    else if(['drain','popped'].includes(point))actual.push('S3','Close');
    else if(['stop','recovery-drain','status'].includes(point)){if(release==='error')actual.push({stop:'S4','recovery-drain':'S5',status:'S260'}[point]);actual.push('Close');}
    const result=point==='open'?(release==='ok'?'abort-before-begin':'device-unavailable'):(point==='begin'&&release==='error')?'device-unavailable':(['write','flush'].includes(point)&&release==='error')?'output-error':'archive-emitted';
    return {schema:'p04-hang-case-v1',point,release,entered:true,prefix,actual,deadlineObservedAt:15000,deadlineSticky:true,lateProcessExitAt:15001,receivedBytes:0,receivedSha256:'0'.repeat(64),result,report:{schema:'p04-controlled-v1',send_result:null,host_deadline:null,process_exit:null,received_bytes:null,received_hash:null,saved:null},realDeviceAccess:false,driverInstalledOrLoaded:false,registeredOrAttached:false,runtimeQualified:false,windowsClientQualified:false};
  }));
}
test('all 18 real H branch records require independent prefixes traces deadlines and false real flags',async()=>{
  const {verifyHangVariants}=await import('./matrix-proof.mjs'); const rows=hangFixture();const text=values=>values.map(v=>JSON.stringify(v)).join('\n');
  assert.equal(verifyHangVariants(text(rows)).length,18);
  for(let index=0;index<rows.length;index++)assert.throws(()=>verifyHangVariants(text(rows.filter((_,i)=>i!==index))));
  assert.throws(()=>verifyHangVariants(text([...rows.slice(1),rows[1]])));
  for(const changed of [{deadlineObservedAt:14999},{deadlineSticky:false},{realDeviceAccess:true},{prefix:[]},{actual:['Open','B1','D2','D2','S3','Close']}])assert.throws(()=>verifyHangVariants(text(rows.map((r,i)=>i===4?{...r,...changed}:r))));
});

test('a self-consistent reduced portable run or duplicate required case cannot pass by helper counts',async()=>{
  const {verifyMatrixProof,requiredNames}=await import('./matrix-proof.mjs'); const names=requiredNames('portable','windows');
  const make=values=>values.map(n=>`test ${n} ... ok`).join('\n')+`\ntest result: ok. ${values.length} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n`+hangFixture().map(r=>JSON.stringify(r)).join('\n');
  assert.equal(verifyMatrixProof('portable','windows',make([...names,'synthetic_hang_child'])).requiredPassed,names.length);
  for(let index=0;index<names.length;index++)assert.throws(()=>verifyMatrixProof('portable','windows',make([...names.filter((_,i)=>i!==index),'synthetic_hang_child'])));
  assert.throws(()=>verifyMatrixProof('portable','windows',make([...names,names[0]])));
});

test('real SDK layout translation unit is compiled once and never linked or executed',()=>{
  const b=build();
  assert.ok(b.includes("$userSdkLayoutSource = 'host/user_sdk_layouts.c'"));
  assert.ok(b.includes("'cl.exe' @('/nologo', '/c', '/std:c17', '/W4', '/WX', '/showIncludes', $userSdkLayoutSource"));
  assert.match(b,/userSdkLayout = \$userSdkLayoutProof/); assert.match(b,/userSdkLayoutCompiled = \$userSdkLayoutCompiled/);
  assert.doesNotMatch(b,/&\s+\$userSdkLayoutObject|Invoke-Recorded\s+\$userSdkLayoutObject|\$objects\.Add\(\$userSdkLayoutObject\)/);
});
