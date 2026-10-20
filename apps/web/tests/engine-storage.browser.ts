/** Real worker OPFS publication, revisions and failed transaction cleanup. */
import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL.');
const output = resolve('build/engine-storage');
await mkdir(output, { recursive: true });
for (const type of [chromium, webkit]) {
  const profile = await mkdtemp(join(output, type.name() + '-profile-'));
  const context = await type.launchPersistentContext(profile, { headless: true });
  try {
    await context.routeWebSocket('**/*', () => {});
    const page = await context.newPage();
    await page.goto(base);
    const result = await page.evaluate(async () => {
      const module = new URL('/src/cameras/store.ts', location.origin).href;
      const source = `import {CameraStore} from ${JSON.stringify(module)};
     onmessage=async()=>{try{
       const store=new CameraStore();
       const beforeRoot=await (await navigator.storage.getDirectory()).getDirectoryHandle('camera-media',{create:true});const beforeFiles=[];for await(const [name]of beforeRoot.entries())beforeFiles.push(name);
       const camera=await store.create(new File([new Uint8Array([1,2,3])],'test.dat'),{id:'storage',model:'Storage',version:'1',sha256:'test'},false);
       const save=bytes=>({durable:new Uint8Array(bytes).buffer,runtimeKey:'storage-test',poweredOn:true,positions:{},modEnabled:false});
       const first=await store.save(camera.id,0,save([4,5,6]));
       const descriptor=()=>new Promise((resolve,reject)=>{const r=indexedDB.open('fujimod-cameras',1);r.onerror=()=>reject(r.error);r.onsuccess=()=>{const db=r.result,tx=db.transaction('saves'),get=tx.objectStore('saves').get(camera.id);get.onsuccess=()=>resolve(get.result);tx.oncomplete=()=>db.close();};});
       const one=await descriptor();let stale;
       try{await store.save(camera.id,0,save([9]));}catch(error){stale=String(error);}
       const retained=[...new Uint8Array((await store.saved(camera.id)).durable)];
       const second=await store.save(camera.id,first.revision,save([7,8]));const two=await descriptor();
       const original=IDBObjectStore.prototype.put;let quota;
       try{IDBObjectStore.prototype.put=function(...args){if(this.name==='cameras')throw new DOMException('forced metadata quota','QuotaExceededError');return original.apply(this,args);};await store.save(camera.id,second.revision,save([10]));}catch(error){quota=String(error);}finally{IDBObjectStore.prototype.put=original;}
       const after=await descriptor();const root=await (await navigator.storage.getDirectory()).getDirectoryHandle('camera-media');let files=[];for await(const [name]of root.entries())files.push(name);
       postMessage({id:camera.id,first,second,one,two,after,stale,quota,retained,files:files.filter(name=>!beforeFiles.includes(name)),bytes:[...new Uint8Array((await store.saved(camera.id)).durable)]});
     }catch(error){postMessage({error:String(error)});}};`;
      return new Promise<any>((resolve, reject) => {
        const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' })),
          worker = new Worker(url, { type: 'module' });
        worker.onerror = (e) => reject(Error(e.message));
        worker.onmessage = ({ data }) => {
          worker.terminate();
          URL.revokeObjectURL(url);
          resolve(data);
        };
        worker.postMessage({});
      });
    });
    await writeFile(join(output, type.name() + '-attempt.json'), JSON.stringify(result, null, 2));
    assert.equal(result.error, undefined);
    assert.equal(result.one.durable.backend, 'opfs');
    assert.equal(result.two.durable.backend, 'opfs');
    assert.notEqual(result.one.durable.file, result.two.durable.file);
    assert.deepEqual(result.retained, [4, 5, 6]);
    assert.deepEqual(result.bytes, [7, 8]);
    assert.match(result.stale, /another tab/);
    assert.match(result.quota, /quota/i);
    assert.deepEqual(result.after, result.two);
    assert.deepEqual(result.files, [result.two.durable.file]);
    await page.reload();
    const resumed = await page.evaluate(async (id) => {
      const module = new URL('/src/cameras/store.ts', location.origin).href;
      return new Promise<any>((resolve, reject) => {
        const source = `import {CameraStore} from ${JSON.stringify(module)};onmessage=async({data})=>{try{const store=new CameraStore();postMessage({bytes:[...new Uint8Array((await store.saved(data)).durable)],record:(await store.list()).find(c=>c.id===data)});}catch(error){postMessage({error:String(error)});}};`;
        const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' })),
          worker = new Worker(url, { type: 'module' });
        worker.onerror = (e) => reject(Error(e.message));
        worker.onmessage = ({ data }) => {
          worker.terminate();
          URL.revokeObjectURL(url);
          resolve(data);
        };
        worker.postMessage(id);
      });
    }, result.id);
    assert.equal(resumed.error, undefined);
    assert.deepEqual(resumed.bytes, [7, 8]);
    assert.equal(resumed.record.revision, 2);
    await writeFile(
      join(output, type.name() + '.json'),
      JSON.stringify({ result, resumed }, null, 2) + '\n',
    );
    console.log(
      `${type.name()}: worker OPFS, stale/quota preservation, cleanup and reload passed.`,
    );
  } finally {
    await context.close();
    await rm(profile, { recursive: true, force: true });
  }
}
