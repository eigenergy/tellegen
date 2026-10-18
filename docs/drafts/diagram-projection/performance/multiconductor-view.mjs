/** Per-phase inspector: pair voltages against their declared band, and conductor currents per branch end. */
import {severity,formatValue} from './results-adapter.mjs';
import {magnitudeUnbalance,pairZone} from './workbench-summary.mjs';
const esc=v=>String(v).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const pct=x=>`${Math.max(0,Math.min(100,x)).toFixed(1)}%`;
const n=(v,d=2)=>Number.isFinite(v)?v.toLocaleString(undefined,{minimumFractionDigits:d,maximumFractionDigits:d}):'—';
/** Each pair on its own band: the track spans the declared band ±25 %, so position reads as margin, not volts. */
export function pairBars(pairs){
 if(!pairs.length)return '<p class="fine">No declared voltage pairs or limits at this bus.</p>';
 const rows=pairs.map((p,k)=>{
  const span=p.max-p.min,lo=p.min-.25*span,hi=p.max+.25*span,x=v=>100*(v-lo)/(hi-lo),state=severity(p.volts,p.min,p.max);
  return `<div class="mc-row ${state}" role="listitem"><span class="mc-label phase-${k%3}">${esc(p.pair.join('–'))}</span><span class="mc-track" title="Declared band ${formatValue(p.min)}–${formatValue(p.max)} V"><i class="mc-band" style="left:${pct(x(p.min))};width:${pct(x(p.max)-x(p.min))}"></i><i class="mc-mark" style="left:${pct(x(p.volts))}"></i></span><span class="mc-value">${n(p.volts)} V${state==='violation'?' !':''}</span></div>`;
 }).join('');
 const u=magnitudeUnbalance(pairs);
 return `<div class="mc-list" role="list" aria-label="Pair voltages against declared limits">${rows}</div><p class="fine">${esc(pairZone(pairs[0].pair))} · track = declared band ± 25 %${u===null?'':` · unbalance ${n(u)} % (${pairZone(pairs[0].pair)==='phase–phase'?'NEMA LVUR':'IEEE PVUR'})`}</p>`;
}
/** Conductor |I| at both ends on one scale; the neutral is called out because its current is an unbalance signal. */
export function conductorBars(ends){
 const live=ends.filter(e=>e?.currents?.length);if(!live.length)return '';
 const max=Math.max(...live.flatMap(e=>e.currents.map(c=>c.amps)).filter(Number.isFinite),1e-9);
 const section=(e,side)=>!e?.currents?.length?'':`<p class="mc-end">${side} end · ${n(e.p)} kW · ${n(e.q)} kvar</p><div class="mc-list" role="list" aria-label="${side} end conductor currents">${e.currents.map((c,k)=>`<div class="mc-row${c.terminal==='n'?' neutral':''}" role="listitem"><span class="mc-label ${c.terminal==='n'?'':'phase-'+(k%3)}">${esc(c.terminal)}</span><span class="mc-track"><i class="mc-fill" style="width:${pct(100*c.amps/max)}"></i></span><span class="mc-value">${n(c.amps,1)} A</span></div>`).join('')}</div>`;
 const neutral=live[0].currents.find(c=>c.terminal==='n'),phaseMax=Math.max(...live[0].currents.filter(c=>c.terminal!=='n').map(c=>c.amps));
 return section(ends[0],'From')+section(ends[1],'To')+`<p class="fine">Bars share one scale (max ${n(max,1)} A).${neutral&&phaseMax>0?` Neutral carries ${n(100*neutral.amps/phaseMax,0)} % of the largest phase current at the from end.`:''}</p>`;
}
export function inspectorMarkup(r,kind,busIndex,branchIndex){
 if(!r)return '';
 const stress=r.kind==='stress'?'<p class="fine stress">ARTIFICIAL STRESS VALUES — NOT A SOLUTION</p>':'';
 if(kind==='branch')return stress+conductorBars(r.branches[branchIndex].ends);
 return stress+pairBars(r.buses[busIndex].pairs);
}
