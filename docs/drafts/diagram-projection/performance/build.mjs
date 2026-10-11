import {createRequire} from 'node:module';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {writeFile,readFile} from 'node:fs/promises';
// Optional root lets an isolated design worktree reuse an installed workspace.
const here=dirname(fileURLToPath(import.meta.url));
const root=resolve(process.argv[2]||resolve(here,'../../../..'));
const req=createRequire(resolve(root,'package.json'));
const svelteReq=createRequire(resolve(root,'packages/svelte/package.json'));
const {build}=await import(pathToFileURL(req.resolve('rolldown')).href);
const core=resolve(dirname(req.resolve('@deck.gl/core')),'index.js'),layers=resolve(dirname(svelteReq.resolve('@deck.gl/layers')),'index.js');
const output=await build({input:resolve(here,'app.js'),resolve:{alias:{'@deck.gl/core':core,'@deck.gl/layers':layers}},
  output:{file:resolve(here,'app.bundle.js'),format:'iife',minify:true}});
const version=JSON.parse(await readFile(resolve(dirname(core),'../package.json'),'utf8')).version;
await writeFile(resolve(here,'build-info.json'),JSON.stringify({deckVersion:version,renderer:'deck.gl OrthographicView',build:'local workspace dependencies'},null,2)+'\n');
// Ship the licenses of packages actually included in the local browser bundle.
const roots=new Map();
for(const chunk of output.output)for(const id of Object.keys(chunk.modules||{})) {
  if(!id.includes('node_modules/'))continue;
  let dir=dirname(id);
  while(dir!==dirname(dir)) {
    try {const pkg=JSON.parse(await readFile(resolve(dir,'package.json'),'utf8'));if(pkg.name){roots.set(dir,pkg);break;}}catch{}
    dir=dirname(dir);
  }
}
const notices=[];
for(const [dir,pkg] of [...roots].sort((a,b)=>a[1].name.localeCompare(b[1].name))) {
  let license='';for(const name of ['LICENSE','LICENSE.txt','LICENSE.md','license','license.txt','COPYING'])try{license=await readFile(resolve(dir,name),'utf8');break;}catch{}
  notices.push(`${pkg.name} ${pkg.version} (${pkg.license||'see package'})\n${license||JSON.stringify(pkg.repository)||'See upstream package for license terms.'}`);
}
await writeFile(resolve(here,'THIRD-PARTY-NOTICES.txt'),notices.join('\n\n----------------------------------------\n\n'));
console.log('Built local deck.gl gallery with deck.gl '+version);
