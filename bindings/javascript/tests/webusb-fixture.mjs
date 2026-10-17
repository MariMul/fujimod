// Camera-side WebUSB fixture for the real nusb backend. Node has no USB access.
const assert={
  equal(actual,expected){if(!Object.is(actual,expected))throw new Error(`Expected ${expected}, received ${actual}`);},
  ok(value){if(!value)throw new Error('Fixture assertion failed');},
  deepEqual(actual,expected){const keys=Object.keys(expected);if(!actual||Object.keys(actual).length!==keys.length||keys.some(key=>!Object.is(actual[key],expected[key])))throw new Error('Unexpected WebUSB control parameters');},
};
const utf16le=text=>{const bytes=new Uint8Array(text.length*2),view=new DataView(bytes.buffer);[...text].forEach((ch,i)=>view.setUint16(i*2,ch.charCodeAt(0),true));return bytes;};
const words=(...values)=>{const bytes=new Uint8Array(values.length*4),v=new DataView(bytes.buffer);values.forEach((n,i)=>v.setUint32(i*4,n,true));return bytes;};
const descriptor=new Uint8Array([18,1,0,2,0,0,0,64,0xcb,4,0x80,0xff,0,1,1,2,3,1]);
const configDescriptor=new Uint8Array([9,2,32,0,1,1,0,0x80,50,9,4,2,3,2,0xff,0,0,0,7,5,0x82,2,64,0,0,7,5,1,2,64,0,0]);
const view=bytes=>new DataView(Uint8Array.from(bytes).buffer);
export function installWebUsb(){
  const previousWindow=Object.getOwnPropertyDescriptor(globalThis,'Window');
  const nav=globalThis.navigator;
  const previousUsb=Object.getOwnPropertyDescriptor(nav,'usb');
  const realWindow=typeof Window!=='undefined'&&globalThis instanceof Window;
  const log=[],state={pending:new Uint8Array(),kind:undefined,cancelPicker:false,hangControl:undefined,pieces:[]};
  const iface={interfaceNumber:2,claimed:false,alternates:[{alternateSetting:3,interfaceClass:0xff,interfaceSubclass:0,interfaceProtocol:0,interfaceName:'fixture'}]};
  const config={configurationValue:1,interfaces:[iface]};
  const device={vendorId:0x04cb,productId:0xff80,deviceVersionMajor:1,deviceVersionMinor:0,deviceVersionSubminor:0,
    usbVersionMajor:2,usbVersionMinor:0,usbVersionSubminor:0,deviceClass:0,deviceSubclass:0,deviceProtocol:0,
    manufacturerName:'FUJIFILM',productName:'Synthetic service device',serialNumber:'synthetic',
    configuration:null,configurations:[config],opened:false,
    async open(){this.opened=true;log.push(['open']);},
    async selectConfiguration(number){assert.equal(number,1);this.configuration=config;log.push(['configuration',number]);},
    async claimInterface(number){assert.equal(number,2);iface.claimed=true;log.push(['claim',number]);},
    async selectAlternateInterface(number,alternate){assert.equal(number,2);assert.equal(alternate,3);log.push(['alternate',number,alternate]);},
    async releaseInterface(number){assert.equal(number,2);iface.claimed=false;log.push(['release',number]);},
    async controlTransferIn(setup,length){
      assert.equal(setup.requestType,'standard');assert.equal(setup.recipient,'device');assert.equal(setup.request,6);
      log.push(['descriptor',setup.value,length]);
      const bytes=setup.value>>>8===1?descriptor:configDescriptor;
      return {status:'ok',data:view(bytes.slice(0,length))};
    },
    async controlTransferOut(setup,raw){
      assert.deepEqual(setup,{requestType:'vendor',recipient:'other',request:0,value:0,index:0});
      const bytes=new Uint8Array(raw),v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
      const kind=bytes[0],offset=v.getUint32(2),size=v.getUint32(6),token=v.getUint16(10);
      assert.equal(bytes.length,12);assert.equal(bytes[1],0);assert.equal(token,kind===0xc0?0:0x1234);
      log.push(['control',kind,size,offset]);state.kind=kind;
      if(kind===state.hangControl)return new Promise(()=>{});
      if(kind===0xc0)state.pending=new Uint8Array([0x34,0x12,0,0,0,0,1,0]);
      else if(kind===0xc2||kind===0xc4)state.pending=new Uint8Array([0xaa,0xbb]);
      else if(kind===0xe1)state.pending=Uint8Array.from({length:size},(_,i)=>(offset+i)&255);
      else if(kind===0xe8)state.pending=new Uint8Array([0x01,0x02,0x03,0x04]);
      else assert.ok(kind===0xd0||kind===0xd1||kind===0xe0);
      return {status:'ok',bytesWritten:bytes.length};
    },
    async transferOut(endpoint,raw){
      assert.equal(endpoint,1);const bytes=Uint8Array.from(new Uint8Array(raw));log.push(['out',endpoint,bytes]);
      if(state.kind===0xe0)state.pieces.push(bytes);
      else{
        assert.equal(state.kind,0xd0);const v=new DataView(bytes.buffer),code=v.getUint32(0,true);
        if(code===0x150000){assert.equal(v.getUint32(16,true),4);const text=utf16le('SYNTHETIC\0');state.pending=new Uint8Array(16+text.length);state.pending.set(words(code,0,0,0));state.pending.set(text,16);}
        else if(code===0x760001)state.pending=words(code,0,0,0,0,0,0,0);
        else throw new Error(`Unexpected fixture command ${code.toString(16)}`);
      }
      return {status:'ok',bytesWritten:bytes.length};
    },
    async transferIn(endpoint,maximum){
      assert.equal(endpoint,2);assert.equal(maximum%64,0);log.push(['in',endpoint,maximum]);
      const raw=state.pending.slice(0,maximum);state.pending=state.pending.slice(raw.length);
      return {status:'ok',data:view(raw)};
    },
    async reset(){throw new Error('Unexpected USB reset');},
  };
  const other={...device,vendorId:0x1234,productId:0x5678};
  if(!realWindow)Object.defineProperty(globalThis,'Window',{configurable:true,value:class Window{static[Symbol.hasInstance](object){return object===globalThis;}}});
  Object.defineProperty(nav,'usb',{configurable:true,value:{
    async getDevices(){log.push(['list']);return [other,device];},
    async requestDevice(options){log.push(['picker',options.filters]);if(state.cancelPicker)throw new DOMException('Cancelled','NotFoundError');return device;},
  }});
  return {device,iface,state,log,restore(){
    if(previousWindow)Object.defineProperty(globalThis,'Window',previousWindow);else delete globalThis.Window;
    if(previousUsb)Object.defineProperty(nav,'usb',previousUsb);else delete nav.usb;
  }};
}
