import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import test from 'node:test';
import { initialize, usb } from '../dist/index.js';
import { TestExchange as E, TestScript } from '../pkg/fujimod.js';
await initialize({ module_or_path: await readFile(new URL('../pkg/fujimod_bg.wasm', import.meta.url)) });
const words = (...values) => { const data=new Uint8Array(values.length*4); const view=new DataView(data.buffer); values.forEach((n,i)=>view.setUint32(i*4,n,true)); return data; };
const concat = (...arrays) => Uint8Array.from(arrays.flatMap(a=>[...a]));
const env = (kind,size,offset=0,token=0x1234) => { const data=new Uint8Array(12); const v=new DataView(data.buffer); data[0]=kind;v.setUint32(2,offset);v.setUint32(6,size);v.setUint16(10,token);return data; };
const command = (request,maximum,response) => [E.control(env(0xd0,request.length)),E.write(request),E.control(env(0xd1,maximum)),E.read(maximum,response)];
const memoryRequest=(fn,address,length,payload=new Uint8Array())=>concat(words(0x200001,0,0,0,fn,0,0,address,length),payload);
const readMemory=(address,data)=>{const req=memoryRequest(2,address,data.length);return command(req,36+data.length,concat(req,data));};
const closeSteps=()=>[E.control(env(0xc4,2)),E.read(2,new Uint8Array([0xaa,0xbb]))];
const limits=(read=4,write=4)=>new usb.FirmwareLimits(read,write,256,256);
async function opened(steps,{packet=4,firmware=limits(),budget=1024,trace}={}) {
  const raw=new Uint8Array([0x34,0x12,0,0,0,0,0,0]);new DataView(raw.buffer).setUint32(4,packet);
  const script=new TestScript([E.control(env(0xc0,8,0,0)),E.read(8,raw),...steps]);
  const options=new usb.TransferOptions(100,budget);if(trace)options.setTrace(trace);
  const session=await script.open(firmware,options);return {session,script};
}
const kind = expected => value => value.kind===expected;
async function until(predicate){for(let n=0;n<50;n++){if(predicate())return;await new Promise(setImmediate);}assert.fail('operation did not reach expected pending stage');}

test('quantities reject negative, fractional, overflowing and nonfinite values',()=>{
  for(const value of [-1,0.5,2**32,NaN,Infinity,"1",false,null,undefined,{},2n]){
    assert.throws(()=>new usb.FirmwareLimits(value,1,1,1),kind(usb.ErrorKind.InvalidArgument));
    assert.throws(()=>new usb.research.CommandCode(value),kind(usb.ErrorKind.InvalidArgument));
    assert.throws(()=>new usb.TransferOptions(value,1024),kind(usb.ErrorKind.InvalidArgument));
  }
  assert.throws(()=>new usb.InterfaceSelection(256,0,0),kind(usb.ErrorKind.InvalidArgument));
  assert.throws(()=>new usb.app80.CommandCode(256),kind(usb.ErrorKind.InvalidArgument));
  assert.throws(()=>new usb.research.Request(new usb.app80.CommandCode(3),new Uint8Array(16),32));
});

test('one direct read, immutable metadata and raw close acknowledgement',async()=>{
  const trace=new usb.tracing.Trace(64,1024),expected=new Uint8Array([1,2,3,4]);
  const {session,script}=await opened([...readMemory(0x12345678,expected),...closeSteps()],{trace});
  assert.equal(session.state,usb.SessionState.Ready);assert.equal(session.packetBytes,4);assert.equal(session.sessionId,0x1234);
  assert.throws(()=>{session.packetBytes=128;},TypeError);
  assert.deepEqual(await session.readMemory(0x12345678,4),expected);
  assert.deepEqual(await session.close(),new Uint8Array([0xaa,0xbb]));
  assert.equal(session.state,usb.SessionState.Closed);session.abandon();assert.equal(session.state,usb.SessionState.Closed);
  assert.equal(script.remaining,0);
  const batch=trace.drain();assert.equal(batch.droppedEvents,0n);assert.equal(batch.droppedBytes,0n);
  assert.ok(batch.events.length>0);assert.equal(typeof batch.events[0].sequence,'bigint');
  assert.ok(batch.events.every(e=>e.timestampMs>1_500_000_000_000));
});

test('range ownership spans chunks and for-await break performs no next read',async()=>{
  const {session,script}=await opened([...readMemory(0x1000,new Uint8Array([1,2,3,4])),...closeSteps()]);
  const reader=usb.memory.readChunks(session,0x1000,12);
  assert.equal(session.state,usb.SessionState.ReadingRange);assert.throws(()=>session.ping(),kind(usb.ErrorKind.Busy));
  for await(const data of reader){assert.deepEqual(data,new Uint8Array([1,2,3,4]));break;}
  assert.equal(reader.bytesCompleted,4);assert.equal(session.state,usb.SessionState.Ready);
  assert.equal(script.remaining,2);await session.close();assert.equal(script.remaining,0);
});

