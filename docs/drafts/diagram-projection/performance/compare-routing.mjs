/** Offline comparison; reads external raw data, writes only aggregate evidence. */
import {readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {cpus,platform,arch} from 'node:os';
import assert from 'node:assert/strict';
import {adaptBMOPF,combineModels} from './bmopf-adapter.mjs';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {routingMetrics} from './routing-metrics.mjs';
import {decodeFixture} from './fixture.mjs';
const root=process.argv[2];if(!root)throw new Error('Usage: node compare-routing.mjs /external/BMOPFDraftData/output [report.json]');
const config=JSON.parse(await readFile(new URL('./bmopf-sources.json',import.meta.url)));
const collection=config.collections.find(c=>c.id==='sapn-combined');
const models=new Map();
for(const source of config.sources.filter(s=>s.id==='french-medium'||collection.members.includes(s.id))){
 const bytes=await readFile(resolve(root,source.file));models.set(source.id,adaptBMOPF(JSON.parse(bytes),{...source,sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length}));
}
models.set(collection.id,combineModels(collection.members.map(id=>models.get(id))));
const report={version:1,created:new Date().toISOString(),environment:{node:process.version,platform:platform(),arch:arch(),cpu:cpus()[0]?.model},methodology:'One warm-up per routing strategy followed by five interleaved runs. Tidy-tree placement in both; fan-out/corridor mode reserves busbar space and expands corridors. Times cover placement, routing and binary packing; validation and diagnostics outside timer. Exact collinear overlaps exclude endpoint contact; overlap length is the union of portions covered more than once. Proper crossings exclude endpoints and collinear sharing. Busbar-interior crossings count horizontal wires through vertical bus footprints (current mode uses centre +/- 1). Symbol and label collisions and screen-space separability are not measured.',datasets:[]};
for(const id of ['french-medium','sapn-combined']){
 const model=models.get(id),times={'current':[],'corridor':[]},fixtures={};
 for(const strategy of Object.keys(times))makeSchematic(model,{strategy:'tidy-tree',routing:strategy});
 for(let run=0;run<5;run++)for(const strategy of (run%2?['corridor','current']:['current','corridor'])){
  const result=makeSchematic(model,{strategy:'tidy-tree',routing:strategy});times[strategy].push(result.manifest.layout.totalMs);
  const f=decodeFixture(result.manifest,result.buffer);validateSchematic(f);fixtures[strategy]=f;
 }
 for(const key of ['parent','parentBranch','depth','component','branchRoles','endpoints','states'])assert.deepEqual(fixtures.corridor[key],fixtures.current[key]);
 const row={id,buses:model.buses.length,branches:model.branches.length,sourceHashes:model.sources.map(s=>({id:s.id,sha256:s.sha256})),routing:{}};
 for(const strategy of Object.keys(times)){
  const m=routingMetrics(fixtures[strategy]);
  row.routing[strategy]={...m,corridors:fixtures[strategy].manifest.routing,timingMs:times[strategy],medianMs:[...times[strategy]].sort((a,b)=>a-b)[2]};
 }
 report.datasets.push(row);console.log(JSON.stringify(row,null,2));
}
if(process.argv[3])await writeFile(process.argv[3],JSON.stringify(report,null,2)+'\n');
