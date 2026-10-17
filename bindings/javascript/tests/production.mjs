import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import test from 'node:test';
import { initialize, usb, defs } from '../dist/index.js';
import * as raw from '../pkg/fujimod.js';
await initialize({ module_or_path: await readFile(new URL('../pkg/fujimod_bg.wasm', import.meta.url)) });
test('production build excludes synthetic endpoint entry points',()=>{
  assert.equal('TestScript' in raw,false);assert.equal('TestExchange' in raw,false);
  assert.equal('TestScript' in usb,false);assert.equal(typeof usb.requestDevice,'function');
});

import { installWebUsb } from './webusb-fixture.mjs';
test('production nusb/WebUSB opens explicit interfaces and preserves endpoint numbers',async()=>{
  const fixture=installWebUsb();
  try{
    const devices=await usb.discover(usb.DeviceFilter.Diagnostic);assert.equal(devices.length,1);assert.equal(devices[0].productId,0xff80);
    const picked=await usb.requestDevice(usb.DeviceFilter.Diagnostic);assert.equal(picked.vendorId,0x04cb);
    assert.deepEqual(fixture.log.find(e=>e[0]==='picker')[1],[{vendorId:0x04cb,productId:0xff80}]);
    const interfaces=await picked.interfaces();assert.equal(interfaces.length,1);
    assert.deepEqual([interfaces[0].configuration,interfaces[0].interface,interfaces[0].alternate],[1,2,3]);
    const trace=new usb.tracing.Trace(128,4096),options=new usb.TransferOptions(100,1024);options.setTrace(trace);
    const session=await usb.Session.open(picked,interfaces[0],new usb.FirmwareLimits(256,256,256,256),options);
    assert.equal((await session.deviceInfo(usb.InfoSelector.Product)).text,'SYNTHETIC');
    assert.deepEqual(await session.readData(123,5),new Uint8Array([123,124,125,126,127]));
    const upload=await usb.data.upload(session,new Uint8Array([9,8,7]),2,usb.DataWriteMode.Preserve);
    assert.equal(upload.bytesWritten,3);assert.deepEqual(fixture.state.pieces,[new Uint8Array([9,8]),new Uint8Array([7])]);
    assert.deepEqual(await session.close(),new Uint8Array([0xaa,0xbb]));assert.equal(fixture.iface.claimed,false);
    assert.ok(fixture.log.some(e=>e[0]==='alternate'&&e[1]===2&&e[2]===3));
    const events=trace.drain().events;assert.ok(events.some(e=>e.endpoint===0x82));assert.ok(events.some(e=>e.endpoint===1));
  }finally{fixture.restore();}
});
test('production WebUSB control deadline fences without retry or C4',async()=>{
  const fixture=installWebUsb();
  try{
    const [device]=await usb.discover(usb.DeviceFilter.Diagnostic),options=new usb.TransferOptions(10,1024);
    const session=await usb.Session.open(device,new usb.InterfaceSelection(1,2,3),new usb.FirmwareLimits(256,256,256,256),options);
    fixture.state.hangControl=0xc2;
    await assert.rejects(session.ping(),e=>{assert.equal(e.kind,usb.ErrorKind.Transfer);assert.equal(e.stage,usb.TransferStage.Control);assert.equal(e.requestKind,usb.RequestKind.Ping);return true;});
    assert.equal(session.state,usb.SessionState.CompletionUnknown);
    await assert.rejects(async()=>session.close(),e=>e.kind===usb.ErrorKind.CompletionUnknown);
    assert.equal(fixture.log.filter(e=>e[0]==='control'&&e[1]===0xc2).length,1);
    assert.equal(fixture.log.some(e=>e[0]==='control'&&e[1]===0xc4),false);
  }finally{fixture.restore();}
});
test('picker cancellation returns no device and sends no USB commands',async()=>{
  const fixture=installWebUsb();fixture.state.cancelPicker=true;
  try{assert.equal(await usb.requestDevice(usb.DeviceFilter.Fujifilm),undefined);assert.deepEqual(fixture.log,[['picker',[{vendorId:0x04cb}]]]);}
  finally{fixture.restore();}
});


