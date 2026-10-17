import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { cachedFetch } from '../src/asset-cache.ts';

test('large local input is revalidated on disk; changed source bytes replace it', async () => {
  let source = 'first', tag = 'one', saved;
  const calls = [];
  const storage={read:async()=>saved,write:async(_url,metadata,response)=>{saved={...metadata,bytes:await response.arrayBuffer()};}};
  globalThis.fetch = async (url, options) => {
    calls.push(options.method ?? 'GET');
    return new Response(options.method === 'HEAD' ? null : source, {headers:{etag:tag}});
  };
  assert.equal(await (await cachedFetch('/local/firmware.dat',fetch,storage)).text(), 'first');
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal(await (await cachedFetch('/local/firmware.dat',fetch,storage)).text(), 'first');
  source='second';tag='two';
  assert.equal(await (await cachedFetch('/local/firmware.dat',fetch,storage)).text(), 'second');
  assert.deepEqual(calls,['GET','HEAD','HEAD','GET']);
});

test('unavailable storage uses the original fetch', async () => {
  const storage={read:async()=>{throw Error('No storage');},write:async()=>{throw Error('No storage');}};
  globalThis.fetch=async()=>new Response('checked by the caller');
  assert.equal(await (await cachedFetch('/local/firmware.dat',fetch,storage)).text(),'checked by the caller');
});

test('OPFS writes response chunks atomically and serves the verified file body', async () => {
  const files=new Map(); let writes=0, gets=0;
  const dir={getFileHandle:async key=>({
    getFile:async()=>{if(!files.has(key))throw Error('absent');return files.get(key);},
    createWritable:async()=>{
      const parts=[];
      return {write:async part=>{writes++;parts.push(part.slice());},
        close:async()=>{files.set(key,new Blob(parts));},abort:async()=>{}};
    },
  })};
  const previous=Object.getOwnPropertyDescriptor(globalThis,'navigator');
  Object.defineProperty(globalThis,'navigator',{configurable:true,value:{storage:{getDirectory:async()=>({getDirectoryHandle:async()=>dir})}}});
  const fetcher=async(_url,options)=>{
    if(options.method==='HEAD')return new Response(null,{headers:{etag:'chunks'}});
    gets++;
    return new Response(new ReadableStream({start(controller){
      controller.enqueue(new Uint8Array([1,2]));controller.enqueue(new Uint8Array([3,4]));controller.close();
    }}),{headers:{etag:'chunks','content-type':'application/octet-stream'}});
  };
  try {
    assert.deepEqual(new Uint8Array(await (await cachedFetch('/chunked',fetcher)).arrayBuffer()),new Uint8Array([1,2,3,4]));
    for(let i=0;i<100&&!files.size;i++)await new Promise(resolve=>setImmediate(resolve));
    assert.equal(files.size,1);assert.equal(writes,3);
    assert.deepEqual(new Uint8Array(await (await cachedFetch('/chunked',fetcher)).arrayBuffer()),new Uint8Array([1,2,3,4]));
    assert.equal(gets,1);
  } finally {
    if(previous)Object.defineProperty(globalThis,'navigator',previous);else delete globalThis.navigator;
  }
});
