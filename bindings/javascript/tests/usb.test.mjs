import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import test from 'node:test';
import { initialize, usb, defs } from '../dist/index.js';
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

// These synthetic bodies deliberately differ from the studied firmware's codes,
// mode flags, mailbox and argument positions.
function serviceDefinition(rows = [[153,'flush_configuration',[7],4]], expectedId='synthetic-b-v2') {
  const document={schema:1,firmware:{id:'synthetic-b-v2',model:'Body B',version:'2',sha256:'ab'.repeat(32)},
    mailbox:{page:3,offset:0x240,configuration_limit:512},entries:rows.map(([code,codec,slots,enabled_modes])=>({code,label:codec,codec,slots,enabled_modes,next_mode:0,firmware_kind:19,handler:4294967296,parameter_table:8192}))};
  return defs.service.Definition.fromJson(JSON.stringify(document),new defs.FirmwareIdentity(expectedId,'ab'.repeat(32)));
}
const serviceModes=()=>new defs.service.ServiceModes(4);
const saveInvocation=()=>new usb.app80.Service(serviceDefinition()).prepare(serviceModes(),usb.app80.Command.flushConfiguration());
function serviceExchange(invocation,result) {
  const payload=invocation.raw;
  const stage=concat(words(0x1a0002,0,0,0,3,1,3,0x241,32),payload.slice(1)),
    publish=concat(words(0x1a0002,0,0,0,3,1,3,0x240,33),payload),
    read=words(0x1a0002,0,0,0,2,1,3,0x240,33);
  return [...command(stage,36,stage.slice(0,36)),...command(publish,36,publish.slice(0,36)),...command(read,69,concat(read,result))];
}
test('shared data loader preserves enums, explicit identity and unreviewed rows',()=>{
  const d=serviceDefinition([[153,'flush_configuration',[7],null]]),entry=d.entries[0];
  assert.equal(d.identity.id,'synthetic-b-v2');assert.equal(d.identity.sha256,'ab'.repeat(32));
  assert.equal(entry.codec,defs.service.Codec.FlushConfiguration);assert.equal(entry.handler,4294967296n);
  assert.equal(entry.enabledModes.kind,defs.service.ModeAvailabilityKind.Unreviewed);
  assert.equal(entry.transition.kind,defs.service.ModeTransitionKind.Keep);
  assert.deepEqual(entry.slots,new Uint8Array([7]));assert.equal(entry.firmwareKind,19);
  assert.equal(d.select(153,serviceModes()),undefined);
  assert.deepEqual([d.mailbox.page,d.mailbox.offset,d.mailbox.configurationLimit],[3,0x240,512]);
  assert.throws(()=>serviceDefinition(undefined,'another-body'),e=>defs.isError(e)&&e.reason.kind===defs.DefinitionFailureKind.FirmwareMismatch);
  assert.throws(()=>new defs.FirmwareIdentity('body','é'.repeat(32)),e=>defs.isError(e)&&e.reason.kind===defs.DefinitionFailureKind.InvalidDigest);
  assert.throws(()=>new defs.service.ServiceModes(0),e=>defs.isError(e)&&e.reason.kind===defs.DefinitionFailureKind.EmptyModes);
  assert.throws(()=>defs.service.Definition.fromJson('{}',d.identity),e=>defs.isError(e)&&e.reason.kind===defs.DefinitionFailureKind.Json);
  assert.throws(()=>{entry.codec=defs.service.Codec.Research;},TypeError);
});

