import test from 'node:test';
import assert from 'node:assert/strict';
import {adaptBMOPF,toFixture,datasetMetadata,referenceData,sourceKey} from './bmopf-adapter.mjs';
import {parseSwitchCommands,applySwitchScenario} from './switch-scenarios.mjs';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
import {validateReferences} from './bmopf-references.mjs';
import {nonTreeBranches} from './circuit-inspection.mjs';
const source={id:'case',sha256:'test',allowMissingCoordinates:true};
function raw(){return {bus:Object.fromEntries(['a','b','c'].map(id=>[id,{terminal_names:['a','n']} ])),
  line:{same:{bus_from:'a',bus_to:'b',terminal_map_from:['a','n'],terminal_map_to:['a','n']}},
  switch:{same:{bus_from:'b',bus_to:'c',terminal_map_from:['n','a'],terminal_map_to:['n','a'],open_switch:true},
    named_OPEN:{bus_from:'a',bus_to:'c',terminal_map_from:['a','n'],terminal_map_to:['a','n'],open_switch:false}}};}
function fixture(model,routing){const result=makeSchematic(model,{strategy:'tidy-tree',routing}),f=decodeFixture({...result.manifest,...datasetMetadata(model,'scenario','Scenario')},result.buffer);f.refs=referenceData(model);validateReferences(f);validateSchematic(f);return f;}
test('switch state is explicit, terminal order and family identity survive every route',()=>{
 const input=raw(),before=JSON.stringify(input),m=adaptBMOPF(input,source),open=m.branches.find(b=>b.id==='same'&&b.family==='switch');
 assert.equal(JSON.stringify(input),before);assert.equal(open.openSwitch,true);assert.equal(open.outOfService,false);assert.deepEqual(open.terminalMapFrom,['n','a']);
 assert.notEqual(sourceKey('case','line','same'),sourceKey('case','switch','same'));
 assert.equal(m.branches.find(b=>b.id==='named_OPEN').openSwitch,false);
 for(const routing of ['current','corridor','crossing-aware']){
  const f=fixture(m,routing),i=m.branches.indexOf(open);assert.equal(f.branchRoles[i],2);assert.ok(![...f.parentBranch].includes(i));assert.ok(![...nonTreeBranches(f)].includes(i));
  const parts=[];for(let s=0;s<f.manifest.segments;s++)if(f.segmentBranches[s]===i)parts.push(s);
  assert.ok(parts.slice(1).some((s,k)=>[0,1].some(a=>f.segmentTargets[parts[k]*3+a]!==f.segmentSources[s*3+a])),'open switch has an actual geometry gap');
 }
});
test('scenario overrides do not mutate imported states, identities or out-of-service status',()=>{
 const input=raw();input.switch.same.enabled=false;const m=adaptBMOPF(input,source),before=JSON.stringify(m);
 const commands=parseSwitchCommands('! not a command\nopen line.NAMED_OPEN term=1 ! explicit\n',m),documented=applySwitchScenario(m,'documented',commands),closed=applySwitchScenario(m,'all-closed');
 assert.equal(JSON.stringify(m),before);assert.equal(documented.branches[1].openSwitch,true);assert.equal(documented.branches[1].sourceOpenSwitch,false);
 assert.equal(closed.branches[2].openSwitch,false);assert.equal(closed.branches[2].outOfService,true);assert.equal(closed.branches[2].disabled,true);
 assert.deepEqual(documented.branches.map(b=>[b.id,b.family,b.source,b.from,b.to]),closed.branches.map(b=>[b.id,b.family,b.source,b.from,b.to]));
 const healthy=adaptBMOPF(raw(),source),all=fixture(applySwitchScenario(healthy,'all-closed'),'crossing-aware');assert.equal(all.manifest.layout.crossLinks,1);assert.equal(nonTreeBranches(all).length,1);
 const f=fixture(closed,'corridor');f.refs.branches[2].openSwitch='false';assert.throws(()=>validateReferences(f),/switch state/);
});
test('switch states and command mapping fail closed on ambiguity',()=>{
 for(const state of [undefined,null,0,1,'false',[false,false]]){const r=raw();r.switch.same.open_switch=state;assert.throws(()=>adaptBMOPF(r,source),/scalar boolean/);}
 const unequal=raw();unequal.switch.same.terminal_map_to=['a'];assert.throws(()=>adaptBMOPF(unequal,source),/unequal/);
 const m=adaptBMOPF(raw(),source);
 for(const text of ['open line.absent term=1','open line.same term=1\nclose line.same term=1','open line.same term=2','open line.same term=1 conductor=1',''])assert.throws(()=>parseSwitchCommands(text,m));
 assert.throws(()=>applySwitchScenario(m,'documented'),/explicit commands/);
});
test('coordinate-free intake is opt-in, cannot be used as geographic geometry, and rejects partial coverage',()=>{
 const input=raw();assert.throws(()=>adaptBMOPF(input,{id:'case'}),/longitude/);
 const m=adaptBMOPF(input,source);assert.equal(m.hasCoordinates,false);assert.equal(m.sources[0].projection.kind,'unavailable');assert.ok(m.buses.every(b=>b.coordinates===null));
 assert.throws(()=>toFixture(m,'case','Case'),/Source coordinates unavailable/);fixture(m,'current');
 input.bus.a.longitude=150;input.bus.a.latitude=-27;assert.throws(()=>adaptBMOPF(input,source),/Partial coordinate/);
 delete input.bus.a.latitude;assert.throws(()=>adaptBMOPF(input,source),/longitude/);
});
