import {writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {SIZES,makeFixture} from './fixture.mjs';
const dir=new URL('./fixtures/',import.meta.url);await mkdir(dir,{recursive:true});
const entries=[];
for(const size of SIZES) {
  const {manifest,buffer}=makeFixture(size);
  manifest.sha256=createHash('sha256').update(new Uint8Array(buffer)).digest('hex');
  await writeFile(new URL(manifest.file,dir),new Uint8Array(buffer)); entries.push(manifest);
  console.log(`${size.toLocaleString()} buses, ${manifest.branches.toLocaleString()} branches, ${manifest.segments.toLocaleString()} segments, ${(manifest.bytes/1048576).toFixed(2)} MiB`);
}
await writeFile(new URL('manifest.json',dir),JSON.stringify(entries,null,2)+'\n');
