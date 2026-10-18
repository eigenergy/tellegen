import {readFile,writeFile} from 'node:fs/promises';
import {dirname,join} from 'node:path';import {fileURLToPath} from 'node:url';import {createHash} from 'node:crypto';
import {decodeFixture} from './fixture.mjs';import {bindResults} from './results-adapter.mjs';
import {prepareResults,visibleAnchors,flowGlyphs} from './results-overlay.mjs';
import {resultIssues,voltageProfile} from './engineering-workflow.mjs';import {profileMarkup} from './voltage-profile-view.mjs';
const here=dirname(fileURLToPath(import.meta.url)),read=async file=>JSON.parse(await readFile(join(here,'local-data',file),'utf8'));
const catalog=await read('manifest.json'),results=await read('results-manifest.json'),rows=[];
const p95=a=>a.sort((a,b)=>a-b)[Math.floor((a.length-1)*.95)];
for(const id of ['french-medium','sapn-combined']){
 const m=catalog.find(m=>m.id===id),g=m.layouts['tidy-tree-equipment'],refs=await read(m.refsFile),meta=results.datasets[id],bytes=await readFile(join(here,'local-data',meta.file));
 if(createHash('sha256').update(bytes).digest('hex')!==meta.sha256)throw new Error('Result hash mismatch');
 const packed=await readFile(join(here,'local-data',g.file)),f=decodeFixture({...m,...g},packed.buffer.slice(packed.byteOffset,packed.byteOffset+packed.byteLength));
 f.refs=refs;f.nodeData={attributes:{}};f.lineData={attributes:{}};f.firstSegment=new Uint32Array(m.branches+1);let b=0;
 for(let s=0;s<f.manifest.segments;s++)while(b<=f.segmentBranches[s])f.firstSegment[b++]=s;f.firstSegment[m.branches]=f.manifest.segments;
 const r=prepareResults(f,bindResults(JSON.parse(bytes),refs));let t=performance.now();const issues=resultIssues(r),issueIndexMs=performance.now()-t;
 const profileMs=[],markupMs=[],scopeMs=[],glyphMs=[];let maxPath=0,maxShown=0,maxMarkupBytes=0,peakGlyphs=0;
 for(let k=0;k<1000;k++){
  const target=(k*997)%m.count;t=performance.now();const p=voltageProfile(f,r,target);profileMs.push(performance.now()-t);maxPath=Math.max(maxPath,p.nodes.length);maxShown=Math.max(maxShown,p.shown?.length??0);
  t=performance.now();const svg=profileMarkup(p,refs);markupMs.push(performance.now()-t);maxMarkupBytes=Math.max(maxMarkupBytes,Buffer.byteLength(svg));
  const a=r.anchors[(k*97)%r.anchors.length];t=performance.now();const q=visibleAnchors(r,{target:a.position,zoom:.6},692,500);scopeMs.push(performance.now()-t);
  t=performance.now();const glyphs=flowGlyphs(r,q.anchors,.6,k/60);glyphMs.push(performance.now()-t);peakGlyphs=Math.max(peakGlyphs,glyphs.length);
 }
 rows.push({id,buses:m.count,kind:r.kind,resultSha256:meta.sha256,geometrySha256:g.sha256,flowScales:r.flowScales,issueIndexMs,voltageIssueCount:issues.voltage.length,samples:1000,profileP95Ms:p95(profileMs),markupP95Ms:p95(markupMs),scopeP95Ms:p95(scopeMs),glyphP95Ms:p95(glyphMs),maxPath,maxShown,maxMarkupBytes,peakGlyphs});
}
const report={created:new Date().toISOString(),node:process.version,method:'1,000 deterministic selected-bus paths and local-camera queries per dataset. Profile construction and SVG string generation exclude browser DOM insertion, picking and WebGL. SAPN values are artificial stress, not a solution. Snapshot scales are computed once during result preparation.',rows};
await writeFile(join(here,'evidence/engineering-workflow-cpu.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(rows,null,2));
