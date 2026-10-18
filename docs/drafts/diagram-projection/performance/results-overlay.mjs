/** Bounded local annotations; resident binary colour buffers change only with the result/mode. */
import {snapshotFlowScales,flowSpeed} from './engineering-workflow.mjs';
import {severity,formatValue} from './results-adapter.mjs';
export const MAX_ANCHORS=256,MAX_CANDIDATES=4096,MIN_ZOOM=-0.2;
const neutral=[117,142,148,255],warning=[219,158,61,255],violation=[225,91,88,255];
export function voltageState(row){return !row.pairs.length?'missing':row.pairs.some(p=>severity(p.volts,p.min,p.max)==='violation')?'violation':'normal';}
export function loadingState(row){return !Number.isFinite(row.loading)?'missing':row.loading>100?'violation':row.loading>=90?'warning':'normal';}
const colour=state=>state==='violation'?violation:state==='warning'?warning:neutral;
export function prepareResults(f,result){
 const started=performance.now(),r={...result,anchors:[],cells:new Map(),nodeColors:new Uint8Array(f.manifest.count*4),lineColors:new Uint8Array(f.manifest.segments*4)};
 for(let i=0;i<r.buses.length;i++)r.nodeColors.set(colour(voltageState(r.buses[i])),i*4);
 for(let s=0;s<f.manifest.segments;s++)r.lineColors.set(f.states[f.segmentBranches[s]]?[155,155,155,255]:colour(loadingState(r.branches[f.segmentBranches[s]])),s*4);
 r.nodeData={...f.nodeData,attributes:{...f.nodeData.attributes,getFillColor:{value:r.nodeColors,size:4,normalized:true}}};
 r.lineData={...f.lineData,attributes:{...f.lineData.attributes,getColor:{value:r.lineColors,size:4,normalized:true}}};
 if(f.openTies?.compact){r.compactColors=new Uint8Array(f.openTies.compact.segments.length*4);f.openTies.compact.segments.forEach((s,i)=>r.compactColors.set(r.lineColors.subarray(s*4,s*4+4),i*4));r.compactLineData={...f.compactLineData,attributes:{...f.compactLineData.attributes,getColor:{value:r.compactColors,size:4,normalized:true}}};}
 for(let b=0;b<r.branches.length;b++){
  if(f.states[b])continue;
  const start=f.firstSegment[b],end=f.firstSegment[b+1];if(start===end)continue;
  for(let side=0;side<2;side++){
   if(!r.branches[b].ends[side])continue;
   // The packer preserves source-from → source-to order, including symbol gaps.
   const s=side?end-1:start,a=Array.from(f.segmentSources.subarray(s*3,s*3+3)),z=Array.from(f.segmentTargets.subarray(s*3,s*3+3)),length=Math.hypot(z[0]-a[0],z[1]-a[1]);
   if(length<1)continue;
   const anchor={b,side,a,z,length,position:a.map((x,i)=>x+(z[i]-x)*(side?.75:.25))},index=r.anchors.length;r.anchors.push(anchor);
   const key=`${Math.floor(anchor.position[0]/128)},${Math.floor(anchor.position[1]/128)}`;if(!r.cells.has(key))r.cells.set(key,[]);r.cells.get(key).push(index);
  }
 }
 r.flowScales=snapshotFlowScales(result);
 r.arrayBytes=r.nodeColors.byteLength+r.lineColors.byteLength+(r.compactColors?.byteLength??0);r.prepareMs=performance.now()-started;return r;
}
export function visibleAnchors(r,camera,width,height,selected=null){
 const stats={candidates:0,cells:0,capped:false};if(camera.zoom<MIN_ZOOM)return {anchors:[],stats};
 const scale=2**camera.zoom,[cx,cy]=camera.target,x0=cx-width/2/scale,y0=cy-height/2/scale,x1=cx+width/2/scale,y1=cy+height/2/scale;
 const nx=Math.floor(x1/128)-Math.floor(x0/128)+1,ny=Math.floor(y1/128)-Math.floor(y0/128)+1;
 if(nx*ny>MAX_CANDIDATES)return {anchors:[],stats:{...stats,capped:true}};
 const anchors=[];
 outer:for(let y=Math.floor(y0/128);y<=Math.floor(y1/128);y++)for(let x=Math.floor(x0/128);x<=Math.floor(x1/128);x++){
  stats.cells++;for(const i of r.cells.get(`${x},${y}`)??[]){
   if(++stats.candidates>MAX_CANDIDATES){stats.capped=true;break outer;}
   const a=r.anchors[i],[px,py]=a.position;
   if(px<x0||px>x1||py<y0||py>y1||a.length*scale<28||selected!==null&&a.b!==selected)continue;
   anchors.push(a);if(anchors.length>=MAX_ANCHORS){stats.capped=true;break outer;}
  }
 }
 return {anchors,stats};
}
/** Positive end power enters the element. Source-to is reversed relative to route order. */
export function routeSign(value,side){return !Number.isFinite(value)||Math.abs(value)<1e-9?0:Math.sign(value)*(side?-1:1);}
export function flowGlyphs(r,anchors,zoom,seconds=0,channels={p:true,q:true}){
 const data=[],scale=2**zoom;
 for(const a of anchors)for(const [lane,key] of ['p','q'].entries()){
  if(!channels[key])continue;
  const value=r.branches[a.b].ends[a.side][key],direction=routeSign(value,a.side);if(!direction)continue;
  const dx=(a.z[0]-a.a[0])/a.length,dy=(a.z[1]-a.a[1])/a.length;
  const speed=flowSpeed(value,r.flowScales?.[key]??Math.abs(value)),periodPixels=.2*a.length*scale;
  const phase=((seconds*speed/periodPixels)%1+1)%1,t=(a.side?.65:.15)+.2*(direction>0?phase:1-phase),offset=(lane?7:-7)/scale;
  data.push({b:a.b,side:a.side,key,value,speed,text:lane?'›':'▸',angle:-Math.atan2(dy*direction,dx*direction)*180/Math.PI,position:[a.a[0]+(a.z[0]-a.a[0])*t-dy*offset,a.a[1]+(a.z[1]-a.a[1])*t+dx*offset,0],color:lane?[176,139,227,255]:[58,168,184,255]});
 }
 return data;
}
export function resultDescription(r,kind,index){
 if(!r)return 'No result snapshot available.';
 const prefix=r.kind==='stress'?'ARTIFICIAL STRESS VALUES — NOT A SOLUTION. ':'';
 if(kind==='bus')return prefix+(r.buses[index].pairs.map(p=>`${p.pair.join('–')}: ${formatValue(p.volts)} V [${formatValue(p.min)}, ${formatValue(p.max)} V] · ${severity(p.volts,p.min,p.max)}`).join('\n')||'Voltage limits/reference unavailable.');
 if(kind!=='branch')return prefix+'Select its bus for voltage or a branch for flows.';
 const b=r.branches[index];return prefix+b.ends.map((e,i)=>`${i?'To':'From'}: ${e?`${formatValue(e.p)} kW, ${formatValue(e.q)} kvar (positive INTO equipment)`:'unavailable / inactive'}`).join('\n')+`\nLoading: ${formatValue(b.loading)}% · ${loadingState(b)}\n${b.basis??'Rating unavailable'}\nEnd P/Q includes all terminal powers, including neutral. End totals need not be opposite.`;
}