test('bulk failure retains completed chunks and partial bytes without cleanup',async()=>{
  const {session,script}=await opened([E.control(env(0xe1,4,0)),E.read(4,new Uint8Array([1,2,3,4])),E.control(env(0xe1,4,4)),E.disconnected(4,new Uint8Array([5,6]))]);
  const reader=usb.data.readChunks(session,new usb.DataWindow(0x1000,0x1000,12,4,usb.OffsetMode.Checked),0x1000,10);
  const first=await reader.next();assert.deepEqual(first.value,new Uint8Array([1,2,3,4]));
  await assert.rejects(reader.next(),e=>{assert.equal(e.kind,usb.ErrorKind.Transfer);assert.deepEqual(e.partial,new Uint8Array([5,6]));assert.equal(e.stage,usb.TransferStage.BulkIn);assert.equal(e.cause.transferKind,usb.TransferFailureKind.Disconnected);return true;});
  assert.equal(reader.bytesCompleted,4);assert.equal(session.state,usb.SessionState.CompletionUnknown);
  await assert.rejects(async()=>session.close(),kind(usb.ErrorKind.CompletionUnknown));
  assert.equal(script.remaining,0);assert.deepEqual(first.value,new Uint8Array([1,2,3,4]));
});

test('abandon a dispatched operation prevents all further transfers',async()=>{
  const trace=new usb.tracing.Trace(64,1024);
  const {session,script}=await opened([E.control(env(0xc2,2)),E.pendingRead(2)],{trace});
  const pending=session.ping(),rejected=assert.rejects(pending,kind(usb.ErrorKind.Abandoned));
  assert.equal(session.state,usb.SessionState.InFlight);assert.throws(()=>session.ping(),kind(usb.ErrorKind.Busy));
  await until(()=>script.remaining===0);session.abandon();await rejected;
  assert.equal(session.state,usb.SessionState.Abandoned);assert.throws(()=>session.close(),kind(usb.ErrorKind.Abandoned));
  assert.ok(trace.drain().events.some(e=>e.phase===usb.tracing.Phase.Cancelled));
});

test('abandon before dispatch emits no operation transfer',async()=>{
  const {session,script}=await opened([E.control(env(0xc2,2)),E.pendingRead(2)]);
  const pending=session.ping();session.abandon();await assert.rejects(pending,kind(usb.ErrorKind.Abandoned));assert.equal(script.remaining,2);
});

test('abandon an active range read retains explicit abandoned state',async()=>{
  const {session,script}=await opened([E.control(env(0xe1,4,0)),E.pendingRead(4)]);
  const reader=usb.data.readChunks(session,new usb.DataWindow(0,0,8,4,usb.OffsetMode.Checked),0,8);
  const pending=reader.next(),rejected=assert.rejects(pending,kind(usb.ErrorKind.Abandoned));
  await until(()=>script.remaining===0);assert.throws(()=>reader.close(),kind(usb.ErrorKind.Busy));
  session.abandon();await rejected;assert.equal(session.state,usb.SessionState.Abandoned);assert.equal(reader.bytesCompleted,0);
});

test('a failed close consumes the session and cannot send another C4',async()=>{
  const {session,script}=await opened([E.control(env(0xc4,2)),E.read(2,new Uint8Array([0x12]))]);
  await assert.rejects(session.close(),e=>{assert.equal(e.kind,usb.ErrorKind.Protocol);assert.equal(e.protocolKind,usb.ProtocolFailureKind.ReplyLength);assert.equal(e.expected,2);assert.equal(e.actual,1);return true;});assert.equal(session.state,usb.SessionState.CompletionUnknown);
  assert.throws(()=>session.close(),kind(usb.ErrorKind.CompletionUnknown));assert.equal(script.remaining,0);
});

test('download Writing is an application enum without a hidden poll or clear',async()=>{
  const request=words(0x150000,0,0,0,0xf0001,0,0,0),text=Uint8Array.from(Buffer.from('WRITING\0','utf16le'));
  const {session,script}=await opened(command(request,128,concat(words(0x150000,0,0,0),text)));
  const result=await session.firmwareDownloadStatus();assert.equal(result.state,usb.FirmwareDownloadState.Writing);
  assert.equal(result.text,'WRITING');assert.equal(session.state,usb.SessionState.Ready);assert.equal(script.remaining,0);session.abandon();
});

test('research device status is a completed reply with preserved raw fields',async()=>{
  const request=new usb.research.Request(new usb.research.CommandCode(0x1a0001),new Uint8Array(16),32),raw=words(0x1a0001,1,0xffff,0,2,0,0,0);
  const {session,script}=await opened(command(words(0x1a0001,0,0,0,0,0,0,0),32,raw));
  const result=await session.researchCommand(request);
  assert.equal(result.status,usb.CommandStatus.DeviceError);assert.equal(result.primary,1);assert.equal(result.secondary,0xffff);
  assert.deepEqual(result.raw,raw);assert.equal(session.state,usb.SessionState.Ready);assert.equal(script.remaining,0);session.abandon();
});

