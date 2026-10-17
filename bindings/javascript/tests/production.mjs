import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import test from 'node:test';
import { initialize, usb } from '../dist/index.js';
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