test('production WebUSB executes firmware-selected service requests and fences bad nested replies',async()=>{
  const fixture=installWebUsb();
  try{
    const definition=defs.service.Definition.fromJson(JSON.stringify({schema:1,
      firmware:{id:'synthetic-b-v2',model:'Body B',version:'2',sha256:'ab'.repeat(32)},
      mailbox:{page:3,offset:0x240,configuration_limit:512},
      entries:[{code:153,label:'save',codec:'flush_configuration',slots:[7],enabled_modes:4,next_mode:0,firmware_kind:19,handler:4096,parameter_table:8192}]}),
      new defs.FirmwareIdentity('synthetic-b-v2','ab'.repeat(32)));
    const invocation=new usb.app80.Service(definition).prepare(new defs.service.ServiceModes(4),usb.app80.Command.flushConfiguration());
    const [device]=await usb.discover(usb.DeviceFilter.Diagnostic),[iface]=await device.interfaces();
    const open=()=>usb.Session.open(device,iface,new usb.FirmwareLimits(256,256,256,256),new usb.TransferOptions(100,1024));
    fixture.state.onConfigurationWrite=(page,address,bytes)=>{
      if(page===3&&address===0x240){assert.equal(bytes[0],153);assert.equal(bytes[8],255);fixture.state.configuration.set('3:584',1);}
    };
    const session=await open(),reply=await usb.app80.execute(session,invocation);
    assert.deepEqual(reply.value,{kind:usb.app80.ReplyKind.ConfigurationSaved,status:{kind:usb.app80.SaveStatusKind.Failed}});
    assert.equal(session.state,usb.SessionState.Ready);await session.close();
    fixture.state.onConfigurationWrite=(page,address)=>{if(page===3&&address===0x240)fixture.state.configuration.set('3:576',0);};
    const bad=await open();await assert.rejects(usb.app80.execute(bad,invocation),e=>e.kind===usb.ErrorKind.Protocol&&e.protocolKind===usb.ProtocolFailureKind.CommandEcho);
    assert.equal(bad.state,usb.SessionState.CompletionUnknown);
    await assert.rejects(bad.close(),e=>e.kind===usb.ErrorKind.CompletionUnknown);
    assert.equal(fixture.log.filter(e=>e[0]==='control'&&e[1]===0xc4).length,1);
    bad.abandon();
  }finally{fixture.restore();}
});

test('production charge status one is a failed result without another host request',async()=>{
  const fixture=installWebUsb();
  try{
    const definition=defs.service.Definition.fromJson(JSON.stringify({schema:1,
      firmware:{id:'synthetic-charge-v2',model:'Synthetic',version:'2',sha256:'ab'.repeat(32)},
      mailbox:{page:3,offset:576,configuration_limit:512},entries:[{code:153,label:'charge',codec:'charge_control',slots:[0,1,2],enabled_modes:4,next_mode:0,firmware_kind:1,handler:0,parameter_table:0}]}),
      new defs.FirmwareIdentity('synthetic-charge-v2','ab'.repeat(32)));
    const invocation=new usb.app80.Service(definition).prepare(new defs.service.ServiceModes(4),usb.app80.Command.chargeControl(usb.app80.ChargeAction.control(usb.app80.Toggle.Enabled)));
    fixture.state.onConfigurationWrite=(page,address,raw)=>{if(page===3&&address===576){assert.equal(raw[3],255);fixture.state.configuration.set('3:579',1);}};
    const [device]=await usb.discover(usb.DeviceFilter.Diagnostic),[iface]=await device.interfaces();
    const session=await usb.Session.open(device,iface,new usb.FirmwareLimits(256,256,256,256),new usb.TransferOptions(100,1024));
    const reply=await usb.app80.execute(session,invocation);
    assert.deepEqual(reply.value,{kind:usb.app80.ReplyKind.ChargeApplied,status:{kind:usb.app80.ChargeResultKind.Failed}});
    assert.equal(session.state,usb.SessionState.Ready);
    assert.equal(fixture.log.filter(e=>e[0]==='control'&&e[1]===0xd0).length,3);
    await session.close();
  }finally{fixture.restore();}
});

