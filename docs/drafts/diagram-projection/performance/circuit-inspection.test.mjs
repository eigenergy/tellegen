import test from 'node:test';
import assert from 'node:assert/strict';
import {treePath,inspectCircuit,nonTreeBranches,branchSegments,overlayBounds} from './circuit-inspection.mjs';
function forest(parent){
 const n=parent.length,depth=new Uint32Array(n),component=new Uint32Array(n),parentBranch=new Uint32Array(n).fill(0xffffffff),endpoints=[];let root=0;
 for(let u=0;u<n;u++){if(parent[u]===u){root=u;continue;}depth[u]=depth[parent[u]]+1;component[u]=root;parentBranch[u]=endpoints.length/2;endpoints.push(parent[u],u);}
 for(let u=0;u<n;u++)if(parent[u]===u)component[u]=u;
 const branches=endpoints.length/2;
 return {manifest:{count:n,branches},parent:Uint32Array.from(parent),parentBranch,depth,component,endpoints,branchRoles:new Array(branches).fill(0),states:new Array(branches).fill(0)};
}
function tie(f,a,b,disabled=false){const i=f.manifest.branches++;f.endpoints.push(a,b);f.branchRoles.push(disabled?2:1);f.states.push(Number(disabled));return i;}
function walk(f,path,a,b){assert.equal(path.nodes[0],a);assert.equal(path.nodes.at(-1),b);assert.equal(path.edges.length,path.nodes.length-1);assert.equal(new Set(path.nodes).size,path.nodes.length);for(let i=0;i<path.edges.length;i++)assert.deepEqual(new Set(f.endpoints.slice(path.edges[i]*2,path.edges[i]*2+2)),new Set(path.nodes.slice(i,i+2)));}
test('path stops at common ancestor; does not include shared route to root',()=>{
 const f=forest([0,0,1,1,2,3]);const b=tie(f,4,5),p=inspectCircuit(f,b);
 assert.deepEqual(p.nodes,[4,2,1,3,5]);assert.deepEqual(p.edges,[3,1,2,4]);assert.equal(p.ancestor,1);walk(f,p,4,5);
 assert.deepEqual(treePath(f,5,4).nodes,[5,3,1,2,4]);assert.deepEqual(treePath(f,1,4).nodes,[1,2,4]);
});
test('parallel circuits retain separate identities; self-loop and disconnected components are explicit',()=>{
 const f=forest([0,0,2,2]);const a=tie(f,0,1),b=tie(f,0,1),self=tie(f,1,1),external=tie(f,1,3),disabled=tie(f,1,0,true);
 assert.deepEqual([...nonTreeBranches(f)],[a,b,self,external]);assert.equal(inspectCircuit(f,a).branch,a);assert.equal(inspectCircuit(f,b).branch,b);
 assert.deepEqual(inspectCircuit(f,a).edges,[0]);assert.deepEqual(inspectCircuit(f,self).nodes,[1]);assert.deepEqual(inspectCircuit(f,self).edges,[]);
 assert.equal(inspectCircuit(f,external).connected,false);assert.throws(()=>inspectCircuit(f,disabled),/enabled/);assert.throws(()=>inspectCircuit(f,0),/non-tree/);
 assert.equal(nonTreeBranches({}).length,0);assert.throws(()=>treePath({},0,1),/unavailable/);
});
test('random endpoint paths match an independent ancestor-chain oracle',()=>{
 let seed=1459;const rand=n=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed%n;};
 const parents=[0];for(let i=1;i<1000;i++)parents.push(rand(i));const f=forest(parents);
 const ancestors=n=>{const out=[n];while(f.parent[n]!==n){n=f.parent[n];out.push(n);}return out;};
 for(let i=0;i<2000;i++){const a=rand(1000),b=rand(1000),left=ancestors(a),right=ancestors(b),common=left.find(n=>right.includes(n)),expected=left.slice(0,left.indexOf(common)+1).concat(right.slice(0,right.indexOf(common)).reverse()),p=treePath(f,a,b);assert.deepEqual(p.nodes,expected);walk(f,p,a,b);}
});
test('deep 100k tree is iterative; corrupt parent/state fails instead of looping',()=>{
 const f=forest(Array.from({length:100000},(_,i)=>Math.max(0,i-1))),b=tie(f,0,99999),p=inspectCircuit(f,b);assert.equal(p.edges.length,99999);assert.equal(p.nodes.length,100000);
 f.parent[99999]=99999;assert.throws(()=>inspectCircuit(f,b),/Invalid/);
 const small=forest([0,0,1]);small.states[1]=1;assert.throws(()=>treePath(small,2,0),/tree branch/);
});
test('overlays retain segment order and backing buffers; fit includes detours and busbar extents',()=>{
 const f={firstSegment:Uint32Array.from([0,1,3]),segmentSources:Float32Array.from([0,0,0,10,0,0,10,20,0]),segmentTargets:Float32Array.from([1,0,0,10,20,0,0,20,0]),positions:Float32Array.from([0,0,0,1,0,0]),busbarSources:Float32Array.from([0,-3,0,1,-2,0]),busbarTargets:Float32Array.from([0,4,0,1,2,0])};
 const s=branchSegments(f,[1]);assert.equal(s.length,2);assert.equal(s[0].source.buffer,f.segmentSources.buffer);assert.deepEqual([...s[1].target],[0,20,0]);assert.deepEqual(overlayBounds(f,[0,1],s),[0,-3,10,20]);
});
