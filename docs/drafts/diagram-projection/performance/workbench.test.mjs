import test from 'node:test';import assert from 'node:assert/strict';
import {rankIssues,snapshotSummary,magnitudeUnbalance,exceedance,pairZone} from './workbench-summary.mjs';
import {buildSearchIndex,search,entityKey,syntheticBus} from './search-index.mjs';
import {encodeCamera,decodeCamera,withView} from './view-state.mjs';
const pn=(t,volts,min=207,max=253)=>({pair:[t,'n'],volts,min,max}),pp=(a,b,volts)=>({pair:[a,b],volts,min:19000,max:21000});
function result(){return {kind:'solved',
 buses:[{pairs:[pp('1','2',20400),pp('2','3',20500),pp('3','1',20450)]},{pairs:[pn('1',230),pn('2',232),pn('3',205)]},{pairs:[pn('1',255),pn('2',240),pn('3',240)]},{pairs:[]}],
 branches:[{ends:[{p:100,q:10},{p:-98,q:-9}],loading:95},{ends:[{p:50,q:5},{p:-49.5,q:-4.8}],loading:120},{ends:[null,null],loading:null},{ends:[{p:10,q:1},{p:-10,q:-1}],loading:101}]};}
const refs={sources:[{id:'s',label:'Feeder'},{id:'t',label:'Other'}],
 buses:[{id:'MV1',source:0},{id:'LV1',source:0},{id:'LV10',source:0},{id:'LV1',source:1}],
 branches:[{id:'L1',source:0,family:'line'},{id:'T1',source:0,family:'transformer',kind:'2w'},{id:'LV1',source:0,family:'switch'}],
 attachments:[{id:'load-LV1',source:0,family:'load',bus:1}]};
test('pair zones follow declared references, not inferred nominal voltage',()=>{assert.equal(pairZone(['1','n']),'phase–neutral');assert.equal(pairZone(['1','2']),'phase–phase');assert.equal(pairZone(['stress','reference']),'phase–phase');});
test('exceedance is relative to the violated limit and negative inside the band',()=>{assert.ok(Math.abs(exceedance(pn('1',205))-100*2/207)<1e-12);assert.ok(exceedance(pn('1',255))>0);assert.ok(exceedance(pn('1',230))<0);});
test('issues rank violations before warnings, then by exceedance, with deterministic ties',()=>{const issues=rankIssues(result());
 assert.deepEqual(issues.map(i=>[i.kind,i.severity,i.target.index]),[['loading','violation',1],['loading','violation',3],['voltage','violation',1],['voltage','violation',2],['loading','warning',0]]);
 const low=issues.find(i=>i.target.index===1&&i.kind==='voltage');assert.equal(low.low,true);assert.equal(low.limit,207);assert.deepEqual(low.pair,['3','n']);});
test('summary losses sum both into-equipment ends and skip inactive branches',()=>{const s=snapshotSummary(result());assert.ok(Math.abs(s.loss.p-2.5)<1e-9);assert.ok(Math.abs(s.loss.q-1.2)<1e-9);assert.equal(s.loss.branches,3);assert.deepEqual(s.loading,{branch:1,value:120});assert.deepEqual(s.counts,{voltage:2,overload:2,warning:1});});
test('summary voltage extremes are per declared zone and keep the bus and pair',()=>{const s=snapshotSummary(result()),lv=s.zones.find(z=>z.zone==='phase–neutral'),mv=s.zones.find(z=>z.zone==='phase–phase');
 assert.deepEqual([lv.min.bus,lv.min.volts,lv.min.severity,lv.max.bus,lv.max.volts,lv.buses],[1,205,'violation',2,255,2]);assert.deepEqual([mv.min.volts,mv.max.volts],[20400,20500]);});
test('magnitude unbalance needs three pairs of one zone and names its definition',()=>{assert.equal(magnitudeUnbalance([pn('1',230),pn('2',230)]),null);assert.equal(magnitudeUnbalance([pn('1',230),pn('2',230),pp('1','2',400)]),null);
 const s=snapshotSummary(result());assert.equal(s.unbalance.bus,1);assert.equal(s.unbalance.definition,'IEEE PVUR');assert.ok(Math.abs(s.unbalance.percent-100*(222.333333-205)/222.333333)<1e-4);});
test('search ranks exact, qualified, prefix and substring matches; buses before branches on ties',()=>{const index=buildSearchIndex(refs);
 const lv1=search(index,'lv1');assert.deepEqual(lv1.matches.slice(0,3).map(m=>[m.kind,m.qualified]),[['bus','s/LV1'],['bus','t/LV1'],['branch','s/LV1']]);assert.ok(lv1.matches[0].exact);assert.equal(lv1.total,5);
 assert.equal(search(index,'t/LV1').matches[0].qualified,'t/LV1');assert.equal(search(index,'t/lv1').matches[0].exact,true);
 assert.deepEqual(search(index,'load').matches.map(m=>m.kind),['attachment']);assert.equal(search(index,'  ').total,0);assert.equal(search(index,'LV',{limit:2}).matches.length,2);assert.deepEqual(search(index,'1',{kinds:['branch']}).matches.map(m=>m.id),['L1','T1','LV1']);});
test('entity keys disambiguate repeated raw IDs across sources and families',()=>{const index=buildSearchIndex(refs),keys=[entityKey(refs,'bus',1),entityKey(refs,'bus',3),entityKey(refs,'branch',2),entityKey(refs,'branch',1)];
 assert.equal(new Set(keys).size,4);assert.deepEqual(keys,['bus:s/LV1','bus:t/LV1','branch:switch:s/LV1','branch:transformer.2w:s/T1']);for(const k of keys)assert.ok(index.byKey.has(k));});
test('synthetic bus names accept B-prefixed or bare numbers within range',()=>{assert.equal(syntheticBus('B000501',1000),500);assert.equal(syntheticBus('7',1000),6);assert.equal(syntheticBus('B0',1000),null);assert.equal(syntheticBus('B1001',1000),null);assert.equal(syntheticBus('bus',1000),null);});
test('camera round-trips and rejects malformed or out-of-range state',()=>{const c=decodeCamera(encodeCamera({target:[12.345,-6.78,0],zoom:1.234}));assert.deepEqual(c,{target:[12.3,-6.8,0],zoom:1.23});for(const bad of ['','1,2','1,2,x','1,2,9',null])assert.equal(decodeCamera(bad),null);});
test('view URLs merge state and remove cleared keys without touching other parameters',()=>{const url=withView('http://x/p/?dataset=a&keep=1&sel=bus:s/1',{sel:null,view:'1,2,0',results:'loading'});assert.equal(url.search,'?dataset=a&keep=1&view=1%2C2%2C0&results=loading');});