test('typed command selection errors retain enum reasons without fallback',()=>{
  const service=new usb.app80.Service(serviceDefinition()),modes=serviceModes();
  assert.throws(()=>new usb.app80.PinBank(16),e=>usb.app80.isError(e)&&e.reason.kind===usb.app80.CommandFailureKind.Argument&&e.reason.argument===usb.app80.ArgumentKind.PinBank);
  assert.throws(()=>new usb.app80.WordAddress(3),e=>usb.app80.isError(e)&&e.reason.argument===usb.app80.ArgumentKind.WordAlignment);
  assert.throws(()=>service.prepare(new defs.service.ServiceModes(1),usb.app80.Command.flushConfiguration()),e=>usb.app80.isError(e)&&e.reason.kind===usb.app80.CommandFailureKind.Unsupported&&e.reason.codec===defs.service.Codec.FlushConfiguration);
  assert.throws(()=>service.prepareResearch(new usb.app80.CommandCode(153),new defs.service.ServiceModes(1),new Uint8Array()),e=>usb.app80.isError(e)&&e.reason.kind===usb.app80.CommandFailureKind.Unavailable&&e.reason.code===153);
  assert.throws(()=>service.prepareResearch(new usb.app80.CommandCode(153),modes,new Uint8Array(33)),e=>usb.app80.isError(e)&&e.reason.kind===usb.app80.CommandFailureKind.OversizeArguments);
  const ambiguous=new usb.app80.Service(serviceDefinition([[153,'flush_configuration',[7],4],[154,'flush_configuration',[7],4]]));
  assert.throws(()=>ambiguous.prepare(modes,usb.app80.Command.flushConfiguration()),e=>usb.app80.isError(e)&&e.reason.kind===usb.app80.CommandFailureKind.Ambiguous);
  assert.throws(()=>new usb.app80.Command());assert.throws(()=>service.prepare(4,usb.app80.Command.flushConfiguration()));
  assert.throws(()=>usb.app80.Command.readPins(0));assert.throws(()=>usb.app80.Command.profileByte('read'));
  for(const value of ['1',false,NaN,Infinity,1.5,-1,256])assert.throws(()=>usb.app80.Command.readChannel(value),kind(usb.ErrorKind.InvalidArgument));
  assert.throws(()=>usb.app80.ProfileAction.read('First',new usb.app80.ProfileIndex(0)),kind(usb.ErrorKind.InvalidArgument));
});

test('all typed command variants delegate encoding and slot selection to Rust',()=>{
  const C=usb.app80.Command,P=usb.app80.ProfileAction;
  const cases=[
    ['write_pins',C.writePins(new usb.app80.PinBank(2),0x1234,0x5678),[2,0x12,0x34,0x56,0x78]],
    ['read_pins',C.readPins(new usb.app80.PinBank(2)),[2,0,0]],
    ['read_channel',C.readChannel(30),[30,0,0]],
    ['write_channel',C.writeChannel(30,0x1234),[30,0x12,0x34]],
    ['profile_byte',C.profileByte(P.read(usb.app80.ProfileBank.First,new usb.app80.ProfileIndex(3))),[1,3,0]],
    ['profile_byte',C.profileByte(P.write(usb.app80.ProfileBank.Second,new usb.app80.ProfileIndex(4),37)),[4,4,37]],
    ['profile_byte',C.profileByte(P.reset()),[5,0,0]],
    ['download_status',C.downloadStatus(),[0]],['copy_configuration',C.copyConfiguration(9),[9]],
    ['flush_configuration',C.flushConfiguration(),[255]],
    ['memory_word',C.readWord(new usb.app80.WordAddress(0x40001000)),[0,0,0x40,0,0x10,0,0,0,0,0]],
    ['memory_word',C.writeWord(new usb.app80.WordAddress(0x40001000),0x12345678),[1,0,0x40,0,0x10,0,0x12,0x34,0x56,0x78]],
  ];
  for(const [codec,command,expected] of cases) {
    const slots=expected.map((_,i)=>(i*3+7)%32),service=new usb.app80.Service(serviceDefinition([[153,codec,slots,4]]));
    const invocation=service.prepare(serviceModes(),command),raw=invocation.raw;
    assert.equal(raw[0],153);assert.deepEqual(slots.map(s=>raw[1+s]),expected);assert.equal(invocation.configurationLimit,512);
    raw[0]=0;assert.equal(invocation.raw[0],153);
  }
});

