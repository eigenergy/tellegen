/** Read external BMOPF files; publish only derived rendering geometry and references. */
import {readFile,writeFile,mkdir,rename,rm} from 'node:fs/promises';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {adaptBMOPF,combineModels,toFixture,datasetMetadata,referenceData,ADAPTER_VERSION} from './bmopf-adapter.mjs';
import {parseSwitchCommands,applySwitchScenario} from './switch-scenarios.mjs';
import {makeSchematic,validateSchematic} from './schematic.mjs';
import {decodeFixture} from './fixture.mjs';
const here=dirname(fileURLToPath(import.meta.url)),root=process.argv[2];
if(!root)throw new Error('Usage: node stage-bmopf.mjs /absolute/path/to/BMOPFDraftData/output');
const config=JSON.parse(await readFile(join(here,'bmopf-sources.json'),'utf8'));
const out=join(here,'local-data'),pending=join(here,`.bmopf-stage-${process.pid}`),models=new Map(),catalog=[];
const hash=data=>createHash('sha256').update(data).digest('hex');
const inventory={version:1,adapterVersion:ADAPTER_VERSION,sources:[],collections:[]};
await mkdir(pending,{recursive:true});
try{
  async function emit(model,id,label,preparationStart){
    const base=model.hasCoordinates===false?makeSchematic(model,{strategy:'tidy-tree'}):toFixture(model,id,label);
    const {buffer}=base,manifest={...base.manifest,...datasetMetadata(model,id,label)},refs=referenceData(model);
    decodeFixture(manifest,buffer);
    for(const b of refs.branches)if(refs.buses[b.from].source!==b.source||refs.buses[b.to].source!==b.source)throw new Error('Cross-source connection');
    const json=JSON.stringify(refs);
    Object.assign(manifest,{sha256:hash(new Uint8Array(buffer)),refsSha256:hash(json),refsBytes:Buffer.byteLength(json),stagingMs:performance.now()-preparationStart});
    await writeFile(join(pending,manifest.file),new Uint8Array(buffer));
    await writeFile(join(pending,manifest.refsFile),json);
    manifest.layouts={};
    for(const [mode,strategy] of [['schematic','leaf-order'],['tidy-tree','tidy-tree']])for(const routing of ['current','corridor','crossing-aware','equipment']){
      const key=mode+(routing==='current'?'':'-'+routing);
      const result=makeSchematic(model,{strategy,routing});
      Object.assign(result.manifest,{file:`${id}.${key}.bin`,sha256:hash(new Uint8Array(result.buffer))});
      validateSchematic(decodeFixture(result.manifest,result.buffer));
      await writeFile(join(pending,result.manifest.file),new Uint8Array(result.buffer));
      manifest.layouts[key]=result.manifest;
      console.log(`${id} / ${key}: ${manifest.count} buses; ${result.manifest.layout.totalMs.toFixed(1)} ms / ${result.manifest.segments} segments`);
    }
    catalog.push(manifest);
    return manifest;
  }
  for(const source of config.sources){
    const start=performance.now(),path=resolve(root,source.file);
    if(!path.startsWith(resolve(root)+'/'))throw new Error('Source path leaves external root');
    const bytes=await readFile(path),sha256=hash(bytes),model=adaptBMOPF(JSON.parse(bytes),{...source,sha256,bytes:bytes.length});
    let m;
    if(source.scenarios){
      const commandPath=resolve(dirname(resolve(root)),source.switchCommands);
      if(!commandPath.startsWith(dirname(resolve(root))+'/'))throw new Error('Command path leaves external repository');
      const commandBytes=await readFile(commandPath),commands=parseSwitchCommands(commandBytes.toString(),model);
      for(const kind of source.scenarios){
        const scenario=applySwitchScenario(model,kind,commands,{commandFile:source.switchCommands,commandSha256:hash(commandBytes)});
        m=await emit(scenario,`${source.id}-${kind}`,`${source.label} · ${scenario.scenario.label}`,start);
      }
    }else m=await emit(model,source.id,source.label,start);
    inventory.sources.push({id:source.id,file:source.file,sha256,bytes:bytes.length,buses:m.count,lines:m.lines,transformerObjects:m.transformers,switches:m.switches,attachments:m.attachments});
    if(config.collections.some(c=>c.members.includes(source.id)))models.set(source.id,model);
  }
  for(const collection of config.collections){
    const start=performance.now(),model=combineModels(collection.members.map(id=>{if(!models.has(id))throw new Error(`Missing member ${id}`);return models.get(id);}));
    const m=await emit(model,collection.id,collection.label,start);
    inventory.collections.push({id:collection.id,members:collection.members,buses:m.count,lines:m.lines,transformerObjects:m.transformers,switches:m.switches,attachments:m.attachments});
  }
  await writeFile(join(pending,'manifest.json'),JSON.stringify(catalog,null,2)+'\n');
  // All sources must succeed before publishing. The manifest is replaced last.
  await mkdir(out,{recursive:true});
  for(const m of catalog)for(const file of [m.file,m.refsFile,...Object.values(m.layouts).map(l=>l.file)])await rename(join(pending,file),join(out,file));
  await rename(join(pending,'manifest.json'),join(out,'manifest.json'));
  await writeFile(join(here,'bmopf-inventory.json'),JSON.stringify(inventory,null,2)+'\n');
}finally{await rm(pending,{recursive:true,force:true});}
