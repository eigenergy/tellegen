/** Independent result projection for the diagram POC. No solver or layout dependency. */
import {sourceKey} from './bmopf-adapter.mjs';
export const resultKey=(refs,row,family)=>sourceKey(refs.sources[row.source].id,family??row.family,row.id,row.kind??'');
const magnitude=z=>z&&Number.isFinite(z.re)&&Number.isFinite(z.im)?Math.hypot(z.re,z.im):null;
const difference=(a,b)=>a&&b?magnitude({re:a.re-b.re,im:a.im-b.im}):null;
export function severity(value,min,max){return !Number.isFinite(value)?'missing':value<min||value>max?'violation':'normal';}
export function formatValue(value){return Number.isFinite(value)?Number(value.toPrecision(7)).toString():'unavailable';}
function indexed(rows,key){const map=new Map();for(const row of rows){const k=key(row);if(map.has(k))throw new Error(`Duplicate result identity: ${k}`);map.set(k,row);}return map;}
/** Deliberately narrow French fixture projection: explicit neutral or uniform three-phase pair bounds. */
export function projectFrenchResults(raw,solved,refs,provenance){
 if(!solved.converged)throw new Error('Unconverged result');
 const volts=indexed(solved.terminals,t=>JSON.stringify([t.bus,t.terminal]));
 const ports=new Map();for(const p of solved.element_ports){const k=JSON.stringify([p.kind,p.element,p.bus]);if(!ports.has(k))ports.set(k,[]);ports.get(k).push(p);}
 const buses=refs.buses.map(b=>{
  const row=raw.bus[b.id], phases=b.terminals.filter(t=>t!=='n'), pairs=[];
  const voltage=t=>volts.get(JSON.stringify([b.id,t]))?.voltage;
  if(row.vpn_min&&row.vpn_max&&b.terminals.includes('n')){
   phases.forEach((t,i)=>pairs.push({pair:[t,'n'],volts:difference(voltage(t),voltage('n')),min:row.vpn_min[i],max:row.vpn_max[i]}));
  }else if(row.vpp_min&&row.vpp_max&&phases.length===3){
   if(new Set(row.vpp_min).size!==1||new Set(row.vpp_max).size!==1)throw new Error('Nonuniform phase-pair limit order requires an explicit mapping');
   for(const [a,c] of [[0,1],[1,2],[2,0]])pairs.push({pair:[phases[a],phases[c]],volts:difference(voltage(phases[a]),voltage(phases[c])),min:row.vpp_min[0],max:row.vpp_max[0]});
  }
  if(pairs.some(p=>!Number.isFinite(p.volts)||!Number.isFinite(p.min)||!Number.isFinite(p.max)||p.min>=p.max))throw new Error('Missing voltage or invalid pair limits');
  return {key:resultKey(refs,b,'bus'),pairs};
 });
 const branches=refs.branches.map(b=>{
  const row=b.family==='transformer'?raw.transformer[b.kind][b.id]:raw[b.family]?.[b.id];
  const ends=[b.from,b.to].map((bus,i)=>{
   const rows=ports.get(JSON.stringify([b.family,b.id,refs.buses[bus].id]))??[],map=i?b.terminalMapTo:b.terminalMapFrom;
   if(!rows.length||b.disabled)return null;
   if(rows.some(p=>!Number.isFinite(p.power_into_element?.re)||!Number.isFinite(p.power_into_element?.im)||magnitude(p.current_into_element)===null))throw new Error('Nonfinite port result');
   if(rows.length!==map.length||map.some(t=>!rows.some(p=>p.terminal===t)))throw new Error('Incomplete or unknown element terminal');
   if(new Set(rows.map(p=>p.terminal)).size!==rows.length)throw new Error('Ambiguous repeated element port');
   const currents=map.map(t=>{const p=rows.find(p=>p.terminal===t);return {terminal:t,amps:magnitude(p?.current_into_element)};});
   return {p:rows.reduce((s,p)=>s+p.power_into_element.re,0)/1000,q:rows.reduce((s,p)=>s+p.power_into_element.im,0)/1000,currents};
  });
  let loading=null,basis=null;
  if(ends.every(Boolean)&&b.family==='line'&&Array.isArray(row.i_max)&&row.i_max.length===b.terminalMapFrom.length&&row.i_max.every(x=>Number.isFinite(x)&&x>0)&&ends.every(e=>e.currents.every(c=>c.amps!==null))){
   loading=100*Math.max(...ends.flatMap(e=>e.currents.map((c,i)=>c.amps/row.i_max[i])));basis='Maximum conductor |I| / i_max across both ends';
  }else if(ends.every(Boolean)&&b.family==='transformer'&&Number.isFinite(row.s_rating)&&row.s_rating>0){
   loading=100*Math.max(...ends.map(e=>Math.hypot(e.p,e.q)))*1000/row.s_rating;basis='Maximum end |sum S| / s_rating; aggregate MVA, not worst winding/coil';
  }
  return {key:resultKey(refs,b),ends,loading,basis};
 });
 return {version:1,kind:'solved',label:'Saved Tellegen multiconductor power flow',sources:refs.sources.map(s=>({id:s.id,sha256:s.sha256})),provenance,buses,branches};
}
/** Intentionally nonphysical. Only for rendering throughput on the real SAPN topology. */
export function stressResults(refs){
 return {version:1,kind:'stress',label:'ARTIFICIAL rendering stress — not a SAPN power-flow solution',sources:refs.sources.map(s=>({id:s.id,sha256:s.sha256})),provenance:{generator:'deterministic index pattern v1; no electrical calculation'},
 buses:refs.buses.map((b,i)=>({key:resultKey(refs,b,'bus'),pairs:[{pair:['stress','reference'],volts:225+(i%21),min:230,max:240}]})),
 branches:refs.branches.map((b,i)=>({key:resultKey(refs,b),ends:b.disabled?[null,null]:[{p:(i%2?-1:1)*(20+i%90),q:i%3?-12:8},{p:(i%2?1:-1)*(19+i%90),q:i%3?11:-9}],loading:b.disabled?null:20+i%101,basis:'Artificial percentage; no equipment rating used'}))};
}
export function bindResults(sidecar,refs){
 if(sidecar.version!==1||!['solved','stress'].includes(sidecar.kind))throw new Error('Unsupported result sidecar');
 if(JSON.stringify(sidecar.sources)!==JSON.stringify(refs.sources.map(s=>({id:s.id,sha256:s.sha256}))))throw new Error('Stale results: source input identity/hash mismatch');
 const busMap=indexed(sidecar.buses,b=>b.key),branchMap=indexed(sidecar.branches,b=>b.key);
 if(busMap.size!==refs.buses.length||branchMap.size!==refs.branches.length)throw new Error('Result coverage mismatch');
 const buses=refs.buses.map(b=>busMap.get(resultKey(refs,b,'bus'))),branches=refs.branches.map(b=>branchMap.get(resultKey(refs,b)));
 if(buses.some(b=>!b)||branches.some(b=>!b))throw new Error('Unknown result source identity');
 for(const b of buses)if(!Array.isArray(b.pairs)||b.pairs.some(p=>!Array.isArray(p.pair)||p.pair.length!==2||![p.volts,p.min,p.max].every(Number.isFinite)||p.min>=p.max))throw new Error('Invalid voltage result');
 for(const [i,b] of branches.entries())if(!Array.isArray(b.ends)||b.ends.length!==2||b.ends.some(e=>e!==null&&(!Number.isFinite(e.p)||!Number.isFinite(e.q)))||b.loading!==null&&(!Number.isFinite(b.loading)||b.loading<0)||refs.branches[i].disabled&&b.ends.some(Boolean))throw new Error('Invalid branch result');
 return {...sidecar,buses,branches};
}
