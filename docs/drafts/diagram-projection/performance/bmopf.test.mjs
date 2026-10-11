import test from 'node:test';
import assert from 'node:assert/strict';
import {adaptBMOPF,combineModels,toFixture,sourceKey,busQuery} from './bmopf-adapter.mjs';
import {decodeFixture,makeFixture} from './fixture.mjs';
import {validateReferences} from './bmopf-references.mjs';
const source={id:'case-a',label:'Case A',file:'external.json',sha256:'test'};
function raw(){return {bus:{'b/<x>':{longitude:151,latitude:-27,terminal_names:['1','2','3','n'],perfectly_grounded_terminals:['n']},a:{longitude:151,latitude:-27,terminal_names:['1','2','3','n']}},line:{same:{bus_from:'a',bus_to:'b/<x>',terminal_map_from:['3','1','2','n'],terminal_map_to:['1','2','3','n'],status:0}},transformer:{delta_wye:{same:{bus_from:'a',bus_to:'b/<x>',terminal_map_from:['3','1','2'],terminal_map_to:['1','2','3','n']}}},load:{same:{bus:'b/<x>',terminal_map:['1','n'],configuration:'wye'}}};}
function fixture(model){const x=toFixture(model,'test','Test'),f=decodeFixture(x.manifest,x.buffer);f.refs=x.refs;validateReferences(f);return f;}
test('preserves exact IDs, family namespaces, terminal ordering, grounding and attachments',()=>{
  const input=raw(),before=JSON.stringify(input),f=fixture(adaptBMOPF(input,source));
  assert.equal(JSON.stringify(input),before);assert.equal(f.refs.buses[1].id,'b/<x>');assert.deepEqual(f.refs.buses[1].grounded,['n']);
  assert.deepEqual(f.refs.branches[0].terminalMapFrom,['3','1','2','n']);assert.equal(f.refs.branches[1].meaning,'equipment_association');
  assert.equal(f.refs.attachments[0].bus,1);assert.equal(f.states[0],1);
  assert.notEqual(sourceKey('a','line','same'),sourceKey('a','transformer','same','delta_wye'));
  assert.equal(busQuery('case-a','b/<x>'),'case-a/b%2F%3Cx%3E');
});
test('coincident buses stay separate and branches have owned, selectable display detours',()=>{
  const f=fixture(adaptBMOPF(raw(),source));assert.equal(f.manifest.coincidentBuses,1);assert.equal(f.manifest.coincidentBranches,2);
  assert.deepEqual([...f.positions.slice(0,3)],[...f.positions.slice(3)]);
  assert.equal(f.segmentBranches.length,7);assert.ok(f.segmentSources.some(n=>n!==0));
  assert.deepEqual([...f.segmentSources.slice(0,3)],[...f.positions.slice(0,3)]);
  assert.deepEqual([...f.segmentTargets.slice(-3)],[...f.positions.slice(3)]);
});
test('collections retain duplicate raw IDs and never invent cross-source edges',()=>{
  const a=adaptBMOPF(raw(),source),b=adaptBMOPF(raw(),{...source,id:'case-b'}),f=fixture(combineModels([a,b]));
  assert.equal(f.manifest.count,4);assert.equal(f.manifest.branches,4);assert.equal(f.refs.buses[0].id,f.refs.buses[2].id);
  assert.notEqual(f.positions[0],f.positions[6]);
  for(const e of f.refs.branches){assert.equal(f.refs.buses[e.from].source,e.source);assert.equal(f.refs.buses[e.to].source,e.source);}
  assert.throws(()=>combineModels([a,a]),/Duplicate source/);
  f.refs.branches[0].to=2;assert.throws(()=>validateReferences(f),/disagree/);
});
test('invalid inputs fail explicitly instead of losing topology or terminal meaning',()=>{
  for(const [mutate,pattern] of [
    [r=>delete r.bus.a.longitude,/longitude/],
    [r=>r.line.same.bus_to='absent',/unknown bus/],
    [r=>r.line.same.terminal_map_to=['z'],/unknown terminals/],
    [r=>r.line.same.terminal_map_to=['1'],/unequal/],
    [r=>r.line.same.status='open',/unsupported status/],
    [r=>r.line.same.enabled=true,/contradictory/],
    [r=>r.switch={s:{}},/Switch elaboration/],
    [r=>r.generator={g:{}},/Unsupported BMOPF/],
    [r=>r.transformer.unknown={t:{}},/Unsupported transformer/]
  ]){const r=raw();mutate(r);assert.throws(()=>adaptBMOPF(r,source),pattern);}
});
test('adaptation is deterministic regardless of object insertion order',()=>{
  const a=raw(),b=raw();b.bus=Object.fromEntries(Object.entries(b.bus).reverse());
  const f=toFixture(adaptBMOPF(a,source),'same','Same'),g=toFixture(adaptBMOPF(b,source),'same','Same');
  assert.deepEqual(f,g);
});
test('decoder supports small BMOPF cases and rejects corrupt references or non-finite geometry',()=>{
  const f=toFixture(adaptBMOPF(raw(),source),'same','Same');assert.equal(decodeFixture(f.manifest,f.buffer).positions.length,6);
  new Float32Array(f.buffer)[0]=NaN;assert.throws(()=>decodeFixture(f.manifest,f.buffer),/Non-finite/);
  const synthetic=makeFixture(1000);assert.equal(decodeFixture(synthetic.manifest,synthetic.buffer).positions.length,3000);
});
