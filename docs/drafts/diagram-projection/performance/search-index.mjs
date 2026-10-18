/** One search over buses, branches and bus-owned attachments by source identity. Linear scan; no fuzzy guessing. */
import {busQuery} from './bmopf-adapter.mjs';
export const KIND_ORDER={bus:0,branch:1,attachment:2};
const familyOf=(kind,row)=>kind==='bus'?'bus':row.family+(row.kind?' / '+row.kind:'');
/** Readable URL identity, e.g. `branch:line:feeder/L1`; kind + family + source + id stays unique when raw IDs repeat. */
export function entityKey(refs,kind,index){
 const row=kind==='bus'?refs.buses[index]:kind==='branch'?refs.branches[index]:refs.attachments[index];
 return `${kind}:${kind==='bus'?'':`${row.family}${row.kind?'.'+row.kind:''}:`}${busQuery(refs.sources[row.source].id,row.id)}`;
}
export function buildSearchIndex(refs){
 const entries=[],byKey=new Map();
 for(const [kind,rows] of [['bus',refs.buses],['branch',refs.branches],['attachment',refs.attachments??[]]])rows.forEach((row,index)=>{
  const source=refs.sources[row.source],qualified=busQuery(source.id,row.id),entry={kind,index,id:row.id,family:familyOf(kind,row),source:source.label,qualified,id_lc:String(row.id).toLowerCase(),q_lc:qualified.toLowerCase()};
  entries.push(entry);byKey.set(entityKey(refs,kind,index),entry);
 });
 return {entries,byKey};
}
/** Rank: exact ID, exact qualified ID, ID prefix, then substring. Ties prefer buses, shorter IDs, then index. */
export function search(index,query,{limit=8,kinds=null}={}){
 const q=query.trim().toLowerCase();if(!q)return {matches:[],total:0};
 const hits=[];
 for(const e of index.entries){
  if(kinds&&!kinds.includes(e.kind))continue;
  const rank=e.id_lc===q?0:e.q_lc===q?1:e.id_lc.startsWith(q)?2:e.id_lc.includes(q)||e.q_lc.includes(q)?3:-1;
  if(rank>=0)hits.push({rank,e});
 }
 hits.sort((a,b)=>a.rank-b.rank||KIND_ORDER[a.e.kind]-KIND_ORDER[b.e.kind]||a.e.id.length-b.e.id.length||a.e.index-b.e.index);
 return {matches:hits.slice(0,limit).map(h=>({...h.e,exact:h.rank<=1})),total:hits.length};
}
/** Synthetic fixtures only have generated bus names B000001…; accept either form. */
export function syntheticBus(query,count){
 const digits=query.trim().replace(/^B/i,''),id=Number(digits);
 return /^\d+$/.test(digits)&&Number.isSafeInteger(id)&&id>=1&&id<=count?id-1:null;
}