test('production audio/storage results and payload-free commands finish without extra polling',async()=>{
  const fixture=installWebUsb(),a=usb.app80;
  try{
    const [device]=await usb.discover(usb.DeviceFilter.Diagnostic),[iface]=await device.interfaces();
    for(const [codec,count,command,index,status,expected] of [
      ['audio_capture',3,a.Command.audioCapture(new a.AudioCount(10),a.AudioRoute.Request82),2,1,{kind:a.ReplyKind.AudioCaptured,status:{kind:a.AudioResultKind.Failed}}],
      ['audio_playback',2,a.Command.audioPlayback(a.AudioPlayback.AllLanes),1,255,{kind:a.ReplyKind.AudioPlayed,status:{kind:a.AudioResultKind.Unknown,rawStatus:255}}],
      ['erase_storage',2,a.Command.eraseStorage(a.StorageErase.ResetProfiles),1,1,{kind:a.ReplyKind.StorageErased,status:{kind:a.StorageEraseResultKind.Failed}}],
      ['storage_status',1,a.Command.storageStatus(),0,0,{kind:a.ReplyKind.StorageStatus,status:{kind:a.StorageStatusKind.Zero}}],
      ['format_storage',0,a.Command.formatStorage(),null,null,{kind:a.ReplyKind.NoPayload}],
      ['display_setup',0,a.Command.displaySetup(),null,null,{kind:a.ReplyKind.NoPayload}],
    ]){
      const definition=defs.service.Definition.fromJson(JSON.stringify({schema:1,
        firmware:{id:'synthetic-storage-v2',model:'Synthetic',version:'2',sha256:'ab'.repeat(32)},
        mailbox:{page:3,offset:576,configuration_limit:512},entries:[{code:153,label:codec,codec,slots:Array.from({length:count},(_,n)=>7+n),enabled_modes:4,next_mode:0,firmware_kind:1,handler:0,parameter_table:0}]}),new defs.FirmwareIdentity('synthetic-storage-v2','ab'.repeat(32)));
      const invocation=new a.Service(definition).prepare(new defs.service.ServiceModes(4),command);
      fixture.state.onConfigurationWrite=(page,address,raw)=>{if(page===3&&address===576&&index!==null){assert.equal(raw[8+index],255);fixture.state.configuration.set(`3:${584+index}`,status);}};
      const before=fixture.log.filter(e=>e[0]==='control'&&e[1]===0xd0).length;
      const session=await usb.Session.open(device,iface,new usb.FirmwareLimits(256,256,256,256),new usb.TransferOptions(100,1024));
      assert.deepEqual((await a.execute(session,invocation)).value,expected);
      assert.equal(session.state,usb.SessionState.Ready);
      assert.equal(fixture.log.filter(e=>e[0]==='control'&&e[1]===0xd0).length-before,3);
      await session.close();
    }
  }finally{fixture.restore();}
});

test('production radio and NAND results do not request host completion polling',async()=>{
 const fixture=installWebUsb(),a=usb.app80;
 try{
  const [device]=await usb.discover(usb.DeviceFilter.Diagnostic),[iface]=await device.interfaces();
  for(const [codec,count,command,index,byte,expected] of [
   ['nand_read_blocks',3,a.Command.nandReadBlocks(a.NandSpare.Include,255),2,1,{kind:a.ReplyKind.NandRead,status:{kind:a.NandReadResultKind.RejectedOrWaitError}}],
   ['option41',1,a.Command.option41(),0,1,{kind:a.ReplyKind.Option41,state:{kind:a.Option41StateKind.Zero}}],
   ['wlan_control',1,a.Command.wlanControl(a.Toggle.Enabled),null,null,{kind:a.ReplyKind.NoPayload}],
   ['radio_control',4,a.Command.radioControl(a.RadioAction.primary(a.RadioMode.Mode1,new a.RadioChannel(14),new a.RadioPower(12),a.Toggle.Enabled)),null,null,{kind:a.ReplyKind.NoPayload}],
   ['signal_strength',7,a.Command.signalStrength(new a.PeerAddress(new Uint8Array(6))),6,255,{kind:a.ReplyKind.SignalStrength,rawValue:255}],
   ['iperf',1,a.Command.iperf(),0,1,{kind:a.ReplyKind.Iperf,rawResult:1}],
  ]){
   const definition=defs.service.Definition.fromJson(JSON.stringify({schema:1,firmware:{id:'radio-b',model:'Synthetic',version:'2',sha256:'ab'.repeat(32)},mailbox:{page:3,offset:576,configuration_limit:512},entries:[{code:153,label:codec,codec,slots:Array.from({length:count},(_,n)=>n+7),enabled_modes:4,next_mode:0,firmware_kind:1,handler:0,parameter_table:0}]}),new defs.FirmwareIdentity('radio-b','ab'.repeat(32)));
   const invocation=new a.Service(definition).prepare(new defs.service.ServiceModes(4),command);
   fixture.state.onConfigurationWrite=(page,address,raw)=>{if(page===3&&address===576&&index!==null){assert.equal(raw[8+index],255);fixture.state.configuration.set(`3:${584+index}`,byte);}};
   const before=fixture.log.filter(e=>e[0]==='control'&&e[1]===0xd0).length;
   const session=await usb.Session.open(device,iface,new usb.FirmwareLimits(256,256,256,256),new usb.TransferOptions(100,1024));
   assert.deepEqual((await a.execute(session,invocation)).value,expected);assert.equal(session.state,usb.SessionState.Ready);
   assert.equal(fixture.log.filter(e=>e[0]==='control'&&e[1]===0xd0).length-before,3);await session.close();
  }
 }finally{fixture.restore();}
});

