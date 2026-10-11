import {sourceKey} from './bmopf-adapter.mjs';
/** Validate the reference sidecar against geometry before permitting selection. */
export function validateReferences(f){
  const r=f.refs,m=f.manifest;
  if(r?.version!==1||r.buses?.length!==m.count||r.branches?.length!==m.branches||r.attachments?.length!==m.attachments||r.sources?.length!==m.sources?.length)throw new Error('Reference counts do not match geometry');
  const names=new Set();
  for(let i=0;i<r.sources.length;i++){const s=r.sources[i];if(typeof s.id!=='string'||names.has(s.id)||s.id!==m.sources[i].id||s.sha256!==m.sources[i].sha256)throw new Error('Invalid source namespace');names.add(s.id);}
  const identities=new Set();
  function check(row,family){
    if(typeof row.id!=='string'||!Number.isSafeInteger(row.source)||!r.sources[row.source])throw new Error('Invalid source reference');
    const key=sourceKey(r.sources[row.source].id,family,row.id,row.kind??'');
    if(identities.has(key))throw new Error('Duplicate source identity');identities.add(key);
  }
  r.buses.forEach(b=>check(b,'bus'));
  r.branches.forEach((b,i)=>{
    check(b,b.family);
    if(b.family==='switch'&&([b.openSwitch,b.sourceOpenSwitch,b.outOfService].some(v=>typeof v!=='boolean')||b.disabled!==(b.openSwitch||b.outOfService)))throw new Error('Invalid switch state reference');
    if(b.from!==f.endpoints[i*2]||b.to!==f.endpoints[i*2+1]||r.buses[b.from]?.source!==b.source||r.buses[b.to]?.source!==b.source||Number(b.disabled)!==f.states[i])throw new Error('Branch references disagree with geometry');
  });
  r.attachments.forEach(a=>{check(a,a.family);if(r.buses[a.bus]?.source!==a.source)throw new Error('Invalid attachment bus');});
}
