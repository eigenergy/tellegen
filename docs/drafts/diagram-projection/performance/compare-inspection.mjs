/** Local derived geometry only. No coordinates or source payloads in output. */
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {resolve,dirname} from 'node:path';
import assert from 'node:assert/strict';
import {decodeFixture} from './fixture.mjs';
import {nonTreeBranches,inspectCircuit,branchSegments} from './circuit-inspection.mjs';
const here=dirname(fileURLToPath(import.meta.url)),catalog=JSON.parse(await readFile(resolve(here,'local-data/manifest.json'),'utf8'));
const results=[];
for(const id of ['french-medium','sapn-combined']){
 const entry=catalog.find(m=>m.id===id),m={...entry,...entry.layouts['tidy-tree-corridor']},bytes=await readFile(resolve(here,'local-data',m.file)),f=decodeFixture(m,bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength));
 f.firstSegment=new Uint32Array(m.branches+1);let b=0;for(let s=0;s<m.segments;s++)while(b<=f.segmentBranches[s])f.firstSegment[b++]=s;f.firstSegment[m.branches]=m.segments;
 const start=performance.now(),indices=nonTreeBranches(f),indexMs=performance.now()-start;let maxPath=0,maxBranch=null,maxSegments=0;const times=[];
 for(let pass=0;pass<6;pass++)for(const branch of indices){
  const start=performance.now(),p=inspectCircuit(f,branch),segments=branchSegments(f,[branch,...p.edges]),duration=performance.now()-start;
  if(pass)times.push(duration);
  assert.ok(p.connected);assert.equal(p.nodes[0],p.endpoints[0]);assert.equal(p.nodes.at(-1),p.endpoints[1]);assert.equal(p.nodes.length,p.edges.length+1);assert.equal(new Set(p.nodes).size,p.nodes.length);
  for(let i=0;i<p.edges.length;i++)assert.deepEqual(new Set(f.endpoints.slice(p.edges[i]*2,p.edges[i]*2+2)),new Set(p.nodes.slice(i,i+2)));
  if(p.edges.length>maxPath){maxPath=p.edges.length;maxBranch=branch;}maxSegments=Math.max(maxSegments,segments.length);
 }
 times.sort((a,b)=>a-b);
 results.push({id,buses:m.count,geometrySha256:m.sha256,nonTreeConnections:indices.length,indexBytes:indices.byteLength,indexMs,maxTreePathEdges:maxPath,maxTreePathBranch:maxBranch,maxOverlaySegments:maxSegments,samples:times.length,p50PathAndOverlayMs:times[Math.floor(times.length*.5)]??null,p95PathAndOverlayMs:times[Math.floor(times.length*.95)]??null,maxPathAndOverlayMs:times.at(-1)??null,allPathsVerified:true});
}
console.log(JSON.stringify({node:process.version,platform:process.platform,arch:process.arch,methodology:'One warm-up and five passes over every enabled non-tree connection; path + segment-overlay CPU time only, excludes assertions, UI, busbar objects and drawing. Local derived data; no raw payload copied.',results},null,2));
