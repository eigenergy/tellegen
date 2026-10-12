import test from 'node:test';
import assert from 'node:assert/strict';
import {buildOpenTiePresentation,incidentPresentation,compactBranch} from './open-ties.mjs';
import {adaptBMOPF,referenceData} from './bmopf-adapter.mjs';
import {makeSchematic} from './schematic.mjs';
import {decodeFixture,makeFixture} from './fixture.mjs';
import {nonTreeBranches,branchSegments} from './circuit-inspection.mjs';
function fixture(routing){
 const edge=(a,b,open,status=true)=>({bus_from:a,bus_to:b,terminal_map_from:['a'],terminal_map_to:['a'],open_switch:open,enabled:status});
 const model=adaptBMOPF({bus:Object.fromEntries(['a','b','c','d'].map(id=>[id,{terminal_names:['a']}])),
  line:{disabled:edge('a','b',false,false)},switch:{closed:edge('b','c',false),outOfService:edge('c','d',false,false),open:edge('a','c',true),parallel:edge('a','c',true),loop:edge('a','a',true),openOutOfService:edge('a','d',true,false)}}, {id:'case',allowMissingCoordinates:true});
 const data=makeSchematic(model,{strategy:'tidy-tree',routing}),f=decodeFixture(data.manifest,data.buffer);f.refs=referenceData(model);
 f.firstSegment=new Uint32Array(f.manifest.branches+1);let b=0;for(let s=0;s<f.manifest.segments;s++)while(b<=f.segmentBranches[s])f.firstSegment[b++]=s;f.firstSegment[b]=f.manifest.segments;
 return f;
}
test('compact view suppresses only explicit open switches; picking resolves original owners in every routing',()=>{
 for(const routing of ['current','corridor','crossing-aware']){
  const f=fixture(routing),before=f.buffer.slice(0),refs=JSON.stringify(f.refs),p=buildOpenTiePresentation(f);
  assert.equal(p.branchIndices.length,4);assert.equal(p.markers.length,3);
  const retained=new Set();
  for(let i=0;i<p.compact.segments.length;i++){
   const b=compactBranch(f,p,i),s=p.compact.segments[i];assert.equal(b,f.segmentBranches[s]);assert.ok(!p.branchSet.has(b));retained.add(b);
   assert.deepEqual([...p.compact.sources.subarray(i*3,i*3+3)],[...f.segmentSources.subarray(s*3,s*3+3)]);
   assert.deepEqual([...p.compact.targets.subarray(i*3,i*3+3)],[...f.segmentTargets.subarray(s*3,s*3+3)]);
  }
  assert.deepEqual([...retained].map(b=>f.refs.branches[b].id).sort(),['closed','disabled','outOfService']);
  assert.equal(compactBranch(f,p,-1),null);assert.equal(compactBranch(f,p,p.compact.segments.length),null);
  assert.deepEqual(f.buffer,before);assert.equal(JSON.stringify(f.refs),refs);
  assert.ok([...nonTreeBranches(f)].every(b=>!p.branchSet.has(b)));
 }
});
test('parallel ties share endpoint annotations without losing identity; self loops counted once per bus',()=>{
 const f=fixture('corridor'),p=buildOpenTiePresentation(f),a=f.refs.buses.findIndex(b=>b.id==='a');
 assert.equal(p.byBus.get(a).length,4);assert.equal(new Set(p.byBus.get(a)).size,4);
 assert.deepEqual(p.markers.find(m=>m.bus===a).position,f.positions.subarray(a*3,a*3+3));
 for(const b of p.branchIndices){const endpoints=new Set([f.endpoints[b*2],f.endpoints[b*2+1]]);for(const bus of endpoints)assert.ok(p.byBus.get(bus).includes(b));}
 const incident=Array.from({length:f.manifest.branches},(_,i)=>i);
 assert.ok(incidentPresentation(incident,p,true).every(b=>!p.branchSet.has(b)));assert.strictEqual(incidentPresentation(incident,p,false),incident);
 // Selection references the complete original route, including its gap, without restoring peers.
 const b=p.branchIndices[0],segments=branchSegments(f,[b]);assert.equal(segments.length,f.firstSegment[b+1]-f.firstSegment[b]);
 assert.ok(segments.slice(1).some((s,i)=>s.source.some((x,k)=>x!==segments[i].target[k])));
});
test('all-closed and synthetic scenes have a zero-copy fast path',()=>{
 const f=fixture('current');f.refs.branches.forEach(b=>b.openSwitch=false);
 const p=buildOpenTiePresentation(f);assert.equal(p.compact,null);assert.equal(p.arrayBytes,0);assert.equal(p.markers.length,0);assert.equal(compactBranch(f,p,0),null);
 delete f.refs;assert.equal(buildOpenTiePresentation(f).compact,null);
});
test('100k buses partition linearly with bounded array storage and exact dense-index remapping',()=>{
 const data=makeFixture(100000),f=decodeFixture(data.manifest,data.buffer);
 f.refs={branches:Array.from({length:f.manifest.branches},(_,i)=>({family:i%100===0?'switch':'line',openSwitch:i%100===0}))};
 const p=buildOpenTiePresentation(f);assert.ok(p.markers.length<=p.branchIndices.length*2);
 assert.equal(p.arrayBytes,p.compact.segments.length*28);
 for(let i=0;i<p.compact.segments.length;i++){const b=compactBranch(f,p,i);assert.ok(b%100!==0);assert.equal(b,f.segmentBranches[p.compact.segments[i]]);}
});
