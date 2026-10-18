import test from 'node:test';import assert from 'node:assert/strict';
import {placeLabels,busCandidate,branchCandidate} from './value-labels.mjs';
const c=(id,x,y,state='normal',priority=0,selected=false)=>({id,screen:[x,y],state,priority,selected});
test('placement prefers selection, then severity, then magnitude, and rejects overlaps',()=>{
 const out=placeLabels([c('low',100,100,'normal',1),c('high',105,100,'normal',9),c('bad',300,100,'violation',0),c('sel',102,101,'normal',0,true)],{width:800,height:600});
 assert.deepEqual(out.map(o=>o.id),['sel','bad']);});
test('placement respects the budget and keeps unselected labels inside the canvas margins',()=>{
 const many=Array.from({length:100},(_,i)=>c('n'+i,30+(i%10)*70,60+Math.floor(i/10)*20,'normal',i));
 assert.equal(placeLabels(many,{width:2000,height:2000,budget:7}).length,7);
 assert.equal(placeLabels([c('edge',790,300)],{width:800,height:600}).length,0);assert.equal(placeLabels([c('edge',790,300,'normal',0,true)],{width:800,height:600}).length,1);});
test('bus values use the pair nearest a limit; branch values follow the view family',()=>{
 const r={buses:[{pairs:[{volts:230,min:200,max:260},{volts:205,min:207,max:253}]},{pairs:[]}],branches:[{ends:[{p:-84.71},{p:81.4}],loading:120.04},{ends:[null,null],loading:null}]};
 assert.deepEqual(busCandidate(r,0).text,'205 V');assert.equal(busCandidate(r,0).state,'violation');assert.equal(busCandidate(r,1),null);
 assert.deepEqual(branchCandidate(r,0,'loading'),{text:'120.0 %',state:'violation',priority:120.04});assert.equal(branchCandidate(r,0,'power').text,'84.71 kW');assert.equal(branchCandidate(r,1,'power'),null);assert.equal(branchCandidate(r,1,'loading'),null);});
