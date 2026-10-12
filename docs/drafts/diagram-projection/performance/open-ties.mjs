/** Presentation only. Never changes state, identity, routes or the display forest. */
export function buildOpenTiePresentation(f){
  const branchIndices=[];
  f.refs?.branches.forEach((b,i)=>{if(b.family==='switch'&&b.openSwitch===true)branchIndices.push(i);});
  const branchSet=new Set(branchIndices),byBus=new Map();
  for(const b of branchIndices){
    const a=f.endpoints[b*2],z=f.endpoints[b*2+1];
    for(const bus of a===z?[a]:[a,z]){if(!byBus.has(bus))byBus.set(bus,[]);byBus.get(bus).push(b);}
  }
  const markers=[...byBus].map(([bus,branches])=>({bus,branches,position:f.positions.subarray(bus*3,bus*3+3)}));
  // No switches open: reuse the original layer, with no segment copies or extra wire layer.
  if(!branchIndices.length)return {branchIndices,branchSet,byBus,markers,compact:null,arrayBytes:0,suppressedSegments:0};
  let count=0;for(const b of f.segmentBranches)if(!branchSet.has(b))count++;
  const sources=new Float32Array(count*3),targets=new Float32Array(count*3),segments=new Uint32Array(count);
  for(let s=0,i=0;s<f.manifest.segments;s++)if(!branchSet.has(f.segmentBranches[s])){
    sources.set(f.segmentSources.subarray(s*3,s*3+3),i*3);targets.set(f.segmentTargets.subarray(s*3,s*3+3),i*3);segments[i++]=s;
  }
  return {branchIndices,branchSet,byBus,markers,compact:{sources,targets,segments},
    arrayBytes:sources.byteLength+targets.byteLength+segments.byteLength,suppressedSegments:f.manifest.segments-count};
}
/** Bus selection should not restore a bundle of suppressed open routes at once. */
export function incidentPresentation(branches,presentation,compact){
  return compact?Array.from(branches).filter(b=>!presentation.branchSet.has(b)):branches;
}
/** Map a compact draw/pick index to the original equipment owner. */
export function compactBranch(f,presentation,index){
  if(!presentation.compact||!Number.isInteger(index)||index<0||index>=presentation.compact?.segments.length)return null;
  const s=presentation.compact?.segments[index];return s===undefined?null:f.segmentBranches[s];
}
