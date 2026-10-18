/** Optional solved-result staging; source models and solver dumps remain external. */
import {readFile,writeFile,rename} from 'node:fs/promises';
import {join,dirname,basename} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {projectFrenchResults,stressResults,bindResults} from './results-adapter.mjs';
const here=dirname(fileURLToPath(import.meta.url)),out=join(here,'local-data'),root=process.argv[2];
if(!root)throw new Error('Usage: node stage-results.mjs /path/to/BMOPFDraftData/output');
const read=async p=>JSON.parse(await readFile(p,'utf8')),hash=x=>createHash('sha256').update(x).digest('hex');
const catalog=await read(join(out,'manifest.json')),config=await read(join(here,'bmopf-sources.json')),manifest={version:1,datasets:{}};
const dir=join(root,'FrenchPowerGrids/validation/tellegen_pf'),summary=await read(join(dir,'summary.json')),build=await read(join(dir,'build_provenance.json'));
for(const id of ['french-medium','sapn-combined']){
 const m=catalog.find(m=>m.id===id);if(!m)throw new Error(`Stage geometry first: ${id}`);
 const refBytes=await readFile(join(out,m.refsFile));if(hash(refBytes)!==m.refsSha256)throw new Error('Stale refs');const refs=JSON.parse(refBytes);
 let result;
 if(id==='french-medium'){
  const source=config.sources.find(s=>s.id===id),input=await readFile(join(root,source.file)),record=summary.cases.find(c=>c.case===basename(source.file));
  if(record?.status!=='passed'||record.input_sha256!==hash(input)||refs.sources[0].sha256!==hash(input))throw new Error('Saved solve does not match staged input');
  const bytes=await readFile(join(dir,record.result_file)),solved=JSON.parse(gunzipSync(bytes));
  result=projectFrenchResults(JSON.parse(input),solved,refs,{solver:'Tellegen McPfResult',revision:build.revision,buildHadLocalChanges:!!build.worktree_status,created:summary.created_utc,inputSha256:hash(input),resultFile:record.result_file,resultSha256:hash(bytes),binarySha256:summary.binary_sha256,options:summary.options,iterations:solved.iterations,physicalKclResidualA:solved.physical_kcl_residual});
 }else result=stressResults(refs);
 bindResults(result,refs);
 const json=JSON.stringify(result),sha256=hash(json),file=`${id}.results-${sha256.slice(0,12)}.json`;
 await writeFile(join(out,file),json);
 manifest.datasets[id]={file,sha256,bytes:Buffer.byteLength(json),kind:result.kind,label:result.label};
 console.log(`${id}: ${result.kind}, ${result.buses.length} buses, ${result.branches.length} branches, ${(Buffer.byteLength(json)/1048576).toFixed(2)} MiB`);
}
await writeFile(join(out,'results-manifest.pending.json'),JSON.stringify(manifest,null,2)+'\n');
await rename(join(out,'results-manifest.pending.json'),join(out,'results-manifest.json'));
