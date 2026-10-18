/** Result value labels by priority under a screen-space collision budget: selection, then severity, then magnitude. */
import {bandPosition} from './result-colours.mjs';
import {loadingState} from './results-overlay.mjs';
export const VALUE_LABEL_BUDGET=60,MIN_LABEL_ZOOM=-0.2;
const SEVERITY={violation:2,warning:1,normal:0,missing:0};
/** Greedy placement: candidates sorted by (selected, severity, priority); a coarse grid rejects overlaps. */
export function placeLabels(candidates,{width,height,budget=VALUE_LABEL_BUDGET,box=[64,16],margin=[0,40,0,24]}){
 const sorted=[...candidates].sort((a,b)=>(b.selected?1:0)-(a.selected?1:0)||SEVERITY[b.state]-SEVERITY[a.state]||b.priority-a.priority),occupied=new Set(),out=[];
 const [cw,ch]=box;
 for(const c of sorted){
  if(out.length>=budget)break;
  const [sx,sy]=c.screen;if(!c.selected&&(sx<margin[3]||sx+cw>width-margin[1]||sy-ch<margin[0]||sy>height-margin[2]))continue;
  const keys=[];for(let gy=Math.floor((sy-ch)/ch);gy<=Math.floor(sy/ch);gy++)for(let gx=Math.floor(sx/cw);gx<=Math.floor((sx+cw)/cw);gx++)keys.push(gx+','+gy);
  if(!c.selected&&keys.some(k=>occupied.has(k)))continue;
  keys.forEach(k=>occupied.add(k));out.push(c);
 }
 return out;
}
const v=x=>Number(x.toPrecision(x>=1000?5:4)).toLocaleString();
/** Candidate text and priority per view family; voltage uses the pair nearest a limit. */
export function busCandidate(r,i){
 const pairs=r.buses[i].pairs;if(!pairs.length)return null;
 const p=pairs.reduce((a,b)=>Math.abs(bandPosition(b))>Math.abs(bandPosition(a))?b:a),t=Math.abs(bandPosition(p));
 return {text:`${v(p.volts)} V`,state:t>1?'violation':'normal',priority:t};
}
export function branchCandidate(r,b,family){
 const row=r.branches[b];
 if(family==='loading')return Number.isFinite(row.loading)?{text:`${row.loading.toFixed(1)} %`,state:loadingState(row),priority:row.loading}:null;
 const e=row.ends[0]??row.ends[1];if(!e)return null;
 return {text:`${v(Math.abs(e.p))} kW`,state:'normal',priority:Math.abs(e.p)};
}
