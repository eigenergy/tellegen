import {readFile,writeFile} from 'node:fs/promises';import {dirname,join} from 'node:path';import {fileURLToPath} from 'node:url';import {createHash} from 'node:crypto';
import {decodeFixture} from './fixture.mjs';import {bindResults} from './results-adapter.mjs';import {prepareResults,visibleAnchors,flowGlyphs,voltageState} from './results-overlay.mjs';
const here=dirname(fileURLToPath(import.meta.url)),read=async file=>JSON.parse(await readFile(join(here,'local-data',file),'utf8'));
const catalog=await read('manifest.json'),rc=await read('results-manifest.json'),rows=[];
const percentile=(a,p)=>[...a].sort((a,b)=>a-b)[Math.floor((a.length-1)*p)];
for(const id of ['french-medium','sapn-combined']){
 const m=catalog.find(m=>m.id===id),g=m.layouts['tidy-tree-equipment'],refs=await read(m.refsFile),meta=rc.datasets[id],bytes=await readFile(join(here,'local-data',meta.file));
 if(createHash('sha256').update(bytes).digest('hex')!==meta.sha256)throw new Error('Result hash mismatch');
 const packed=await readFile(join(here,'local-data',g.file)),f=decodeFixture({...m,...g},packed.buffer.slice(packed.byteOffset,packed.byteOffset+packed.byteLength));
 f.refs=refs;f.nodeData={attributes:{}};f.lineData={attributes:{}};f.firstSegment=new Uint32Array(m.branches+1);let b=0;for(let s=0;s<f.manifest.segments;s++)while(b<=f.segmentBranches[s])f.firstSegment[b++]=s;f.firstSegment[m.branches]=f.manifest.segments;
 const parse=[],bind=[],prepare=[];let result,r;
 for(let k=0;k<5;k++){let start=performance.now();result=JSON.parse(bytes);parse.push(performance.now()-start);start=performance.now();const bound=bindResults(result,refs);bind.push(performance.now()-start);start=performance.now();r=prepareResults(f,bound);prepare.push(performance.now()-start);}
 const scope=[],frames=[];let peakGlyphs=0,peakCandidates=0;
 for(let k=0;k<1000;k++){
  const a=r.anchors[(k*97)%r.anchors.length],camera={target:a.position,zoom:k%3===0?-7:.6};
  let start=performance.now();const q=visibleAnchors(r,camera,1190,560);scope.push(performance.now()-start);peakCandidates=Math.max(peakCandidates,q.stats.candidates);start=performance.now();const glyphs=flowGlyphs(r,q.anchors,camera.zoom,k/60);frames.push(performance.now()-start);peakGlyphs=Math.max(peakGlyphs,glyphs.length);
 }
 rows.push({id,kind:r.kind,buses:m.count,branches:m.branches,geometrySha256:g.sha256,refsSha256:m.refsSha256,resultSha256:meta.sha256,resultJsonBytes:bytes.length,resultColourArrayBytes:r.arrayBytes,anchorObjects:r.anchors.length,spatialBuckets:r.cells.size,parseMs:parse,bindMs:bind,prepareMs:prepare,medianParseMs:percentile(parse,.5),medianBindMs:percentile(bind,.5),medianPrepareMs:percentile(prepare,.5),queries:1000,scopeP95Ms:percentile(scope,.95),glyphGenerationP95Ms:percentile(frames,.95),peakGlyphs,peakCandidates,voltageViolatingBuses:r.kind==='solved'?r.buses.filter(b=>voltageState(b)==='violation').length:null,maxLoadingPercent:r.kind==='solved'?Math.max(...r.branches.map(b=>b.loading??0)):null,missingBranchResults:r.branches.filter(b=>b.ends.some(e=>!e)).length,missingLoading:r.branches.filter(b=>b.loading===null).length,provenance:r.provenance});
}
const report={version:1,created:new Date().toISOString(),node:process.version,platform:process.platform,method:'Five parse/bind/prepare repetitions, 1,000 deterministic overview/detail spatial queries at 1190×560 and local marker generation. CPU only; not WebGL or browser frame timings. SAPN is explicitly artificial rendering stress on real topology. Arrays exclude JSON, JS maps/anchor objects, GPU/library copies.',rows};
await writeFile(join(here,'evidence/results-comparison.json'),JSON.stringify(report,null,2)+'\n');console.log(rows.map(({id,medianParseMs,medianBindMs,medianPrepareMs,scopeP95Ms,glyphGenerationP95Ms,peakGlyphs,resultColourArrayBytes,voltageViolatingBuses,maxLoadingPercent})=>({id,medianParseMs,medianBindMs,medianPrepareMs,scopeP95Ms,glyphGenerationP95Ms,peakGlyphs,resultColourArrayBytes,voltageViolatingBuses,maxLoadingPercent})));
