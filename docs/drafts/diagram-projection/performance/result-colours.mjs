/** Continuous result colour modes and flow widths. Stops mirror packages/svelte/src/lib/colors.ts (checked by test). */
import {selectionBus} from './engineering-workflow.mjs';
export const SEQUENTIAL_STOPS=[[246,227,180,235],[236,178,66,235],[212,116,34,240],[160,56,21,245],[84,18,7,250]];
export const DIVERGING_STOPS=[[27,120,55,240],[127,191,123,230],[223,219,208,160],[194,165,207,230],[118,42,131,240]];
export const COLOUR_MODES={
 none:{label:'Equipment / topology'},
 voltage:{label:'Voltage limits',threshold:true},
 'voltage-band':{label:'Voltage within band',continuous:true},
 loading:{label:'Loading thresholds',threshold:true},
 'loading-heat':{label:'Loading (continuous)',continuous:true}};
export function ramp(stops,t){
 const x=Math.min(1,Math.max(0,t))*(stops.length-1),i=Math.min(stops.length-2,Math.floor(x)),f=x-i;
 return stops[i].map((v,k)=>Math.round(v+(stops[i+1][k]-v)*f));
}
export const gradient=stops=>`linear-gradient(90deg, ${stops.map(s=>`rgb(${s[0]} ${s[1]} ${s[2]})`).join(', ')})`;
/** Position in the declared band: -1 at min, 0 mid-band, +1 at max; beyond ±1 is a violation. */
export const bandPosition=p=>(2*p.volts-p.min-p.max)/(p.max-p.min);
/** The pair nearest a limit decides a bus colour; a violation can never be averaged away. */
export function worstBand(pairs){let w=null;for(const p of pairs){const t=bandPosition(p);if(w===null||Math.abs(t)>Math.abs(w))w=t;}return w;}
/** Low loading starts part-way up the ramp so lightly loaded lines stay visible on paper. */
export const loadingColour=loading=>ramp(SEQUENTIAL_STOPS,.15+.85*Math.min(1,loading/100));
export const bandColour=t=>ramp(DIVERGING_STOPS,(Math.max(-1,Math.min(1,t))+1)/2);
const MISSING=[128,128,128,170],INACTIVE=[155,155,155,255];
/** Lazily build and cache one data set per colour mode; base geometry attributes are shared, only colours differ. */
export function modeData(f,r,mode){
 r.modeCache??=new Map();if(r.modeCache.has(mode))return r.modeCache.get(mode);
 let out={};
 if(mode==='voltage')out={nodeData:r.nodeData};
 else if(mode==='loading')out={lineData:r.lineData,compactLineData:r.compactLineData};
 else if(mode==='voltage-band'||mode==='loading-heat'){
  const nodeColors=new Uint8Array(f.manifest.count*4),lineColors=new Uint8Array(f.manifest.segments*4),busBand=new Float32Array(f.manifest.count);
  for(let i=0;i<f.manifest.count;i++){const t=r.buses[i].pairs.length?worstBand(r.buses[i].pairs):NaN;busBand[i]=t;nodeColors.set(Number.isFinite(t)?bandColour(t):MISSING,i*4);}
  for(let s=0;s<f.manifest.segments;s++){
   const b=f.segmentBranches[s],row=r.branches[b];let c;
   if(f.states[b])c=INACTIVE;
   else if(mode==='loading-heat')c=Number.isFinite(row.loading)?loadingColour(row.loading):MISSING;
   // A branch takes the band colour of its downstream (deeper) bus, so feeders read as voltage heat.
   else{const t=busBand[f.refs?selectionBus(f,'branch',b):f.endpoints[b*2+1]];c=Number.isFinite(t)?bandColour(t):MISSING;}
   lineColors.set(c,s*4);
  }
  out={nodeData:mode==='voltage-band'?{...f.nodeData,attributes:{...f.nodeData.attributes,getFillColor:{value:nodeColors,size:4,normalized:true}}}:undefined,
   lineData:{...f.lineData,attributes:{...f.lineData.attributes,getColor:{value:lineColors,size:4,normalized:true}}}};
  if(f.openTies?.compact){const cc=new Uint8Array(f.openTies.compact.segments.length*4);f.openTies.compact.segments.forEach((s,i)=>cc.set(lineColors.subarray(s*4,s*4+4),i*4));out.compactLineData={...f.compactLineData,attributes:{...f.compactLineData.attributes,getColor:{value:cc,size:4,normalized:true}}};}
  out.arrayBytes=nodeColors.byteLength+lineColors.byteLength+busBand.byteLength;
 }
 r.modeCache.set(mode,out);return out;
}
/** Pixel width per segment from the larger end |P|: square root, 1–6 px, fixed to the snapshot maximum. */
export function flowWidths(f,r){
 if(r.flowWidths)return r.flowWidths;
 const w=new Float32Array(f.manifest.segments),max=r.flowScales?.p||0;
 for(let s=0;s<w.length;s++){const b=f.segmentBranches[s],e=r.branches[b].ends,p=Math.max(Math.abs(e[0]?.p??0),Math.abs(e[1]?.p??0));w[s]=max>0&&!f.states[b]?1+5*Math.sqrt(Math.min(1,p/max)):1;}
 const compact=f.openTies?.compact?Float32Array.from(f.openTies.compact.segments,s=>w[s]):null;
 return r.flowWidths={full:w,compact};
}
