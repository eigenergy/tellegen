/** Offline comparison; reads external raw data, writes only aggregate evidence. */
import {readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {cpus,platform,arch} from 'node:os';
import assert from 'node:assert/strict';
import {adaptBMOPF,combineModels} from './bmopf-adapter.mjs';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
const root=process.argv[2];if(!root)throw new Error('Usage: node compare-layouts.mjs /external/BMOPFDraftData/output [report.json]');
const config=JSON.parse(await readFile(new URL('./bmopf-sources.json',import.meta.url)));
const collection=config.collections.find(c=>c.id==='sapn-combined');
const models=new Map();
for(const source of config.sources.filter(s=>s.id==='french-medium'||collection.members.includes(s.id))){
 const bytes=await readFile(resolve(root,source.file));models.set(source.id,adaptBMOPF(JSON.parse(bytes),{...source,sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length}));
}
models.set(collection.id,combineModels(collection.members.map(id=>models.get(id))));
function metrics(f){
 const ranks=new Map();let minimumGap=Infinity;
 for(let u=0;u<f.manifest.count;u++){const k=`${f.component[u]}/${f.depth[u]}`;if(!ranks.has(k))ranks.set(k,[]);ranks.get(k).push(f.positions[u*3+1]);}
 for(const ys of ranks.values()){ys.sort((a,b)=>a-b);for(let j=1;j<ys.length;j++)minimumGap=Math.min(minimumGap,ys[j]-ys[j-1]);}
 // Count positive-length collinear overlap pairs between different branches.
 // Shared endpoint contact is excluded; shared trunks are included. This is
 // a routing diagnostic, NOT a count of transverse crossings or junctions.
 const groups=new Map();
 for(let i=0;i<f.manifest.segments;i++){
  const k=i*3,a=f.segmentSources,b=f.segmentTargets,vertical=a[k]===b[k],axis=vertical?1:0;
  const key=vertical?`v/${a[k]}`:`h/${a[k+1]}`;
  if(!groups.has(key))groups.set(key,[]);
  groups.get(key).push({start:Math.min(a[k+axis],b[k+axis]),end:Math.max(a[k+axis],b[k+axis]),owner:f.segmentBranches[i]});
 }
 let overlapPairs=0;
 for(const intervals of groups.values()){
  intervals.sort((a,b)=>a.start-b.start);const ends=[...intervals].sort((a,b)=>a.end-b.end),active=new Map();let cursor=0,count=0;
  for(const interval of intervals){
   while(cursor<ends.length&&ends[cursor].end<=interval.start){const e=ends[cursor++];active.set(e.owner,active.get(e.owner)-1);count--;}
   overlapPairs+=count-(active.get(interval.owner)??0);count++;active.set(interval.owner,(active.get(interval.owner)??0)+1);
  }
 }
 const bounds=f.manifest.bounds;
 return {bounds,width:bounds[2]-bounds[0],height:bounds[3]-bounds[1],minBusGapWithinRank:minimumGap===Infinity?null:minimumGap,collinearSegmentOverlapPairs:overlapPairs,segments:f.manifest.segments,geometryBytes:f.manifest.bytes,panels:f.manifest.panels.map(p=>({source:p.source,width:p.bounds[2]-p.bounds[0],height:p.bounds[3]-p.bounds[1]}))};
}
const report={version:1,created:new Date().toISOString(),environment:{node:process.version,platform:platform(),arch:arch(),cpu:cpus()[0]?.model},methodology:'One warm-up per strategy followed by five interleaved runs of placement, routing and binary packing. Decode/validation and metrics outside timer. Same forest, root order, spacing and routes; only subtree placement differs. Overlap pairs include shared trunks; transverse crossings, symbol/label collisions and browser frame timing are not measured. Float32 spacing tolerance: 0.01 diagram units.',datasets:[]};
for(const id of ['french-medium','sapn-combined']){
 const model=models.get(id),times={'leaf-order':[],'tidy-tree':[]},fixtures={};
 for(const strategy of Object.keys(times))makeSchematic(model,{strategy});
 for(let run=0;run<5;run++)for(const strategy of (run%2?['tidy-tree','leaf-order']:['leaf-order','tidy-tree'])){
  const result=makeSchematic(model,{strategy});times[strategy].push(result.manifest.layout.totalMs);
  const f=decodeFixture(result.manifest,result.buffer);validateSchematic(f);fixtures[strategy]=f;
 }
 for(const key of ['parent','parentBranch','depth','component','branchRoles','endpoints','states'])assert.deepEqual(fixtures['tidy-tree'][key],fixtures['leaf-order'][key]);
 const row={id,buses:model.buses.length,branches:model.branches.length,sourceHashes:model.sources.map(s=>({id:s.id,sha256:s.sha256})),layouts:{}};
 for(const strategy of Object.keys(times)){
  const m=metrics(fixtures[strategy]);assert.ok(m.minBusGapWithinRank===null||m.minBusGapWithinRank>=23.99,`${id} ${strategy}: insufficient spacing`);
  row.layouts[strategy]={...m,timingMs:times[strategy],medianMs:[...times[strategy]].sort((a,b)=>a-b)[2]};
 }
 report.datasets.push(row);console.log(JSON.stringify(row,null,2));
}
if(process.argv[3])await writeFile(process.argv[3],JSON.stringify(report,null,2)+'\n');
