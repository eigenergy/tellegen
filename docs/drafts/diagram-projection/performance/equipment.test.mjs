import test from 'node:test';
import assert from 'node:assert/strict';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
import {prepareEquipment,equipmentRow} from './equipment.mjs';
import {equipmentClearanceMetrics} from './equipment-metrics.mjs';
import {corridorRouting} from './corridor-routing.mjs';
import {buildOpenTiePresentation} from './open-ties.mjs';
function model(){return {sources:[{id:'source'}],buses:Array.from({length:7},(_,i)=>({id:'b'+i,source:0})),branches:[[0,1,'transformer'],[1,2,'line'],[1,3,'switch'],[2,3,'switch'],[3,4,'transformer'],[4,4,'switch'],[0,5,'switch']].map(([from,to,family],i)=>({id:'e'+i,source:0,from,to,family,disabled:i===3||i===6,...(family==='switch'?{openSwitch:i===3,outOfService:i===6}:{} )})),attachments:Array.from({length:20},(_,i)=>({id:'a'+i,family:['load','shunt','voltage_source'][i%3],bus:i%5,source:0,disabled:false}))};}
function fixture(m,routing='equipment'){const result=makeSchematic(m,{strategy:'tidy-tree',routing}),f=decodeFixture(result.manifest,result.buffer);f.refs=m;validateSchematic(f);return f;}
test('equipment spacing preserves identities, forest, state and immutable inputs',()=>{
 const m=model(),snapshot=JSON.stringify(m),f=fixture(m),base=fixture(m,'crossing-aware');
 for(const key of ['endpoints','states','parent','parentBranch','depth','component','branchRoles'])assert.deepEqual(f[key],base[key]);
 assert.equal(JSON.stringify(m),snapshot);assert.equal(f.manifest.equipment.glyphs,26);
 assert.equal(equipmentClearanceMetrics(f).foreignRouteBoxIntersections,0);
 assert.deepEqual(f.buffer,fixture(m).buffer);
 const g=fixture(m);g.segmentSources[3]+=1;assert.throws(()=>validateSchematic(g));
 const h=fixture(m);h.equipmentGlyphOwners[0]=999999;assert.throws(()=>validateSchematic(h),/owner/);
});
test('compact open presentation preserves pick owners and keeps closed out-of-service switches',()=>{
 const f=fixture(model());f.openTies=buildOpenTiePresentation(f);const eq=prepareEquipment(f);
 assert.ok(eq.full.glyphOwners.includes(3));assert.ok(!eq.compact.glyphOwners.includes(3));assert.ok(eq.compact.glyphOwners.includes(6));
 for(const owner of eq.full.glyphOwners){const {row,kind,index}=equipmentRow(f,owner);assert.equal(row,(kind==='branch'?f.refs.branches:f.refs.attachments)[index]);}
});
test('dense attachments, parallel circuits and loops keep foreign routes outside glyph reservations',()=>{
 const m=model();for(let i=0;i<90;i++)m.branches.push({id:'p'+i,source:0,from:i%4,to:(i+1)%5,family:i%2?'switch':'transformer',openSwitch:false,disabled:false});
 for(let i=0;i<150;i++)m.attachments.push({id:'dense'+i,source:0,bus:1,family:'load'});
 assert.equal(equipmentClearanceMetrics(fixture(m)).foreignRouteBoxIntersections,0);
});
test('clearance audit detects a foreign route passing through an equipment box',()=>{
 const f=fixture(model()),owner=f.equipmentGlyphOwners[0],x=f.equipmentCenters[0],y=f.equipmentCenters[1];
 f.segmentSources.set([x-30,y,0],0);f.segmentTargets.set([x+30,y,0],0);f.segmentBranches[0]=owner===1?2:1;
 assert.ok(equipmentClearanceMetrics(f).foreignRouteBoxIntersections>0);
});

test('one tiny row gap does not magnify the entire component',()=>{
 const m={sources:[{id:'s'}],buses:[0,1,2].map(i=>({id:'b'+i,source:0})),branches:[],attachments:[{id:'load',family:'load',bus:0,source:0}]};
 const positions=new Float32Array([0,0,0,0,.01,0,0,10000,0]);
 const routed=corridorRouting({model:m,positions,depth:new Uint32Array(3),component:new Uint32Array(3),components:[{root:0,source:0,maxDepth:0,bounds:[0,0,0,10000]}],roles:new Uint8Array(),equipment:true});
 assert.equal(routed.metadata.verticalSpacing,'monotone-local-expansion');
 assert.ok(positions[7]-positions[1]<10200);
 assert.ok(positions[4]-positions[1]>=86);
});
