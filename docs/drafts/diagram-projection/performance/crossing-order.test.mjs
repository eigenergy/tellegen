import test from 'node:test';
import assert from 'node:assert/strict';
import {improveOrder,orderCost,crossingAwareOrder,MAX_ORDERED_LANES} from './crossing-order.mjs';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
import {routingMetrics} from './routing-metrics.mjs';
const model=(n,edges)=>({buses:Array.from({length:n},(_,i)=>({id:`b${i}`,source:0})),branches:edges.map(([from,to,disabled=false])=>({from,to,disabled,source:0})),attachments:[{family:'voltage_source',bus:0}],sources:[{id:'test',label:'Test'}]});
const fixture=(m,routing='crossing-aware',strategy='tidy-tree')=>{const r=makeSchematic(m,{routing,strategy}),f=decodeFixture(r.manifest,r.buffer);validateSchematic(f);return f;};
const stable=['positions','branchPorts','busbarSources','busbarTargets','parent','parentBranch','depth','component','branchRoles','endpoints','states'];
test('mesh insertion can cross several tree lanes without reordering the tree',()=>{
 const cost=[[0,0,9,4],[0,0,2,3],[0,0,0,5],[0,0,0,0]],r=improveOrder(cost,2);
 assert.deepEqual(r.order.filter(i=>i<2),[0,1]);assert.deepEqual(new Set(r.order),new Set([0,1,2,3]));assert.equal(orderCost(r.order,cost),0);assert.ok(r.moves>0);
 const zero=Array.from({length:5},()=>new Float64Array(5));assert.deepEqual(improveOrder(zero,2).order,[0,1,2,3,4]);
});
test('cost model equals strict geometric crossings in randomized mesh fixtures; no identity or footprint changes',()=>{
 let seed=719;const rand=n=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed%n;};let reduced=0;
 for(let run=0;run<100;run++){
  const edges=Array.from({length:39},(_,i)=>[rand(i+1),i+1]);for(let i=0;i<15;i++)edges.push([rand(40),rand(40)]);
  const m=model(40,edges),before=JSON.stringify(m);
  for(const strategy of ['tidy-tree','leaf-order']){
   const a=fixture(m,'corridor',strategy),b=fixture(m,'crossing-aware',strategy),am=routingMetrics(a),bm=routingMetrics(b),o=b.manifest.routing.ordering;
   for(const key of stable)assert.deepEqual(a[key],b[key]);assert.equal(am.geometryBytes,bm.geometryBytes);assert.equal(am.segments,bm.segments);
   assert.equal(am.properRouteCrossings-bm.properRouteCrossings,o.before-o.after);
   assert.ok(bm.properRouteCrossings<=am.properRouteCrossings);assert.equal(bm.collinearOverlapPairs,0);assert.equal(bm.horizontalWireBusbarInteriorCrossings,0);
   if(bm.properRouteCrossings<am.properRouteCrossings)reduced++;
   assert.deepEqual(b.buffer,fixture(m,'crossing-aware',strategy).buffer);
  }
  assert.equal(JSON.stringify(m),before);
 }
 assert.ok(reduced>0);
});
test('radial tree geometry is byte-identical to corridor routing',()=>{
 const m=model(7,[[0,1],[0,4],[0,6],[1,2],[1,3],[4,5]]),a=fixture(m,'corridor'),b=fixture(m);
 assert.deepEqual(a.buffer,b.buffer);assert.equal(b.manifest.routing.ordering.eligible,0);assert.equal(routingMetrics(b).properRouteCrossings,0);
});
test('self loops, parallel circuits, disabled gaps and exterior escapes stay owned and valid',()=>{
 const m=model(8,[[0,1],[1,2],[2,0],[0,1],[1,1],[2,3,true],[3,4],[4,5],[0,5,true],[6,7]]),a=fixture(m,'corridor'),b=fixture(m);
 for(const key of stable)assert.deepEqual(a[key],b[key]);assert.equal(b.manifest.routing.externalLinks,2);assert.ok(b.manifest.routing.ordering.skipped['external-or-disabled']>0);
 assert.equal(routingMetrics(a).properRouteCrossings-routingMetrics(b).properRouteCrossings,b.manifest.routing.ordering.before-b.manifest.routing.ordering.after);
});
test('wide corridors fall back before constructing quadratic cost matrices',()=>{
 const n=MAX_ORDERED_LANES+100,m=model(n+1,[...Array.from({length:n},(_,i)=>[0,i+1]),[0,1]]),a=fixture(m,'corridor'),b=fixture(m);
 assert.ok(b.manifest.routing.ordering.skipped['lane-cap']>0);assert.deepEqual(a.buffer,b.buffer);
 const intervals=[{bank:2,id:0,lane:0}];assert.equal(crossingAwareOrder({count:100000,intervals},{}).skipped,'lane-cap');
});
test('100k-bus chain remains iterative with no candidate lane matrices',()=>{
 const n=100000,f=fixture(model(n,Array.from({length:n-1},(_,i)=>[i,i+1])));
 assert.equal(f.manifest.layout.treeEdges,n-1);assert.equal(f.manifest.routing.ordering.eligible,0);
});