test('production WPS and station snapshot bytes never trigger another host request',async()=>{
 const fixture=installWebUsb(),a=usb.app80;
 try{
  const [device]=await usb.discover(usb.DeviceFilter.Diagnostic),[iface]=await device.interfaces();
  for(const [codec,count,command,output,expected] of [
   ['wps',8,a.Command.wps(a.WpsAction.start(a.WpsTarget.broadcast())),{7:1},{kind:a.ReplyKind.WpsCallback,snapshot:{kind:a.WpsCallbackResultKind.Timeout}}],
   ['wps',8,a.Command.wps(a.WpsAction.publishProgress()),{7:19},{kind:a.ReplyKind.WpsProgress,snapshot:{kind:a.WpsProgressKind.State,phase:a.WpsPhase.CredentialsReceived,connection:a.ConnectionState.Connected}}],
   ['wps',8,a.Command.wps(a.WpsAction.stop()),{},{kind:a.ReplyKind.NoPayload}],
   ['wps',8,a.Command.wps(a.WpsAction.connect()),{},{kind:a.ReplyKind.NoPayload}],
   ['station',4,a.Command.station(a.StationAction.startStored()),{2:255},{kind:a.ReplyKind.StationConnection,rawResult:255}],
   ['station',4,a.Command.station(a.StationAction.scan(a.StationScanMode.Mode1)),{2:1,3:255},{kind:a.ReplyKind.StationScan,rawResult:1,rawCount:255}],
   ['station',4,a.Command.station(a.StationAction.stop()),{},{kind:a.ReplyKind.NoPayload}],
  ]){
   const definition=defs.service.Definition.fromJson(JSON.stringify({schema:1,firmware:{id:'wps-b',model:'Synthetic',version:'2',sha256:'ab'.repeat(32)},mailbox:{page:3,offset:576,configuration_limit:512},entries:[{code:153,label:codec,codec,slots:Array.from({length:count},(_,n)=>n+7),enabled_modes:4,next_mode:0,firmware_kind:1,handler:0,parameter_table:0}]}),new defs.FirmwareIdentity('wps-b','ab'.repeat(32)));
   const invocation=new a.Service(definition).prepare(new defs.service.ServiceModes(4),command);
   fixture.state.onConfigurationWrite=(page,address,raw)=>{if(page===3&&address===576)for(const [index,byte] of Object.entries(output)){assert.equal(raw[8+Number(index)],255);fixture.state.configuration.set(`3:${584+Number(index)}`,byte);}};
   const before=fixture.log.filter(e=>e[0]==='control'&&e[1]===0xd0).length;
   const session=await usb.Session.open(device,iface,new usb.FirmwareLimits(256,256,256,256),new usb.TransferOptions(100,1024));
   assert.deepEqual((await a.execute(session,invocation)).value,expected);assert.equal(session.state,usb.SessionState.Ready);
   assert.equal(fixture.log.filter(e=>e[0]==='control'&&e[1]===0xd0).length-before,3);await session.close();
  }
 }finally{fixture.restore();}
});