test('typed result unions retain rejection, unknown bytes and immutable payloads',()=>{
  const C=usb.app80.Command,R=usb.app80.ReplyKind;
  function decode(codec,slots,command,changes={}) {
    const invocation=new usb.app80.Service(serviceDefinition([[153,codec,slots,4]])).prepare(serviceModes(),command),raw=invocation.raw;
    for(const [position,byte] of Object.entries(changes))raw[1+slots[position]]=byte;
    const reply=invocation.decode(raw);assert.deepEqual(reply.raw,raw);assert.ok(Object.isFrozen(reply.value));return reply.value;
  }
  assert.deepEqual(decode('read_pins',[7,9,3],C.readPins(new usb.app80.PinBank(2)),{1:0x12,2:0x34}),{kind:R.Pins,levels:0x1234});
  assert.deepEqual(decode('read_channel',[7,9,3],C.readChannel(30),{1:0x12,2:0x34}),{kind:R.Channel,value:0x1234});
  assert.deepEqual(decode('profile_byte',[7,9,3],C.profileByte(usb.app80.ProfileAction.read(usb.app80.ProfileBank.First,new usb.app80.ProfileIndex(0))),{2:57}),{kind:R.ProfileByte,value:57});
  assert.deepEqual(decode('download_status',[7],C.downloadStatus(),{0:1}),{kind:R.DownloadStatus,rawStatus:1});
  assert.deepEqual(decode('copy_configuration',[7],C.copyConfiguration(1)),{kind:R.NoPayload});
  assert.deepEqual(decode('flush_configuration',[7],C.flushConfiguration()),{kind:R.ConfigurationSaved,status:{kind:usb.app80.SaveStatusKind.Unknown,rawStatus:255}});
  const read=C.readWord(new usb.app80.WordAddress(0x40001000)),write=C.writeWord(new usb.app80.WordAddress(0x40001000),0x12345678),slots=[0,1,2,3,4,5,6,7,8,9];
  assert.deepEqual(decode('memory_word',slots,read,{1:1,6:0x12,7:0x34,8:0x56,9:0x78}),{kind:R.ReadWord,result:{kind:usb.app80.WordReadKind.Value,value:0x12345678}});
  assert.deepEqual(decode('memory_word',slots,read,{1:0}).result,{kind:usb.app80.WordReadKind.Rejected});
  assert.deepEqual(decode('memory_word',slots,read,{1:42}).result,{kind:usb.app80.WordReadKind.Unknown,rawStatus:42});
  assert.deepEqual(decode('memory_word',slots,write,{1:1}).result,{kind:usb.app80.WordWriteKind.Confirmed});
  assert.deepEqual(decode('memory_word',slots,write,{1:0}).result,{kind:usb.app80.WordWriteKind.Rejected});
  assert.deepEqual(decode('memory_word',slots,write,{1:42}).result,{kind:usb.app80.WordWriteKind.Unknown,rawStatus:42});
  const invocation=saveInvocation();assert.throws(()=>invocation.decode(new Uint8Array(33)),kind(usb.ErrorKind.Protocol));
  const research=new usb.app80.Service(serviceDefinition()).prepareResearch(new usb.app80.CommandCode(153),serviceModes(),new Uint8Array());
  assert.deepEqual(research.decode(new Uint8Array(33)).value,{kind:R.Research});
});

test('typed invocation awaits publish and status one never starts a poll',async()=>{
  const invocation=saveInvocation(),raw=invocation.raw;raw[8]=1;
  const {session,script}=await opened(serviceExchange(invocation,raw));
  const pending=usb.app80.execute(session,invocation);assert.equal(session.state,usb.SessionState.InFlight);
  const reply=await pending;assert.deepEqual(reply.value,{kind:usb.app80.ReplyKind.ConfigurationSaved,status:{kind:usb.app80.SaveStatusKind.Failed}});
  assert.equal(session.state,usb.SessionState.Ready);assert.equal(script.remaining,0);session.abandon();
  const small=await opened([],{firmware:new usb.FirmwareLimits(256,256,32,256)});
  assert.throws(()=>usb.app80.execute(small.session,invocation),kind(usb.ErrorKind.InvalidArgument));
  assert.equal(small.session.state,usb.SessionState.Ready);assert.equal(small.script.remaining,0);small.session.abandon();
});

test('typed foreign reply and interrupted publication fence session ownership',async()=>{
  const invocation=saveInvocation(),bad=await opened(serviceExchange(invocation,new Uint8Array(33)));
  await assert.rejects(usb.app80.execute(bad.session,invocation),e=>e.kind===usb.ErrorKind.Protocol&&e.protocolKind===usb.ProtocolFailureKind.CommandEcho);
  assert.equal(bad.session.state,usb.SessionState.CompletionUnknown);await assert.rejects(bad.session.close(),kind(usb.ErrorKind.CompletionUnknown));assert.equal(bad.script.remaining,0);
  const steps=serviceExchange(invocation,invocation.raw).slice(0,8);steps[7]=E.pendingRead(36);
  const {session,script}=await opened(steps),pending=usb.app80.execute(session,invocation),rejected=assert.rejects(pending,kind(usb.ErrorKind.Cancelled));
  await until(()=>script.remaining===0);session.cancelPending();await rejected;
  assert.equal(session.state,usb.SessionState.CompletionUnknown);assert.throws(()=>session.ping(),kind(usb.ErrorKind.CompletionUnknown));
});
