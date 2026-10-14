import test from 'node:test';
import assert from 'node:assert/strict';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
function model(parents){return {buses:parents.map((_,i)=>({id:`b${i}`,source:0})),branches:parents.flatMap((p,i)=>p===i?[]:[{from:p,to:i,source:0,disabled:false,family:'line'}]),attachments:[{family:'voltage_source',bus:0}],sources:[{id:'test',label:'Test'}]};}
function place(m,strategy='tidy-tree'){const x=makeSchematic(m,{strategy}),f=decodeFixture(x.manifest,x.buffer);validateSchematic(f);return f;}
function checkSpacing(f){
 const ranks=new Map(),children=new Map();
 for(let u=0;u<f.parent.length;u++){
  const key=`${f.component[u]}/${f.depth[u]}`;if(!ranks.has(key))ranks.set(key,[]);ranks.get(key).push(f.positions[u*3+1]);
  if(f.parent[u]!==u){if(!children.has(f.parent[u]))children.set(f.parent[u],[]);children.get(f.parent[u]).push(u);}
 }
 for(const row of ranks.values()){row.sort((a,b)=>a-b);for(let i=1;i<row.length;i++)assert.ok(row[i]-row[i-1]>=24-.002,`Insufficient rank spacing: ${row[i]-row[i-1]}`);}
 for(const [u,kids]of children){const ys=kids.map(v=>f.positions[v*3+1]);assert.ok(Math.abs(f.positions[u*3+1]-(Math.min(...ys)+Math.max(...ys))/2)<.002);}
}
test('asymmetric seven-bus regression separates a completed child before entering its sibling',()=>{
 const f=place(model([0,0,1,1,0,4,0]));checkSpacing(f);
 assert.equal(f.positions[6*3+1]-f.positions[4*3+1],24);
});
test('deterministic uneven trees preserve spacing, centering, identities and topology',()=>{
 let seed=42;const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296;};
 for(let run=0;run<200;run++){
  const m=model(Array.from({length:150},(_,i)=>i?Math.floor(random()*i):0)),before=JSON.stringify(m),f=place(m),old=place(m,'leaf-order');checkSpacing(f);
  assert.equal(JSON.stringify(m),before);assert.deepEqual(f.buffer,place(m).buffer);
  for(const key of ['parent','parentBranch','depth','component','branchRoles','endpoints','states'])assert.deepEqual(f[key],old[key]);
 }
});
test('tidy forest keeps disabled edges, loops, parallel circuits, isolated buses and source panels',()=>{
 const m=model([0,0,1,3,3,5]);m.sources.push({id:'other',label:'Other'});m.buses[5].source=1;
 m.branches.push({from:0,to:2,source:0},{from:0,to:1,source:0},{from:2,to:2,source:0},{from:2,to:3,source:0,disabled:true});
 const f=place(m),old=place(m,'leaf-order');checkSpacing(f);assert.equal(f.manifest.panels.length,2);
 for(const key of ['parent','parentBranch','depth','component','branchRoles','endpoints','states'])assert.deepEqual(f[key],old[key]);
 const m2=structuredClone(m);m2.buses.forEach(b=>b.coordinates=[999,888]);assert.deepEqual(f.buffer,place(m2).buffer);
});
for(const shape of ['chain','star'])test(`100k-bus ${shape} uses iterative passes and preserves spacing`,()=>{
 const n=100000,f=place(model(Array.from({length:n},(_,i)=>shape==='chain'?Math.max(0,i-1):0)));checkSpacing(f);assert.equal(f.manifest.layout.treeEdges,n-1);
});
test('unknown strategy fails explicitly',()=>assert.throws(()=>makeSchematic(model([0]),{strategy:'unknown'}),/strategy/));
