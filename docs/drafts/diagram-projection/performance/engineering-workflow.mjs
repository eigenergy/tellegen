/** Engineering interactions over a validated result snapshot, independent of rendering. */
import {formatValue,severity} from './results-adapter.mjs';
export const MAX_PROFILE_WALK=4096,MAX_PROFILE_POINTS=256;
export function snapshotFlowScales(result){
 const scales={p:0,q:0};for(const b of result.branches)for(const e of b.ends)if(e)for(const k of ['p','q'])if(Number.isFinite(e[k]))scales[k]=Math.max(scales[k],Math.abs(e[k]));return scales;
}
/** CSS px/s at 1× playback: square-root scale, capped at snapshot maximum. */
export function flowSpeed(value,maximum){
 if(!Number.isFinite(value)||!Number.isFinite(maximum)||maximum<=0||Math.abs(value)<1e-9)return 0;
 return Math.max(2,72*Math.sqrt(Math.min(1,Math.abs(value)/maximum)));
}
export function resultIssues(result){
 const voltage=[];let highest=-1,power=-1,peak=-1;
 result.buses.forEach((b,i)=>{if(b.pairs.some(p=>severity(p.volts,p.min,p.max)==='violation'))voltage.push(i);});
 result.branches.forEach((b,i)=>{if(b.loading!==null&&(highest<0||b.loading>result.branches[highest].loading))highest=i;for(const e of b.ends)if(e&&Math.abs(e.p)>peak){peak=Math.abs(e.p);power=i;}});
 return {voltage,highest,power};
}
export function selectionBus(f,kind,index){
 if(kind==='bus')return index;if(kind==='attachment')return f.refs.attachments[index].bus;
 const b=f.refs.branches[index];return f.depth&&f.depth[b.from]>f.depth[b.to]?b.from:b.to;
}
/** A display-tree path, never a claim about a unique path in a meshed circuit. */
export function voltageProfile(f,result,target,{maxWalk=MAX_PROFILE_WALK,maxPoints=MAX_PROFILE_POINTS}={}){
 if(!f.parent)return {notice:'Choose Tidy tree or Leaf order to inspect a display-tree voltage profile.',zones:[],nodes:[],edges:[]};
 const nodes=[],edges=[],seen=new Set();let u=target;
 while(true){
  if(!Number.isInteger(u)||u<0||u>=f.parent.length||seen.has(u))throw new Error('Invalid profile parent path');
  if(nodes.length>=maxWalk)return {notice:`Path exceeds ${maxWalk} buses. Choose a nearer bus; no partial path is presented as a complete feeder.`,zones:[],nodes:[],edges:[]};
  seen.add(u);nodes.push(u);const p=f.parent[u];if(p===u)break;
  const b=f.parentBranch[u],row=f.refs.branches[b];if(!row||row.disabled||!((row.from===u&&row.to===p)||(row.to===u&&row.from===p)))throw new Error('Invalid profile tree edge');
  edges.push(b);u=p;
 }
 nodes.reverse();edges.reverse();
 const shown=nodes.slice(-maxPoints),start=nodes.length-shown.length,zones=[];let lastSignature=null;
 for(let i=0;i<shown.length;i++){
  const bus=shown[i],pairs=result.buses[bus].pairs,hop=start+i,signature=JSON.stringify(pairs.map(p=>p.pair));
  const transformer=hop>0&&f.refs.branches[edges[hop-1]].family==='transformer';
  if(!zones.length||transformer||signature!==lastSignature)zones.push({reason:transformer?'Transformer boundary':zones.length?'Voltage reference changes':'Root-side voltage zone',rows:[]});
  zones.at(-1).rows.push({bus,hop,pairs});lastSignature=signature;
 }
 return {target,root:nodes[0],nodes,edges,zones,shown,notice:start?`Showing the last ${shown.length} of ${nodes.length} path buses; full path remains highlighted.`:'All path buses shown. Branch steps are topology distance, not kilometres.'};
}
export function hoverDescription(f,kind,index){
 const row=kind==='bus'?f.refs.buses[index]:kind==='attachment'?f.refs.attachments[index]:f.refs.branches[index];
 const lines=[`${f.refs.sources[row.source].label} / ${kind==='bus'?'bus':row.family}`,row.id];
 if(!f.results)return lines.concat('No result snapshot available.','Click to pin source details.').join('\n');
 if(f.results.kind==='stress')lines.unshift('ARTIFICIAL STRESS — NOT A SOLUTION');
 if(kind==='branch'){
  const b=f.results.branches[index];lines.push(`${f.refs.buses[row.from].id} → ${f.refs.buses[row.to].id}`);
  b.ends.forEach((e,i)=>lines.push(`${i?'To':'From'}: ${e?`${formatValue(e.p)} kW · ${formatValue(e.q)} kvar`:'unavailable / inactive'}`));
  lines.push('Positive P/Q enters equipment at that end.',`Loading: ${formatValue(b.loading)}%`,b.basis??'Rating unavailable');
 }else{
  const bus=selectionBus(f,kind,index);if(kind==='attachment')lines.push(`Connected bus: ${f.refs.buses[bus].id} (bus voltages below; device power unavailable)`);
  for(const p of f.results.buses[bus].pairs)lines.push(`${p.pair.join('–')}: ${formatValue(p.volts)} V · ${severity(p.volts,p.min,p.max)} [${formatValue(p.min)}, ${formatValue(p.max)}]`);
 }
 return lines.concat('Click to pin details and link the voltage profile.').join('\n');
}
