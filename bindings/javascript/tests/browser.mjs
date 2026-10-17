import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
const files=new Map([
  ['/dist/index.js',['../dist/index.js','text/javascript']],
  ['/pkg/fujimod.js',['../pkg/fujimod.js','text/javascript']],
  ['/pkg/fujimod_bg.wasm',['../pkg/fujimod_bg.wasm','application/wasm']],
  ['/tests/webusb-fixture.mjs',['./webusb-fixture.mjs','text/javascript']],
]);
const server=createServer(async(req,res)=>{
  const path=new URL(req.url,'http://localhost').pathname;
  if(path==='/'){res.writeHead(200,{'content-type':'text/html'});res.end('<!doctype html><title>FujiMod browser tests</title>');return;}
  const entry=files.get(path);if(!entry){res.writeHead(404).end();return;}
  try{const data=await readFile(new URL(entry[0],import.meta.url));res.writeHead(200,{'content-type':entry[1]});res.end(data);}
  catch{res.writeHead(500).end();}
});
server.listen(0,'127.0.0.1');await once(server,'listening');
let browser;
try{
  browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH}:{})});
  const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/`);
  const result=await page.evaluate(async()=>{
    const {usb,initialize}=await import('/dist/index.js');
    const {installWebUsb}=await import('/tests/webusb-fixture.mjs');
    await initialize();
    const fixture=installWebUsb();
    try{
      const device=await usb.requestDevice(usb.DeviceFilter.Diagnostic);
      const interfaces=await device.interfaces(),options=new usb.TransferOptions(20,1024),trace=new usb.tracing.Trace(128,4096);options.setTrace(trace);
      const session=await usb.Session.open(device,interfaces[0],new usb.FirmwareLimits(256,256,256,256),options);
      const product=(await session.deviceInfo(usb.InfoSelector.Product)).text;
      const data=[...await session.readData(123,5)];const acknowledgement=[...await session.close()];
      const closed=session.state===usb.SessionState.Closed;
      const traceEndpoints=trace.drain().events.map(e=>e.endpoint);
      const second=await usb.Session.open(device,interfaces[0],new usb.FirmwareLimits(256,256,256,256),options);
      fixture.state.hangControl=0xc2;
      let timeout=false;try{await second.ping();}catch(error){timeout=usb.isError(error)&&error.kind===usb.ErrorKind.Transfer&&error.stage===usb.TransferStage.Control;}
      const unknown=second.state===usb.SessionState.CompletionUnknown;second.abandon();
      return {secure:isSecureContext,realWindow:window instanceof Window,product,data,acknowledgement,closed,traceEndpoints,timeout,unknown,controls:fixture.log.filter(e=>e[0]==='control').map(e=>e[1])};
    }finally{fixture.restore();}
  });
  assert.equal(result.secure,true);assert.equal(result.realWindow,true);assert.equal(result.product,'SYNTHETIC');
  assert.deepEqual(result.data,[123,124,125,126,127]);assert.deepEqual(result.acknowledgement,[0xaa,0xbb]);assert.equal(result.closed,true);
  assert.ok(result.traceEndpoints.includes(0x82));assert.ok(result.traceEndpoints.includes(1));assert.equal(result.timeout,true);assert.equal(result.unknown,true);
  assert.equal(result.controls.filter(kind=>kind===0xc4).length,1);
  console.log('PASS Chromium: picker, descriptors, identification, bulk data, raw close, tracing and timeout fencing (mock WebUSB; no hardware)');
}finally{await browser?.close();server.close();await once(server,'close');}
