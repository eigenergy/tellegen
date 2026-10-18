/** Tabular result model: columns, filtering, sorting and CSV over source identities. Pure; no DOM. */
import {worstBand} from './result-colours.mjs';
import {voltageState,loadingState} from './results-overlay.mjs';
import {magnitudeUnbalance,pairZone} from './workbench-summary.mjs';
const num=(digits=2)=>v=>Number.isFinite(v)?v.toLocaleString(undefined,{minimumFractionDigits:digits,maximumFractionDigits:digits}):'—';
const text=v=>v??'—';
/** Up to three pair columns, labelled by position; each row names its own pairs in the Pairs column. */
export function busColumns(){
 return [{key:'id',label:'Bus',format:text},{key:'source',label:'Source',format:text},{key:'ref',label:'Reference',format:text},{key:'pairs',label:'Pairs',format:text},
  ...[0,1,2].map(k=>({key:'v'+k,label:`V${k+1}`,unit:'V',numeric:true,format:num(2)})),
  {key:'band',label:'Band position',unit:'±1 = limit',numeric:true,format:num(3),title:'Worst pair: −1 at the declared minimum, +1 at the maximum. Sorting orders by distance from mid-band.'},
  {key:'unbalance',label:'Unbalance',unit:'%',numeric:true,format:num(2),title:'Magnitude-only: IEEE PVUR (phase–neutral) or NEMA LVUR (phase–phase)'},
  {key:'state',label:'Status',format:text}];
}
export function branchColumns(){
 return [{key:'id',label:'Branch',format:text},{key:'family',label:'Type',format:text},{key:'from',label:'From bus',format:text},{key:'to',label:'To bus',format:text},
  {key:'pFrom',label:'P from',unit:'kW',numeric:true,format:num(2),title:'Positive into the equipment at the from end'},{key:'qFrom',label:'Q from',unit:'kvar',numeric:true,format:num(2)},
  {key:'pTo',label:'P to',unit:'kW',numeric:true,format:num(2),title:'Positive into the equipment at the to end'},{key:'qTo',label:'Q to',unit:'kvar',numeric:true,format:num(2)},
  {key:'loss',label:'Losses',unit:'kW',numeric:true,format:num(3),title:'Sum of both into-equipment end P'},
  {key:'loading',label:'Loading',unit:'%',numeric:true,format:num(1)},{key:'state',label:'Status',format:text}];
}
export function busRow(f,r,i){
 const row=f.refs.buses[i],pairs=r.buses[i].pairs,band=worstBand(pairs);
 return {id:row.id,source:f.refs.sources[row.source].label,ref:pairs.length?[...new Set(pairs.map(p=>pairZone(p.pair)))].join(', '):null,pairs:pairs.map(p=>p.pair.join('–')).join(' ')||null,
  v0:pairs[0]?.volts??null,v1:pairs[1]?.volts??null,v2:pairs[2]?.volts??null,band,unbalance:magnitudeUnbalance(pairs),state:voltageState(r.buses[i])};
}
export function branchRow(f,r,b){
 const row=f.refs.branches[b],res=r.branches[b],[a,z]=res.ends;
 return {id:row.id,family:row.family+(row.kind?' / '+row.kind:''),from:f.refs.buses[row.from].id,to:f.refs.buses[row.to].id,
  pFrom:a?.p??null,qFrom:a?.q??null,pTo:z?.p??null,qTo:z?.q??null,loss:a&&z?a.p+z.p:null,loading:res.loading,
  state:row.disabled||f.states?.[b]?'inactive':loadingState(res)};
}
export function tableModel(f,r,kind){
 const count=kind==='bus'?f.refs.buses.length:f.refs.branches.length,make=kind==='bus'?busRow:branchRow,cache=new Map();
 return {kind,count,columns:kind==='bus'?busColumns():branchColumns(),
  row(i){if(!cache.has(i)){if(cache.size>4096)cache.clear();cache.set(i,make(f,r,i));}return cache.get(i);}};
}
const ISSUE={bus:new Set(['violation']),branch:new Set(['violation','warning'])};
/** Filter by case-insensitive text over ID/source/endpoints and optionally issues only; then a stable sort. */
export function query(model,{text='',issuesOnly=false,sortKey=null,descending=false}={}){
 const q=text.trim().toLowerCase(),out=[];
 for(let i=0;i<model.count;i++){
  const row=model.row(i);
  if(issuesOnly&&!ISSUE[model.kind].has(row.state))continue;
  if(q&&![row.id,row.source,row.from,row.to,row.family].some(v=>v&&String(v).toLowerCase().includes(q)))continue;
  out.push(i);
 }
 const col=model.columns.find(c=>c.key===sortKey);
 if(col){
  const values=new Map(out.map(i=>[i,model.row(i)[sortKey]])),sign=descending?-1:1;
  // Missing values sort last in both directions.
  out.sort((a,b)=>{const x=values.get(a),y=values.get(b),mx=x===null||x===undefined||Number.isNaN(x),my=y===null||y===undefined||Number.isNaN(y);
   if(mx||my)return mx===my?a-b:mx?1:-1;
   return (col.numeric?(sortKey==='band'?Math.abs(x)-Math.abs(y):x-y):String(x).localeCompare(String(y)))*sign||a-b;});
 }
 return Int32Array.from(out);
}
/** Text that a spreadsheet would evaluate as a formula gets a leading apostrophe; numbers stay numeric. */
const csvCell=v=>{if(v===null||v===undefined)return '';let s=String(v);if(typeof v!=='number'&&/^[=+\-@\t\r]/.test(s))s="'"+s;return /[",\n\r']/.test(s)?`"${s.replace(/"/g,'""')}"`:s;};
/** Full-precision CSV of the given rows; units in headers. */
export function toCsv(model,indices,meta=[]){
 const head=model.columns.map(c=>c.unit?`${c.label} (${c.unit})`:c.label);
 const lines=[...meta.map(m=>`# ${m}`),head.map(csvCell).join(',')];
 for(const i of indices){const row=model.row(i);lines.push(model.columns.map(c=>csvCell(row[c.key])).join(','));}
 return lines.join('\r\n')+'\r\n';
}
