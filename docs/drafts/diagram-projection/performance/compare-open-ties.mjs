/** Read ignored staged fixtures; commit only aggregate presentation diagnostics. */
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {cpus,platform,arch} from 'node:os';
import assert from 'node:assert/strict';
import {decodeFixture} from './fixture.mjs';
import {validateReferences} from './bmopf-references.mjs';
import {buildOpenTiePresentation,compactBranch} from './open-ties.mjs';
import {routingMetrics} from './routing-metrics.mjs';
const root=new URL('./local-data/',import.meta.url),catalog=JSON.parse(await readFile(new URL('manifest.json',root)));
const hash=x=>createHash('sha256').update(x).digest('hex');
const report={version:1,created:new Date().toISOString(),environment:{node:process.version,platform:platform(),arch:arch(),cpu:cpus()[0]?.model},
 methodology:'Precomputed tidy-tree geometry; one warm-up and five presentation preparation runs per scenario/routing pair. Timers cover open-switch grouping and cached coordinate/pick-index copies, excluding color copies, validation and diagnostics. Compact counts omit suppressed routes; original endpoints, geometry, state and forest are unchanged. Proper segment crossings exclude endpoints/collinear sharing. Endpoint ring screen collisions are not measured. Additional array bytes include browser compact colors; exclude JS objects, GPU copies and library overhead.',datasets:[]};
for(const id of ['mvlv-meshed-documented','mvlv-meshed-all-closed','sapn-combined']){
 const base=catalog.find(x=>x.id===id);assert.ok(base,'Stage external BMOPF sources first');
 const refsBytes=await readFile(new URL(base.refsFile,root));assert.equal(hash(refsBytes),base.refsSha256);const refs=JSON.parse(refsBytes);
 for(const routing of (id==='sapn-combined'?['crossing-aware']:['current','corridor','crossing-aware'])){
  const m={...base,...base.layouts['tidy-tree'+(routing==='current'?'':'-'+routing)]},bytes=await readFile(new URL(m.file,root));assert.equal(hash(bytes),m.sha256);
  const f=decodeFixture(m,bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength));f.refs=refs;validateReferences(f);
  buildOpenTiePresentation(f);const times=[];let p;
  for(let i=0;i<5;i++){const t=performance.now();p=buildOpenTiePresentation(f);times.push(performance.now()-t);}
  const c=p.compact,owners=c?Uint32Array.from(c.segments,(_,i)=>compactBranch(f,p,i)):f.segmentBranches;
  const compact=c?{...f,manifest:{...m,segments:c.segments.length},segmentSources:c.sources,segmentTargets:c.targets,segmentBranches:owners}:f;
  assert.ok([...owners].every(b=>!p.branchSet.has(b)));assert.equal(hash(new Uint8Array(f.buffer)),m.sha256);
  const counts=new Map();for(const b of f.segmentBranches)if(p.branchSet.has(b))counts.set(b,(counts.get(b)??0)+1);
  const row={id,routing,buses:m.count,branches:m.branches,sourceHashes:refs.sources.map(s=>({id:s.id,sha256:s.sha256})),geometryHash:m.sha256,
   openSwitches:p.branchIndices.length,endpointMarkers:p.markers.length,suppressedSegments:p.suppressedSegments,maximumSelectedRouteSegments:Math.max(0,...counts.values()),
   extraArrayBytes:p.arrayBytes+(c?c.segments.length*4:0),prepareMs:times,medianPrepareMs:[...times].sort((a,b)=>a-b)[2],
   full:routingMetrics(f),compact:routingMetrics(compact)};
  report.datasets.push(row);console.log(JSON.stringify(row));
 }
}
if(process.argv[2])await writeFile(process.argv[2],JSON.stringify(report,null,2)+'\n');
