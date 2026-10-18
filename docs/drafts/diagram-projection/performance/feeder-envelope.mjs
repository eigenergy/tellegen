/** Voltage envelope by display-tree depth: min/max over every bus at the same depth, component and pair reference. */
import {pairZone} from './workbench-summary.mjs';
const key=(component,depth,zone)=>`${component}|${depth}|${zone}`;
/** One pass over all buses; cached per result snapshot and layout. Only meaningful with a display tree. */
export function feederEnvelope(f,r){
 if(!f.depth||!f.component)return null;
 const env=new Map();
 for(let i=0;i<r.buses.length;i++)for(const p of r.buses[i].pairs){
  const k=key(f.component[i],f.depth[i],pairZone(p.pair)),e=env.get(k);
  if(!e)env.set(k,{min:p.volts,max:p.volts,count:1});else{e.min=Math.min(e.min,p.volts);e.max=Math.max(e.max,p.volts);e.count++;}
 }
 return env;
}
/** Envelope per profile row, matched on the row's own depth and pair reference; null where no peers exist. */
export function zoneEnvelope(env,f,zone){
 if(!env)return null;
 return zone.rows.map(row=>{const p=row.pairs[0];return p?env.get(key(f.component[row.bus],f.depth[row.bus],pairZone(p.pair)))??null:null;});
}
