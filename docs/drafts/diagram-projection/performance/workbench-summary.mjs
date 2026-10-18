/** Snapshot KPIs and a severity-ranked issue list. Pure; no rendering or DOM dependency. */
import {severity} from './results-adapter.mjs';
export const LOADING_WARNING=90,LOADING_LIMIT=100;
/** Zone by declared reference, never by an inferred nominal voltage. */
export function pairZone(pair){return pair[1]==='n'?'phase–neutral':pair[0]==='n'?'neutral':'phase–phase';}
/** Signed distance outside [min, max] as a percentage of the violated limit; ≤0 inside the band. */
export function exceedance(p){return p.volts<p.min?100*(p.min-p.volts)/p.min:p.volts>p.max?100*(p.volts-p.max)/p.max:-100*Math.min(p.volts-p.min,p.max-p.volts)/(p.max-p.min);}
/** Magnitude-only unbalance: max deviation from the mean / mean (IEEE PVUR for phase–neutral, NEMA LVUR for phase–phase). */
export function magnitudeUnbalance(pairs){
 if(pairs.length!==3||new Set(pairs.map(p=>pairZone(p.pair))).size!==1)return null;
 const mean=(pairs[0].volts+pairs[1].volts+pairs[2].volts)/3;
 return mean>0?100*Math.max(...pairs.map(p=>Math.abs(p.volts-mean)))/mean:null;
}
export function rankIssues(result){
 const issues=[];
 result.buses.forEach((b,index)=>{
  let worst=null;for(const p of b.pairs)if(severity(p.volts,p.min,p.max)==='violation'&&(!worst||exceedance(p)>exceedance(worst)))worst=p;
  if(worst)issues.push({kind:'voltage',severity:'violation',target:{kind:'bus',index},pair:worst.pair,value:worst.volts,limit:worst.volts<worst.min?worst.min:worst.max,low:worst.volts<worst.min,score:exceedance(worst)});
 });
 result.branches.forEach((b,index)=>{
  if(!Number.isFinite(b.loading)||b.loading<LOADING_WARNING)return;
  issues.push({kind:'loading',severity:b.loading>LOADING_LIMIT?'violation':'warning',target:{kind:'branch',index},value:b.loading,limit:LOADING_LIMIT,score:b.loading-LOADING_LIMIT});
 });
 // Violations first, then the largest relative exceedance; index keeps ties deterministic.
 return issues.sort((a,b)=>(a.severity===b.severity?0:a.severity==='violation'?-1:1)||b.score-a.score||a.target.index-b.target.index);
}
export function snapshotSummary(result,issues=rankIssues(result)){
 const zones=new Map(),loss={p:0,q:0,branches:0};let loading=null,unbalance=null;
 result.buses.forEach((b,bus)=>{
  for(const p of b.pairs){
   const zone=pairZone(p.pair);if(!zones.has(zone))zones.set(zone,{zone,min:null,max:null,buses:new Set()});
   const z=zones.get(zone);z.buses.add(bus);
   if(!z.min||p.volts<z.min.volts)z.min={bus,pair:p.pair,volts:p.volts,limit:p.min,severity:severity(p.volts,p.min,p.max)};
   if(!z.max||p.volts>z.max.volts)z.max={bus,pair:p.pair,volts:p.volts,limit:p.max,severity:severity(p.volts,p.min,p.max)};
  }
  const u=magnitudeUnbalance(b.pairs);if(u!==null&&(!unbalance||u>unbalance.percent))unbalance={bus,percent:u,zone:pairZone(b.pairs[0].pair),definition:pairZone(b.pairs[0].pair)==='phase–phase'?'NEMA LVUR':'IEEE PVUR'};
 });
 result.branches.forEach((b,branch)=>{
  // Both ends are signed into the equipment, so their sum is what the branch consumes.
  if(b.ends[0]&&b.ends[1]){loss.p+=b.ends[0].p+b.ends[1].p;loss.q+=b.ends[0].q+b.ends[1].q;loss.branches++;}
  if(Number.isFinite(b.loading)&&(!loading||b.loading>loading.value))loading={branch,value:b.loading};
 });
 const count=(kind,level)=>issues.filter(i=>i.kind===kind&&i.severity===level).length;
 return {kind:result.kind,loss,loading,unbalance,
  zones:[...zones.values()].map(z=>({zone:z.zone,min:z.min,max:z.max,buses:z.buses.size})).sort((a,b)=>b.max.volts-a.max.volts),
  counts:{voltage:count('voltage','violation'),overload:count('loading','violation'),warning:count('loading','warning')}};
}
