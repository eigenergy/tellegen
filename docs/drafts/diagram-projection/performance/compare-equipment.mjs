/** Aggregate evidence only. Raw source records and staged geometry remain external. */
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {cpus,platform,arch} from 'node:os';
import assert from 'node:assert/strict';
import {decodeFixture} from './fixture.mjs';
import {validateReferences} from './bmopf-references.mjs';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {buildOpenTiePresentation} from './open-ties.mjs';
import {prepareEquipment} from './equipment.mjs';
import {equipmentClearanceMetrics} from './equipment-metrics.mjs';
const root=new URL('./local-data/',import.meta.url),catalog=JSON.parse(await readFile(new URL('manifest.json',root)));
const hash=x=>createHash('sha256').update(x).digest('hex'),median=a=>[...a].sort((a,b)=>a-b)[Math.floor(a.length/2)];
const report={version:1,created:new Date().toISOString(),environment:{node:process.version,platform:platform(),arch:arch(),cpu:cpus()[0]?.model},methodology:'One warm-up and five layout/preparation samples. Layout includes packing and glyph tessellation, excludes source parsing, validation and diagnostics. Preparation includes open-tie cache and glyph GPU inputs, excludes general fixture/browser preparation and color assignment. Packed bytes exclude JS/Map objects and GPU copies. Clearance audit counts strict-interior foreign base-route/reserved-box intersections; excludes labels, glyph/stem collisions, pixel widths and screen-space crowding. Tidy-tree baseline and equipment share identities, endpoint/state arrays, spanning forest and branch roles. Footprint uses geometry bounds including fixed padding.',datasets:[]};
for(const id of ['mvlv-meshed-documented','mvlv-meshed-all-closed','sapn-combined']){
 const base=catalog.find(x=>x.id===id);assert.ok(base,'Stage BMOPF datasets first');
 const refsBytes=await readFile(new URL(base.refsFile,root));assert.equal(hash(refsBytes),base.refsSha256);const refs=JSON.parse(refsBytes);let baseline;
 for(const routing of ['crossing-aware','equipment']){
  const m={...base,...base.layouts['tidy-tree-'+routing]},bytes=await readFile(new URL(m.file,root));assert.equal(hash(bytes),m.sha256);
  const f=decodeFixture(m,bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength));f.refs=refs;validateReferences(f);validateSchematic(f);
  if(!baseline)baseline=f;else for(const key of ['endpoints','states','parent','parentBranch','branchRoles'])assert.deepEqual(f[key],baseline[key],key+' changed');
  makeSchematic(refs,{strategy:'tidy-tree',routing});const layoutMs=[],prepareMs=[];
  for(let i=0;i<5;i++){const t=performance.now();const generated=makeSchematic(refs,{strategy:'tidy-tree',routing});layoutMs.push(performance.now()-t);assert.equal(hash(new Uint8Array(generated.buffer)),m.sha256,'Staged geometry is not reproducible');}
  f.openTies=buildOpenTiePresentation(f);prepareEquipment(f);let prepared;
  for(let i=0;i<5;i++){const t=performance.now();f.openTies=buildOpenTiePresentation(f);prepared=prepareEquipment(f);prepareMs.push(performance.now()-t);}
  const clearance=routing==='equipment'?equipmentClearanceMetrics(f):null;if(clearance)assert.equal(clearance.foreignRouteBoxIntersections,0);
  const b=m.bounds,row={id,routing,buses:m.count,branches:m.branches,sourceHashes:refs.sources.map(s=>({id:s.id,sha256:s.sha256})),referenceHash:base.refsSha256,geometryHash:m.sha256,packedBytes:m.bytes,wireSegments:m.segments,glyphs:m.equipment?.glyphs??0,glyphStrokes:m.equipment?.strokes??0,equipmentExtraArrayBytes:prepared?.arrayBytes??0,width:b[2]-b[0],height:b[3]-b[1],layoutMs,medianLayoutMs:median(layoutMs),prepareMs,medianPrepareMs:median(prepareMs),clearance};
  report.datasets.push(row);console.log(JSON.stringify(row));
 }
 assert.equal(hash(refsBytes),base.refsSha256);
}
if(process.argv[2])await writeFile(process.argv[2],JSON.stringify(report,null,2)+'\n');
