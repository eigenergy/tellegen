import test from 'node:test';import assert from 'node:assert/strict';
import {tableModel,query,toCsv} from './results-table.mjs';
const pn=(t,v)=>({pair:[t,'n'],volts:v,min:207,max:253});
function data(){const f={states:Uint8Array.of(0,0,1),refs:{sources:[{id:'s',label:'Feeder'}],
 buses:[{id:'B1',source:0},{id:'B2',source:0},{id:'=cmd',source:0}],
 branches:[{id:'L1',source:0,family:'line',from:0,to:1},{id:'T1',source:0,family:'transformer',kind:'2w',from:1,to:2},{id:'S1',source:0,family:'switch',from:0,to:2}]}};
 const r={buses:[{pairs:[pn('1',230),pn('2',231),pn('3',229)]},{pairs:[pn('1',205),pn('2',230),pn('3',240)]},{pairs:[]}],
  branches:[{ends:[{p:100,q:10},{p:-98,q:-9}],loading:95},{ends:[{p:-50,q:5},{p:50.5,q:-4}],loading:40},{ends:[null,null],loading:null}]};return {f,r};}
test('bus rows carry pair voltages, worst band, unbalance and status by source identity',()=>{const {f,r}=data(),m=tableModel(f,r,'bus'),row=m.row(1);
 assert.equal(row.id,'B2');assert.equal(row.pairs,'1–n 2–n 3–n');assert.equal(row.ref,'phase–neutral');assert.deepEqual([row.v0,row.v1,row.v2],[205,230,240]);assert.equal(row.state,'violation');assert.ok(row.band<-1);assert.ok(row.unbalance>0);
 assert.equal(m.row(2).state,'missing');assert.equal(m.row(2).band,null);});
test('branch rows keep signed end powers, losses and inactive state',()=>{const {f,r}=data(),m=tableModel(f,r,'branch');
 assert.deepEqual([m.row(0).pFrom,m.row(0).pTo,m.row(0).loss,m.row(0).state],[100,-98,2,'warning']);assert.equal(m.row(1).family,'transformer / 2w');assert.equal(m.row(2).state,'inactive');assert.equal(m.row(2).loss,null);});
test('query filters by text and issues, sorts numerically with missing values last both ways',()=>{const {f,r}=data(),b=tableModel(f,r,'branch'),bus=tableModel(f,r,'bus');
 assert.deepEqual([...query(b,{text:'t1'})],[1]);assert.deepEqual([...query(b,{issuesOnly:true})],[0]);assert.deepEqual([...query(bus,{issuesOnly:true})],[1]);
 assert.deepEqual([...query(b,{sortKey:'loading'})],[1,0,2]);assert.deepEqual([...query(b,{sortKey:'loading',descending:true})],[0,1,2]);
 assert.deepEqual([...query(b,{sortKey:'id'})],[0,2,1]);assert.deepEqual([...query(bus,{sortKey:'band',descending:true})],[1,0,2]);});
test('CSV keeps full precision and units, quotes separators and neutralises formula-like text',()=>{const {f,r}=data(),m=tableModel(f,r,'bus');r.buses[0].pairs[0].volts=230.123456789;
 const csv=toCsv(m,[0,2],['case: test']).split('\r\n');assert.equal(csv[0],'# case: test');assert.match(csv[1],/^Bus,Source,Reference,Pairs,V1 \(V\),V2 \(V\),V3 \(V\),Band position \(±1 = limit\),Unbalance \(%\),Status$/);
 assert.match(csv[2],/^B1,Feeder,phase–neutral,1–n 2–n 3–n,230.123456789,/);assert.match(csv[3],/^"'=cmd",Feeder,,,,,,,,missing$/);
 const neg=toCsv(tableModel(f,r,'branch'),[0]).split('\r\n')[1];assert.match(neg,/,-98,/);});
