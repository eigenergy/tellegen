import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {validateSchematic} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
import {validateReferences} from './bmopf-references.mjs';
const root=new URL('./local-data/',import.meta.url),catalog=JSON.parse(await readFile(new URL('manifest.json',root)));
const hash=b=>createHash('sha256').update(b).digest('hex');
for(const dataset of catalog){
for(const m of [dataset,...Object.values(dataset.layouts).map(layout=>({...dataset,...layout}))]){
  const data=await readFile(new URL(m.file,root)),refs=await readFile(new URL(m.refsFile,root));
  if(data.length!==m.bytes||refs.length!==m.refsBytes||hash(data)!==m.sha256||hash(refs)!==m.refsSha256)throw new Error(`${m.id}: stale hash or size`);
  const buffer=data.buffer.slice(data.byteOffset,data.byteOffset+data.byteLength),f=decodeFixture(m,buffer);f.refs=JSON.parse(refs);validateReferences(f);if(m.coordinateSpace==='schematic')validateSchematic(f);
  let previous=-1;const seen=new Uint8Array(m.branches);
  for(let s=0;s<m.segments;s++){
    const b=f.segmentBranches[s];if(b<previous)throw new Error('Unsorted segment ownership');
    if(b!==previous){const from=f.endpoints[b*2];for(let axis=0;axis<3;axis++)if(f.segmentSources[s*3+axis]!==f.positions[from*3+axis])throw new Error('Route source mismatch');}
    if(s===m.segments-1||f.segmentBranches[s+1]!==b){const to=f.endpoints[b*2+1];for(let axis=0;axis<3;axis++)if(f.segmentTargets[s*3+axis]!==f.positions[to*3+axis])throw new Error('Route target mismatch');}
    previous=b;seen[b]=1;
  }
  if(seen.some(n=>n!==1))throw new Error('Unrendered branch');
  console.log(`${m.id} / ${m.layoutMode??'source'}: hashes, namespaces, attachments, endpoint routes and ${m.count} buses verified`);
}
}
const combined=catalog.find(m=>m.id==='sapn-combined'),members=combined.sources.map(s=>catalog.find(m=>m.id===s.id));
for(const key of ['count','lines','transformers','branches','attachments'])if(combined[key]!==members.reduce((n,m)=>n+m[key],0))throw new Error(`Collection ${key} mismatch`);
console.log('SAPN collection equals its five source inventories; no cross-source edges.');
