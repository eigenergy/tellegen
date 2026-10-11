import test from 'node:test';
import assert from 'node:assert/strict';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
import {allocateLanes} from './corridor-routing.mjs';
import {routingMetrics} from './routing-metrics.mjs';
function model(n,edges){return {buses:Array.from({length:n},(_,i)=>({id:`b${i}`,source:0})),branches:edges.map(([from,to,disabled=false])=>({from,to,disabled,source:0})),attachments:[{family:'voltage_source',bus:0}],sources:[{id:'test',label:'Test'}]};}
function fixture(m,routing='corridor',strategy='tidy-tree'){const x=makeSchematic(m,{routing,strategy}),f=decodeFixture(x.manifest,x.buffer);validateSchematic(f);return f;}
test('interval colouring reuses only clear lanes, including coincident zero-length spans',()=>{
 const a=[{id:0,start:0,end:10},{id:1,start:2,end:7},{id:2,start:11,end:12},{id:3,start:11,end:11}];assert.equal(allocateLanes(a),2);
 for(let i=0;i<a.length;i++)for(let j=i+1;j<a.length;j++)if(a[i].lane===a[j].lane)assert.ok(a[i].end<a[j].start||a[j].end<a[i].start);
});
test('fan-out removes shared branch trunks and preserves endpoints and forest for both placements',()=>{
 const m=model(7,[[0,1],[0,4],[0,6],[1,2],[1,3],[4,5]]),before=JSON.stringify(m);
 for(const strategy of ['tidy-tree','leaf-order']){const f=fixture(m,'corridor',strategy),old=fixture(m,'current',strategy);
  assert.ok(routingMetrics(old).collinearOverlapPairs>0);assert.equal(routingMetrics(f).collinearOverlapPairs,0);
  for(const key of ['parent','parentBranch','depth','component','branchRoles','endpoints','states'])assert.deepEqual(f[key],old[key]);
  assert.deepEqual(f.buffer,fixture(m,'corridor',strategy).buffer);
 }
 assert.equal(JSON.stringify(m),before);
});
test('parallel circuits, loops, mesh and disabled inter-component links stay distinct and owned',()=>{
 const m=model(7,[[0,1],[1,2],[2,0],[0,1],[1,1],[2,3,true],[3,4],[4,5],[0,5,true]]),f=fixture(m);
 assert.equal(f.manifest.routing.externalLinks,2);assert.equal(routingMetrics(f).collinearOverlapPairs,0);
 const ports=new Set();for(let b=0;b<m.branches.length;b++)for(let end=0;end<2;end++){const k=b*6+end*3,key=`${f.endpoints[b*2+end]}/${f.branchPorts[k+1]}`;assert.ok(!ports.has(key));ports.add(key);}
 assert.notEqual(f.branchPorts[4*6+1],f.branchPorts[4*6+4]);
});
test('high-degree busbar space and multiple source panels do not merge buses',()=>{
 const m=model(502,Array.from({length:500},(_,i)=>[0,i+1]));m.sources.push({id:'island',label:'Island'});m.buses[501].source=1;
 const f=fixture(m);assert.equal(f.manifest.panels.length,2);assert.equal(routingMetrics(f).collinearOverlapPairs,0);assert.ok(f.busbarTargets[1]-f.busbarSources[1]>1900);
});
test('100k-bus chain and many parallel intervals avoid quadratic scans and recursion',()=>{
 const n=100000,f=fixture(model(n,Array.from({length:n-1},(_,i)=>[i,i+1])));assert.equal(f.manifest.layout.treeEdges,n-1);assert.ok(f.manifest.segments<=3*(n-1));
 const intervals=Array.from({length:n},(_,id)=>({id,start:0,end:1}));assert.equal(allocateLanes(intervals),n);
});
test('corrupt port ownership, route endpoints and missing bar geometry fail validation',()=>{
 const m=model(3,[[0,1],[0,2]]),f=fixture(m);f.branchPorts[0]+=1;assert.throws(()=>validateSchematic(f),/Port/);
 const g=fixture(m);g.segmentSources[0]+=1;assert.throws(()=>validateSchematic(g),/endpoint/);
 const h=fixture(m);h.busbarSources=undefined;assert.throws(()=>validateSchematic(h),/Missing/);
 assert.throws(()=>makeSchematic(m,{routing:'missing'}),/routing/);
});
test('uneven radial fans retain zero proper crossings with lane reuse',()=>{
 let seed=71;const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296;};
 for(let i=0;i<100;i++){
  const m=model(80,Array.from({length:79},(_,j)=>[Math.floor(random()*(j+1)),j+1])),metrics=routingMetrics(fixture(m));
  assert.equal(metrics.collinearOverlapPairs,0);assert.equal(metrics.properRouteCrossings,0);assert.equal(metrics.horizontalWireBusbarInteriorCrossings,0);
 }
});
test('diagnostics distinguish collinear overlap, proper crossings and endpoint contact',()=>{
 const f={manifest:{count:1,branches:4,segments:4,bytes:0,bounds:[0,-2,15,2]},positions:new Float32Array([7,0,0]),segmentSources:new Float32Array([0,0,0,5,0,0,7,-2,0,15,-2,0]),segmentTargets:new Float32Array([10,0,0,15,0,0,7,2,0,15,2,0]),segmentBranches:new Uint32Array([0,1,2,3])};
 const m=routingMetrics(f);assert.equal(m.collinearOverlapPairs,1);assert.equal(m.collinearOverlapLength,5);assert.equal(m.properRouteCrossings,2);assert.equal(m.horizontalWireBusbarInteriorCrossings,2);
});
