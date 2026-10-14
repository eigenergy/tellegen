import {equipmentTemplate} from '../symbols/symbols.mjs';
export const EQUIPMENT_CLEARANCE={portGap:40,margin:68,slotOffset:34,halfBox:16,attachmentGap:40};
export const isInline=b=>b.family==='transformer'||b.family==='switch';
/** Numeric owners index the unchanged source sidecar: branches, then attachments. */
export function equipmentGeometry(model,positions,routed){
 const ss=[],tt=[],owners=[],centers=[],glyphOwners=[],junctions=[],gaps=new Map(),inlineGaps=new Float32Array(model.branches.length*6),templates=new Map();
 const segment=(a,b,owner)=>{ss.push(a[0],a[1],0);tt.push(b[0],b[1],0);owners.push(owner);};
 function glyph(kind,state,center,owner,direction=1){
  const key=kind+'/'+state;if(!templates.has(key))templates.set(key,equipmentTemplate(kind,state));const template=templates.get(key);
  centers.push(...center,0);glyphOwners.push(owner);
  for(const [a,b] of template.segments)segment([center[0]+direction*a[0],center[1]+a[1]],[center[0]+direction*b[0],center[1]+b[1]],owner);
  return template;
 }
 model.branches.forEach((b,i)=>{
  if(!isInline(b))return;
  const [a,z]=routed.points(i),direction=Math.sign(z[0]-a[0]);
  if(!direction||Math.abs(z[0]-a[0])<EQUIPMENT_CLEARANCE.margin)throw new Error('Insufficient inline equipment clearance');
  const center=[a[0]+direction*EQUIPMENT_CLEARANCE.slotOffset,a[1]],t=glyph(b.family,b.openSwitch?'open':'closed',center,i,direction);
  const left=[center[0]-direction*t.halfWidth,center[1]],right=[center[0]+direction*t.halfWidth,center[1]];
  gaps.set(i,[left,right]);inlineGaps.set([...left,0,...right,0],i*6);
 });
 const next=new Uint32Array(model.buses.length);
 model.attachments.forEach((a,i)=>{
  const u=a.bus,x=positions[u*3],barBottom=routed.arrays.busbarTargets[u*3+1],slot=next[u]++,y=barBottom+(slot+1)*EQUIPMENT_CLEARANCE.attachmentGap;
  const center=[x-EQUIPMENT_CLEARANCE.slotOffset,y],owner=model.branches.length+i,t=glyph(a.family,'closed',center,owner);
  segment([x,slot?y-EQUIPMENT_CLEARANCE.attachmentGap:barBottom],[x,y],owner);segment([x,y],[center[0]+t.halfWidth,y],owner);
  junctions.push(x,y,0);
  if(a.disabled){segment([center[0]-12,y-12],[center[0]+12,y+12],owner);}
 });
 const arrays={equipmentInlineGaps:inlineGaps,equipmentSources:new Float32Array(ss),equipmentTargets:new Float32Array(tt),equipmentOwners:new Uint32Array(owners),equipmentCenters:new Float32Array(centers),equipmentGlyphOwners:new Uint32Array(glyphOwners),attachmentJunctions:new Float32Array(junctions)};
 return {arrays,gaps,metadata:{version:1,glyphs:glyphOwners.length,strokes:owners.length,inline:gaps.size,attachments:model.attachments.length,clearance:EQUIPMENT_CLEARANCE,ownerEncoding:'branch index; branches.length + attachment index',generalShunts:true}};
}
/** Cache GPU inputs once per fixture/presentation, retaining original pick identities. */
export function prepareEquipment(f){
 if(!f.equipmentOwners)return null;
 function subset(compact){
  const indices=[],glyphs=[];
  const hidden=owner=>compact&&f.openTies.branchSet.has(owner);
  for(let i=0;i<f.equipmentOwners.length;i++)if(!hidden(f.equipmentOwners[i]))indices.push(i);
  for(let i=0;i<f.equipmentGlyphOwners.length;i++)if(!hidden(f.equipmentGlyphOwners[i]))glyphs.push(i);
  const sources=new Float32Array(indices.length*3),targets=new Float32Array(indices.length*3),centers=new Float32Array(glyphs.length*3);
  indices.forEach((i,j)=>{sources.set(f.equipmentSources.subarray(i*3,i*3+3),j*3);targets.set(f.equipmentTargets.subarray(i*3,i*3+3),j*3);});
  glyphs.forEach((i,j)=>centers.set(f.equipmentCenters.subarray(i*3,i*3+3),j*3));
  return {owners:Uint32Array.from(indices,i=>f.equipmentOwners[i]),glyphOwners:Uint32Array.from(glyphs,i=>f.equipmentGlyphOwners[i]),sources,targets,centers};
 }
 // Full view reuses packed arrays. Compact copies only when open switches exist.
 const full={owners:f.equipmentOwners,glyphOwners:f.equipmentGlyphOwners,sources:f.equipmentSources,targets:f.equipmentTargets,centers:f.equipmentCenters};
 const compact=f.openTies.branchSet.size?subset(true):full;
 const ranges=new Map();for(let i=0;i<full.owners.length;i++){const owner=full.owners[i];if(!ranges.has(owner))ranges.set(owner,[i,i+1]);else ranges.get(owner)[1]=i+1;}
 for(const data of new Set([full,compact])){
  data.colors=new Uint8Array(data.owners.length*4);
  data.lines={length:data.owners.length,attributes:{getSourcePosition:{value:data.sources,size:3},getTargetPosition:{value:data.targets,size:3},getColor:{value:data.colors,size:4,normalized:true}}};
  data.hits={length:data.glyphOwners.length,attributes:{getPosition:{value:data.centers,size:3}}};
 }
 const glyphIndex=new Map(Array.from(full.glyphOwners,(owner,i)=>[owner,i]));
 return {full,compact,ranges,glyphIndex,arrayBytes:full.colors.byteLength+(compact===full?0:compact.sources.byteLength+compact.targets.byteLength+compact.centers.byteLength+compact.owners.byteLength+compact.glyphOwners.byteLength+compact.colors.byteLength)};
}
export function equipmentRow(f,owner){return owner<f.manifest.branches?{kind:'branch',index:owner,row:f.refs.branches[owner]}:{kind:'attachment',index:owner-f.manifest.branches,row:f.refs.attachments[owner-f.manifest.branches]};}

export function validateEquipment(f){
 const m=f.manifest.equipment;if(!m)return;
 const expected={equipmentInlineGaps:f.manifest.branches*6,equipmentSources:m.strokes*3,equipmentTargets:m.strokes*3,equipmentOwners:m.strokes,equipmentCenters:m.glyphs*3,equipmentGlyphOwners:m.glyphs,attachmentJunctions:m.attachments*3};
 for(const [name,size] of Object.entries(expected))if(f[name]?.length!==size||!f[name].every(Number.isFinite))throw new Error('Invalid equipment array: '+name);
 const limit=f.manifest.branches+m.attachments,owners=new Set(f.equipmentGlyphOwners);
 if(owners.size!==m.glyphs||!f.equipmentGlyphOwners.every(o=>o<limit)||!f.equipmentOwners.every(o=>owners.has(o)))throw new Error('Invalid equipment source owner');
 if(f.refs){for(const owner of owners){const {row}=equipmentRow(f,owner);if(!row||owner<f.manifest.branches&&!isInline(row))throw new Error('Equipment identity mismatch');}}
 let inline=0;
 for(const owner of owners)if(owner<f.manifest.branches)inline++;
 if(inline!==m.inline||m.glyphs!==inline+m.attachments)throw new Error('Equipment counts mismatch');
}
