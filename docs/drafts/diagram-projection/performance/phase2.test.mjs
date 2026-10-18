import test from 'node:test';import assert from 'node:assert/strict';
import {pairBars,conductorBars,inspectorMarkup} from './multiconductor-view.mjs';
import {feederEnvelope,zoneEnvelope} from './feeder-envelope.mjs';
import {ease,screenAnchored,lerpInto,chordsInto} from './layout-morph.mjs';
import {profileMarkup} from './voltage-profile-view.mjs';
const pn=(t,v)=>({pair:[t,'n'],volts:v,min:200,max:260});
test('pair bars place the declared band and value on a band ±25 % track and flag violations',()=>{
 const html=pairBars([pn('1',230),pn('2',190),pn('3',245)]);
 assert.match(html,/left:16\.7%;width:66\.7%/);assert.match(html,/mc-mark" style="left:50\.0%/);assert.match(html,/class="mc-row violation"/);assert.match(html,/190\.00 V !/);assert.match(html,/IEEE PVUR/);
 assert.match(pairBars([{pair:['<x>','n'],volts:1e9,min:200,max:260}]),/left:100\.0%/);assert.match(pairBars([{pair:['<x>','n'],volts:1,min:200,max:260}]),/&lt;x&gt;/);assert.match(pairBars([]),/No declared/);});
test('conductor bars share one scale across both ends and report neutral current share',()=>{
 const end=(a,b,c,nA)=>({p:10,q:2,currents:[{terminal:'1',amps:a},{terminal:'2',amps:b},{terminal:'3',amps:c},{terminal:'n',amps:nA}]});
 const html=conductorBars([end(100,50,80,40),end(99,50,80,40)]);
 assert.match(html,/width:100\.0%/);assert.match(html,/width:50\.0%/);assert.match(html,/class="mc-row neutral"/);assert.match(html,/Neutral carries 40 %/);assert.match(html,/From end/);assert.match(html,/To end/);
 assert.equal(conductorBars([null,null]),'');assert.equal(conductorBars([{p:1,q:1}]),'');});
test('inspector markup flags stress data and routes branches to conductors, buses to pairs',()=>{
 const r={kind:'stress',buses:[{pairs:[pn('1',230)]}],branches:[{ends:[null,null]}]};
 assert.match(inspectorMarkup(r,'bus',0,0),/NOT A SOLUTION/);assert.match(inspectorMarkup(r,'attachment',0,0),/mc-track/);assert.ok(!inspectorMarkup(r,'branch',0,0).includes('mc-track'));assert.equal(inspectorMarkup(null,'bus',0,0),'');});
test('envelope groups by component, depth and pair reference, never mixing MV and LV',()=>{
 const f={depth:Uint32Array.of(0,1,1,1,1),component:Uint32Array.of(0,0,0,0,1)},pp=v=>({pair:['1','2'],volts:v,min:1,max:2e4});
 const r={buses:[{pairs:[pp(20000)]},{pairs:[pn('1',230)]},{pairs:[pn('1',220)]},{pairs:[pp(19900)]},{pairs:[pn('1',100)]}]};
 const env=feederEnvelope(f,r);assert.deepEqual(env.get('0|1|phase–neutral'),{min:220,max:230,count:2});assert.deepEqual(env.get('0|1|phase–phase'),{min:19900,max:19900,count:1});assert.deepEqual(env.get('1|1|phase–neutral'),{min:100,max:100,count:1});
 assert.deepEqual(zoneEnvelope(env,f,{rows:[{bus:1,pairs:r.buses[1].pairs},{bus:0,pairs:[]}]}),[{min:220,max:230,count:2},null]);assert.equal(feederEnvelope({},r),null);});
test('profile charts draw an envelope band behind the path and include it in the y range',()=>{
 const refs={buses:[{id:'a'},{id:'b'}]},profile={notice:'',zones:[{reason:'Root-side voltage zone',rows:[{bus:0,hop:0,pairs:[pn('1',230)]},{bus:1,hop:1,pairs:[pn('1',228)]}]}]};
 const svg=profileMarkup(profile,refs,[[{min:180,max:235},{min:181,max:229}]]);assert.match(svg,/class="envelope"/);assert.ok(Number(svg.match(/text-anchor="end">([\d.]+)</)[1])<=180);assert.ok(!profileMarkup(profile,refs).includes('envelope'));});
test('morph anchors start positions to the current screen, interpolates, and draws chords',()=>{
 const a=Float32Array.of(10,20,0,30,40,0),start=screenAnchored(a,{target:[10,20],zoom:1},{target:[100,200],zoom:0});
 assert.deepEqual([...start],[100,200,0,140,240,0]);const out=new Float32Array(6);lerpInto(out,start,Float32Array.of(0,0,0,0,0,0),.5);assert.deepEqual([...out],[50,100,0,70,120,0]);
 const s=new Float32Array(3),t=new Float32Array(3);chordsInto(s,t,Uint32Array.of(1,0),out);assert.deepEqual([...s,...t],[70,120,0,50,100,0]);
 assert.equal(ease(0),0);assert.equal(ease(1),1);assert.equal(ease(.5),.5);});
