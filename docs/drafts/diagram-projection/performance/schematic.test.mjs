import test from 'node:test';
import assert from 'node:assert/strict';
import {makeSchematic,validateSchematic,NO_BRANCH,ROLE} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
function model(n,pairs,sources=1){return {positions:new Float32Array(n*3),buses:Array.from({length:n},(_,i)=>({id:`b${i}`,source:0,coordinates:[i,i]})),branches:pairs.map(([from,to,disabled=false],i)=>({id:`e${i}`,from,to,disabled,source:0,family:'line'})),attachments:[{id:'grid',family:'voltage_source',bus:0,disabled:false}],sources:Array.from({length:sources},(_,i)=>({id:`source${i}`,label:`Source ${i}`}))};}
function decoded(m){const x=makeSchematic(m),f=decodeFixture(x.manifest,x.buffer);validateSchematic(f);return f;}
function endpoints(f){let prev=-1;for(let s=0;s<f.manifest.segments;s++){const b=f.segmentBranches[s];assert.ok(b>=prev);if(b!==prev)assert.deepEqual([...f.segmentSources.slice(s*3,s*3+3)],[...f.positions.slice(f.endpoints[b*2]*3,f.endpoints[b*2]*3+3)]);if(s===f.manifest.segments-1||f.segmentBranches[s+1]!==b)assert.deepEqual([...f.segmentTargets.slice(s*3,s*3+3)],[...f.positions.slice(f.endpoints[b*2+1]*3,f.endpoints[b*2+1]*3+3)]);prev=b;}}
test('radial feeder is rooted at the voltage source, independent of coordinates',()=>{
 const m=model(6,[[3,4],[4,2],[3,0],[0,1],[1,5]]);m.attachments[0].bus=3;
 const before=JSON.stringify(m),f=decoded(m);assert.equal(JSON.stringify(m),before);assert.equal(f.manifest.layout.components[0].root,3);assert.equal(f.parent[3],3);assert.equal(f.parentBranch[3],NO_BRANCH);assert.equal(f.manifest.layout.treeEdges,5);assert.deepEqual([...f.depth],[1,2,2,0,1,3]);endpoints(f);
 const a=makeSchematic(m);m.positions.fill(987);m.buses.forEach(b=>b.coordinates=[-10,40]);assert.deepEqual(a.buffer,makeSchematic(m).buffer);
});
test('cycles, parallel edges, self loops and disabled links retain endpoint identities',()=>{
 const m=model(5,[[0,1],[1,2],[2,0],[0,1],[2,2],[2,3,true],[3,4]]),f=decoded(m);endpoints(f);
 assert.equal(f.manifest.layout.components.length,2);assert.equal(f.manifest.layout.treeEdges,3);assert.equal(f.manifest.layout.crossLinks,3);assert.equal(f.manifest.layout.disabledEdges,1);assert.equal(f.branchRoles[5],ROLE.DISABLED);assert.equal(f.states[5],1);
 assert.equal(f.parent[3],3);assert.equal(f.manifest.layout.components[1].rootReason,'stable-ID fallback');
 assert.deepEqual([...f.endpoints],m.branches.flatMap(b=>[b.from,b.to]));
});
test('multiple sources in one component use one deterministic tree root, retaining all source markers',()=>{
 const m=model(3,[[0,1],[1,2]]);m.attachments.push({family:'voltage_source',bus:2,disabled:false});const f=decoded(m);
 assert.equal(f.manifest.layout.components.length,1);assert.deepEqual(f.manifest.layout.sourceBuses,[0,2]);assert.equal(f.manifest.layout.components[0].root,0);
});
test('collections keep source panels, isolated buses, and transformer association edges',()=>{
 const m=model(5,[[0,1],[2,3]],2);for(let i=2;i<5;i++)m.buses[i].source=1;m.branches[1].source=1;m.branches[1].family='transformer';
 const f=decoded(m);assert.equal(f.manifest.panels.length,2);assert.equal(f.manifest.layout.components.length,3);endpoints(f);
 m.branches.push({from:0,to:2,source:0});assert.throws(()=>makeSchematic(m),/source-local/);
});
test('100k-bus chain avoids recursive-stack failure and preserves every bus and edge',()=>{
 const n=100000,m=model(n,Array.from({length:n-1},(_,i)=>[i,i+1])),f=decoded(m);assert.equal(f.depth[n-1],n-1);assert.equal(f.manifest.layout.treeEdges,n-1);assert.equal(f.manifest.segments,n-1);endpoints(f);
});
test('parent cycles and non-orthogonal routes are rejected',()=>{
 const f=decoded(model(3,[[0,1],[0,2]]));f.parent[1]=1;assert.throws(()=>validateSchematic(f),/root/);
 const g=decoded(model(3,[[0,1],[0,2]]));g.segmentTargets[1]+=1;assert.throws(()=>validateSchematic(g),/orthogonal/);
});
