// Static build-artifact precheck, NOT a Windows loader or HVCI qualification.
import fs from 'node:fs';
import { pathToFileURL } from 'node:url';

export function inspectPe(b) {
  const fail=()=>{ throw new Error('D09_PE_REJECTED'); };
  const need=(p,n)=>{ if(!Number.isSafeInteger(p)||p<0||n<0||p>b.length||n>b.length-p) fail(); };
  const u16=p=>{need(p,2);return b.readUInt16LE(p);};
  const u32=p=>{need(p,4);return b.readUInt32LE(p);};
  if(!Buffer.isBuffer(b)||b.length>64*1024*1024) fail();
  need(0,64); if(b.toString('ascii',0,2)!=='MZ') fail();
  const pe=u32(60); need(pe,24);
  if(b.toString('ascii',pe,pe+4)!=='PE\0\0'||u16(pe+4)!==0x8664) fail();
  const count=u16(pe+6), size=u16(pe+20), opt=pe+24;
  if(count<1||count>96||size!==240) fail(); need(opt,size);
  if(u16(opt)!==0x20b||u16(opt+68)!==1||u32(opt+16)===0||u32(opt+32)!==4096) fail();
  if((u16(opt+70)&0x1c0)!==0x1c0||u32(opt+108)!==16) fail();
  // Un-signed research images only; no certificate directory.
  if(u32(opt+112+4*8)!==0||u32(opt+116+4*8)!==0) fail();
  // This probe never uses delayed imports. Reject both mismatched and complete
  // descriptors rather than let a second import path bypass the allowlist.
  if(u32(opt+112+13*8)!==0||u32(opt+116+13*8)!==0) fail();
  const sections=[];
  for(let i=0;i<count;i++) {
    const p=opt+size+40*i; need(p,40);
    const raw=u32(p+16), file=u32(p+20), rva=u32(p+12), flags=u32(p+36);
    need(file,raw);
    if((flags&0x20000000)!==0&&(flags&0x80000000)!==0) fail();
    sections.push({rva,raw,file,flags});
  }
  const map=(rva,length)=>{
    const matches=sections.filter(s=>rva>=s.rva&&rva-s.rva<=s.raw&&length<=s.raw-(rva-s.rva));
    if(matches.length!==1) fail(); return matches[0].file+rva-matches[0].rva;
  };
  // Entry must be backed by an executable, non-writable section.
  const entry=u32(opt+16); map(entry,1);
  if(!sections.some(s=>entry>=s.rva&&entry-s.rva<s.raw&&(s.flags&0x20000000)!==0)) fail();
  const imports=[], importRva=u32(opt+120), importSize=u32(opt+124);
  if((importRva===0)!==(importSize===0)) fail();
  if(importRva!==0) {
    if(importSize<20||importSize>2560) fail();
    let ended=false;
    for(let offset=0;offset+20<=importSize;offset+=20) {
      const p=map(importRva+offset,20);
      if([0,4,8,12,16].every(n=>u32(p+n)===0)) {ended=true;break;}
      const nameRva=u32(p+12); let name='', terminated=false;
      for(let i=0;i<256;i++) {
        const c=b[map(nameRva+i,1)];
        if(c===0) {terminated=true;break;}
        if(c<32||c>126) fail(); name+=String.fromCharCode(c);
      }
      if(!terminated||!['ntoskrnl.exe','hal.dll'].includes(name.toLowerCase())) fail();
      imports.push(name.toLowerCase());
    }
    if(!ended) fail();
  }
  return {machine:'amd64',subsystem:'native',nx:true,dynamicBase:true,integrityCheck:true,unsigned:true,sections:count,imports};
}

if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  if(process.argv.length!==3) throw new Error('D09_PE_USAGE');
  const path=process.argv[2];
  if(fs.statSync(path).size>64*1024*1024) throw new Error('D09_PE_TOO_LARGE');
  console.log(JSON.stringify(inspectPe(fs.readFileSync(path))));
}
