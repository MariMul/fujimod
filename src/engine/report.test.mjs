import test from 'node:test';
import assert from 'node:assert/strict';
import { WorkerEngine } from './worker-engine.ts';
import { faultFrom, stateFault } from './fault.ts';
import { crc32 } from '../archive.ts';

async function entries(blob) {
  const data=new Uint8Array(await blob.arrayBuffer()), view=new DataView(data.buffer), files={};
  let at=0;
  while (view.getUint32(at,true)===0x04034b50) {
    const size=view.getUint32(at+18,true), length=view.getUint16(at+26,true), extra=view.getUint16(at+28,true);
    const name=new TextDecoder().decode(data.subarray(at+30,at+30+length));
    const bytes=data.slice(at+30+length+extra,at+30+length+extra+size);
    assert.equal(view.getUint32(at+14,true),crc32(bytes)); files[name]=bytes;
    at+=30+length+extra+size;
  }
  return files;
}
function setup() {
  globalThis.document={baseURI:'https://example.invalid/'};
  globalThis.window={setTimeout:()=>0};
  globalThis.fetch=async()=>new Response(JSON.stringify({inputs:[]}));
  const sent=[], frames=[];
  const worker={postMessage(data) {
    sent.push(data);
    if(data.type==='boot') queueMicrotask(()=>this.reply({type:'ready',state:{virtual_ticks:120000000},
      runtime:{identity:{bundleVersion:'bundle-hash',nativeSha256:'native-hash'}}, frame:new ArrayBuffer(0)}));
  },reply(data){this.onmessage({data});},terminate(){}};
  const engine=new WorkerEngine(()=>worker,()=>{}); engine.frames(frame=>frames.push(frame));
  return {worker,engine,sent,frames};
}
test('fault report includes binary snapshot, raw details, identities and guest input times',async()=>{
  const {worker,engine,sent,frames}=setup();
  await engine.boot(new ArrayBuffer(1),{bundle:'runtime/',inputs:'inputs.json'},()=>{});
  const fault=faultFrom({type:'UnsupportedDevice',error:'unmodeled source',device:'SDIO',operation:'write',address:'0xfff80000',size:4,value:'0x7'});
  worker.reply({type:'error',fault});
  const pending=engine.errorReport(), request=sent.at(-1);
  assert.equal(request.type,'error_report');
  worker.reply({type:'error_report_result',id:request.id,checkpoint:new Uint8Array([0,1,128,255]).buffer,
    snapshot:{ticks:110000000,ram_bytes:536870912},inputs:[{action:'press',guest_ticks:119000000,options:{id:'menu'}}]});
  const files=await entries(await pending), report=JSON.parse(new TextDecoder().decode(files['report.json']));
  assert.deepEqual(files['snapshot.zip'],new Uint8Array([0,1,128,255]));
  assert.equal(report.error.details.device,'SDIO'); assert.equal(report.error.details.operation,'write');
  assert.equal(report.rawErrors[0].fault.details.error,'unmodeled source');
  assert.equal(report.identity.bundleVersion,'bundle-hash'); assert.equal(report.identity.firmwareSha256.length,64);
  assert.equal(report.inputs[0].guest_ticks,119000000); assert.equal(report.snapshot.ram_bytes,536870912);
  assert.ok(report.browser.userAgent); assert.equal(frames.length,0); engine.dispose();
});
test('a dead worker still produces a diagnostic ZIP with explicit snapshot unavailability',async()=>{
  const {worker,engine}=setup();
  await engine.boot(new ArrayBuffer(1),{bundle:'runtime/',inputs:'inputs.json'},()=>{});
  worker.onerror({message:'worker crashed',filename:'worker.js',lineno:2,colno:3});
  const files=await entries(await engine.errorReport()),report=JSON.parse(new TextDecoder().decode(files['report.json']));
  assert.equal(files['snapshot.zip'],undefined); assert.match(report.snapshot.unavailable,/unavailable/);
  assert.equal(report.identity.bundleVersion,'bundle-hash'); assert.match(JSON.stringify(report.rawErrors),/worker crashed/);
  engine.dispose();
});
test('unsupported hardware retains access fields from both native failure and Python exception',()=>{
  const fault=stateFault({failure:{error_type:'UnsupportedDevice',device:'USBControl',cpu:0,pc:'0x1234',address:'0xfff00000',size:4,operation:'read'},
    execution_error:{type:'UnsupportedDevice',error:'unknown command',cpu:null}});
  assert.equal(fault.kind,'unsupported-device'); assert.equal(fault.details.cpu,0);
  assert.equal(fault.details.device,'USBControl'); assert.equal(fault.details.address,'0xfff00000');
  assert.match(fault.message,/works on a real camera/);
});