test('upload holds ownership through setup, every piece and opaque E8',async()=>{
  const setup=words(0x760001,0,0,0,0,3,0,0),reply=words(0x760001,0,0,0,9,8,7,6);
  const steps=[...command(setup,32,reply),E.control(env(0xe0,2)),E.write(new Uint8Array([1,2])),E.control(env(0xe0,1)),E.write(new Uint8Array([3])),E.control(env(0xe8,32)),E.read(32,new Uint8Array([4,5,6,7]))];
  const {session,script}=await opened(steps),pending=usb.data.upload(session,new Uint8Array([1,2,3]),2,usb.DataWriteMode.Preserve);
  assert.throws(()=>session.ping(),kind(usb.ErrorKind.Busy));const result=await pending;
  assert.equal(result.bytesWritten,3);assert.deepEqual(result.acknowledgement,new Uint8Array([4,5,6,7]));
  assert.deepEqual(result.setup.fields,new Uint32Array([9,8,7,6]));assert.equal(script.remaining,0);session.abandon();
});

test('App80 uses explicit mailbox operations without polling',async()=>{
  const args=Uint8Array.from({length:32},(_,n)=>n),payload=concat(new Uint8Array([0x7f]),args);
  const stage=concat(words(0x1a0002,0,0,0,3,1,0,0x81,32),args),publish=concat(words(0x1a0002,0,0,0,3,1,0,0x80,33),payload),read=words(0x1a0002,0,0,0,2,1,0,0x80,33);
  const {session,script}=await opened([...command(stage,36,stage.slice(0,36)),...command(publish,36,publish.slice(0,36)),...command(read,69,concat(read,payload))]);
  const result=await usb.app80.exchange(session,new usb.app80.Mailbox(0,0x80,256),new usb.app80.Request(new usb.app80.CommandCode(0x7f),args));
  assert.deepEqual(result,payload);assert.equal(script.remaining,0);session.abandon();
});


test('invalid enum choices and address types fail before reserving or dispatching',async()=>{
  const {session,script}=await opened([]);
  for(const value of ['Product','2',false,null,undefined,{},1.5,100]){
    assert.throws(()=>session.deviceInfo(value),kind(usb.ErrorKind.InvalidArgument));
    assert.throws(()=>session.prepareDataWrite(1,value),kind(usb.ErrorKind.InvalidArgument));
  }
  assert.throws(()=>session.readMemory('0x1000',1),kind(usb.ErrorKind.InvalidArgument));
  for(const bytes of ['1234',[1,2,3,4],new Int8Array([1]),new Float32Array([1]),null]){
    assert.throws(()=>session.writeMemory(0x1000,bytes),kind(usb.ErrorKind.InvalidArgument));
  }
  assert.equal(script.remaining,0);assert.equal(session.state,usb.SessionState.Ready);session.abandon();
});

test('abandoning a pending close preserves explicit abandonment without retry',async()=>{
  const {session,script}=await opened([E.control(env(0xc4,2)),E.pendingRead(2)]);
  const pending=session.close(),rejected=assert.rejects(pending,kind(usb.ErrorKind.Abandoned));
  await until(()=>script.remaining===0);session.abandon();await rejected;
  assert.equal(session.state,usb.SessionState.Abandoned);assert.equal(script.remaining,0);
});

test('cancelPending preserves Ready before dispatch and fences dispatched I/O',async()=>{
  const early=await opened([]);
  const never=early.session.ping();early.session.cancelPending();
  await assert.rejects(never,kind(usb.ErrorKind.Cancelled));
  assert.equal(early.session.state,usb.SessionState.Ready);assert.equal(early.script.remaining,0);
  const {session,script}=await opened([E.control(env(0xc2,2)),E.pendingRead(2)]);
  const pending=session.ping(),rejected=assert.rejects(pending,kind(usb.ErrorKind.Cancelled));
  await until(()=>script.remaining===0);session.cancelPending();session.cancelPending();
  assert.throws(()=>session.ping(),kind(usb.ErrorKind.Busy));
  await rejected;assert.equal(session.state,usb.SessionState.CompletionUnknown);
  await assert.rejects(async()=>session.close(),kind(usb.ErrorKind.CompletionUnknown));
});

test('cancelPending on a pending chunk closes the reader and restores fenced ownership',async()=>{
  const {session,script}=await opened([E.control(env(0xe1,4)),E.pendingRead(4)]);
  const reader=usb.data.readChunks(session,new usb.DataWindow(0x1000,0x1000,8,4,usb.OffsetMode.Checked),0x1000,8);
  const pending=reader.next(),rejected=assert.rejects(pending,kind(usb.ErrorKind.Cancelled));
  await until(()=>script.remaining===0);reader.cancelPending();await rejected;
  reader.close();assert.deepEqual(await reader.next(),{done:true,value:undefined});
  assert.equal(reader.bytesCompleted,0);assert.equal(session.state,usb.SessionState.CompletionUnknown);
});
