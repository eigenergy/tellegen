/** Small SVG charts for one selected path, with explicit references and per-pair limits. */
import {formatValue,severity} from './results-adapter.mjs';
const esc=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
/** `envelopes[zone][row]` (optional) is the min/max of all peers at that depth and reference, drawn behind the path. */
export function profileMarkup(profile,refs,envelopes=null){
 if(!profile.zones.length)return `<p class="fine">${esc(profile.notice)}</p>`;
 return profile.zones.map((zone,zoneIndex)=>{
  const rows=zone.rows,env=envelopes?.[zoneIndex]??null,all=[...rows.flatMap(r=>r.pairs.flatMap(p=>[p.volts,p.min,p.max])),...(env??[]).flatMap(e=>e?[e.min,e.max]:[])];
  if(!all.length)return '<p class="fine">Voltage reference/limits unavailable in this part of the path.</p>';
  const low=Math.min(...all),high=Math.max(...all),pad=Math.max((high-low)*.15,1),lo=low-pad,hi=high+pad;
  const x=i=>rows.length===1?330:58+i*540/(rows.length-1),y=v=>145-(v-lo)/(hi-lo)*112;
  const pairs=rows[0].pairs,marks=[];
  const band=env?rows.map((r,i)=>env[i]?[x(i),env[i]]:null).filter(Boolean):[];
  if(band.length>1)marks.push(`<polygon class="envelope" points="${[...band.map(([px,e])=>`${px},${y(e.max)}`),...band.reverse().map(([px,e])=>`${px},${y(e.min)}`)].join(' ')}"><title>Min–max of all buses at the same depth and reference</title></polygon>`);
  pairs.forEach((pair,k)=>{
   const cls=`phase-${k%3}`,points=rows.map((r,i)=>({r,i,p:r.pairs[k]}));
   for(const key of ['min','max','volts'])marks.push(`<polyline class="${cls} ${key==='volts'?'measured':'limit'}" points="${points.map(({i,p})=>`${x(i)},${y(p[key])}`).join(' ')}"/>`);
   points.forEach(({r,i,p})=>{const bad=severity(p.volts,p.min,p.max)==='violation',px=x(i),py=y(p.volts);marks.push(`<g class="profile-point ${cls}${bad?' violation':''}" data-profile-bus="${r.bus}" data-profile-pair="${k}" tabindex="0" role="button" aria-label="${esc(refs.buses[r.bus].id+' '+p.pair.join('–')+' '+formatValue(p.volts)+' V'+(bad?' violation':''))}"><title>${esc(refs.buses[r.bus].id+' · '+p.pair.join('–')+' · '+formatValue(p.volts)+' V')}</title><circle class="hit" cx="${px}" cy="${py}" r="9"/><circle cx="${px}" cy="${py}" r="${bad?4.8:3.5}"/>${bad?`<text x="${px}" y="${py-10}" class="violation-label">!</text>`:''}</g>`);});
  });
  const ticks=[lo,(lo+hi)/2,hi].map(v=>`<line class="grid" x1="58" x2="598" y1="${y(v)}" y2="${y(v)}"/><text x="49" y="${y(v)+4}" text-anchor="end">${v.toFixed(hi>1000?0:1)}</text>`).join('');
  return `<section class="voltage-zone"><h3>${zoneIndex+1}. ${esc(zone.reason)} · ${esc(pairs.map(p=>p.pair.join('–')).join(' / '))}</h3><svg viewBox="0 0 640 180" aria-label="Voltage profile zone ${zoneIndex+1}, volts versus branch steps" role="group"><text x="14" y="17">V</text>${ticks}${marks.join('')}<text x="58" y="170">Step ${rows[0].hop}</text><text x="598" y="170" text-anchor="end">Step ${rows.at(-1).hop}</text></svg><p class="fine">${esc(refs.buses[rows[0].bus].id)} → ${esc(refs.buses[rows.at(-1).bus].id)}</p></section>`;
 }).join('');
}
