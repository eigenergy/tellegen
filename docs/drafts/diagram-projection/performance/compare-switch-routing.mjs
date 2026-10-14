/** External MVLV case, explicit scenarios, aggregate diagnostics only. */
import {readFile,writeFile} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {createHash} from 'node:crypto';
import {cpus,platform,arch} from 'node:os';
import assert from 'node:assert/strict';
import {adaptBMOPF,referenceData,datasetMetadata} from './bmopf-adapter.mjs';
import {parseSwitchCommands,applySwitchScenario} from './switch-scenarios.mjs';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {routingMetrics} from './routing-metrics.mjs';
import {decodeFixture} from './fixture.mjs';
import {validateReferences} from './bmopf-references.mjs';
const root=process.argv[2];if(!root)throw new Error('Usage: node compare-switch-routing.mjs /external/BMOPFDraftData/output [report.json]');
const source=JSON.parse(await readFile(new URL('./bmopf-sources.json',import.meta.url))).sources.find(s=>s.id==='mvlv-meshed');
const hash=x=>createHash('sha256').update(x).digest('hex');
const bytes=await readFile(resolve(root,source.file)),commandBytes=await readFile(resolve(dirname(resolve(root)),source.switchCommands));
const base=adaptBMOPF(JSON.parse(bytes),{...source,sha256:hash(bytes),bytes:bytes.length}),commands=parseSwitchCommands(commandBytes.toString(),base);
const report={version:1,created:new Date().toISOString(),environment:{node:process.version,platform:platform(),arch:arch(),cpu:cpus()[0]?.model},
 methodology:'Tidy-tree; one warm-up per strategy, five interleaved layout/routing/packing runs. Validation and geometry diagnostics outside timers. Proper interior segment crossings exclude endpoint contacts and collinear sharing. Disabled/open routes included in whole-scene diagnostics. No symbol/label or screen-space collision measure. Raw data stays external; source and command hashes identify these reconstructed scenarios.',sourceHash:hash(bytes),commandHash:hash(commandBytes),datasets:[]};
for(const kind of source.scenarios){
 const model=applySwitchScenario(base,kind,commands,{commandFile:source.switchCommands,commandSha256:hash(commandBytes)}),fixtures={},times={current:[],corridor:[],'crossing-aware':[]};
 for(const routing of Object.keys(times))makeSchematic(model,{strategy:'tidy-tree',routing});
 for(let run=0;run<5;run++)for(const routing of (run%2?Object.keys(times).reverse():Object.keys(times))){
  const result=makeSchematic(model,{strategy:'tidy-tree',routing});times[routing].push(result.manifest.layout.totalMs);
  const f=decodeFixture({...result.manifest,...datasetMetadata(model,`${source.id}-${kind}`,kind)},result.buffer);f.refs=referenceData(model);validateReferences(f);validateSchematic(f);fixtures[routing]=f;
 }
 for(const routing of ['corridor','crossing-aware'])for(const key of ['endpoints','states','parent','parentBranch','depth','component','branchRoles'])assert.deepEqual(fixtures.current[key],fixtures[routing][key]);
 for(const key of ['positions','branchPorts','busbarSources','busbarTargets'])assert.deepEqual(fixtures.corridor[key],fixtures['crossing-aware'][key]);
 const row={id:kind,buses:model.buses.length,branches:model.branches.length,switches:fixtures.current.manifest.switches,openSwitches:fixtures.current.manifest.openSwitches,scenario:model.scenario,components:fixtures.current.manifest.layout.components.length,nonTreeConnections:fixtures.current.manifest.layout.crossLinks,routing:{}};
 for(const routing of Object.keys(times))row.routing[routing]={...routingMetrics(fixtures[routing]),ordering:fixtures[routing].manifest.routing.ordering??null,timingMs:times[routing],medianMs:[...times[routing]].sort((a,b)=>a-b)[2]};
 assert.ok(row.routing['crossing-aware'].properRouteCrossings<=row.routing.corridor.properRouteCrossings);
 report.datasets.push(row);console.log(JSON.stringify(row,null,2));
}
assert.equal(report.datasets[0].openSwitches,34);assert.equal(report.datasets[1].openSwitches,0);
assert.equal(report.datasets[0].nonTreeConnections,6);assert.equal(report.datasets[1].nonTreeConnections,40);
if(process.argv[3])await writeFile(process.argv[3],JSON.stringify(report,null,2)+'\n');
