import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {SIZES,makeFixture,decodeFixture} from './fixture.mjs';
const entries=JSON.parse(await readFile(new URL('./fixtures/manifest.json',import.meta.url)));
assert.deepEqual(entries.map(e=>e.count),SIZES);
for(const m of entries) {
  const bytes=await readFile(new URL('./fixtures/'+m.file,import.meta.url));
  assert.equal(createHash('sha256').update(bytes).digest('hex'),m.sha256);
  const f=decodeFixture(m,bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength));
  const regenerated=makeFixture(m.count);
  assert.deepEqual(new Uint8Array(regenerated.buffer),new Uint8Array(f.buffer));
  assert.equal(f.positions.length,m.count*3);assert.equal(f.endpoints.length,m.branches*2);
  assert.equal(f.segmentSources.length,m.segments*3);assert.equal(f.segmentTargets.length,m.segments*3);
  assert.equal(f.segmentBranches.length,m.segments);
  assert.equal(f.states.reduce((a,b)=>a+b,0),m.openTies);
  assert.equal(m.branches-m.openTies,m.count-1);
  // Closed branches form one connected spanning tree, while open ties never merge it.
  const parents=Uint32Array.from({length:m.count},(_,i)=>i);
  const root=i=>{while(parents[i]!==i){parents[i]=parents[parents[i]];i=parents[i];}return i;};
  for(let i=0;i<m.branches;i++) {
    const a=f.endpoints[i*2],b=f.endpoints[i*2+1];assert(a<m.count&&b<m.count&&a!==b);
    if(!f.states[i]){const ra=root(a),rb=root(b);assert.notEqual(ra,rb);parents[ra]=rb;}
  }
  for(let i=1;i<m.count;i++)assert.equal(root(i),root(0));
  for(const a of [f.positions,f.segmentSources,f.segmentTargets])assert(a.every(Number.isFinite));
  assert(f.segmentBranches.every(i=>i<m.branches));
  // Every branch's first/last rendered segment retains its declared bus endpoints.
  const first=new Int32Array(m.branches).fill(-1),last=new Int32Array(m.branches);
  for(let s=0;s<m.segments;s++){const b=f.segmentBranches[s];if(first[b]<0)first[b]=s;last[b]=s;}
  for(let b=0;b<m.branches;b++)for(let k=0;k<3;k++){
    assert.equal(f.segmentSources[first[b]*3+k],f.positions[f.endpoints[b*2]*3+k]);
    assert.equal(f.segmentTargets[last[b]*3+k],f.positions[f.endpoints[b*2+1]*3+k]);
  }
  assert.throws(()=>decodeFixture(m,new ArrayBuffer(4)));
  console.log(`${m.count}: deterministic bytes, finite geometry, endpoints, connected tree and ${m.openTies} open ties verified`);
}
