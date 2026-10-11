/** Rendering-only BMOPF adapter. It does not solve or elaborate conductor internals. */
import {packArrays} from './fixture.mjs';
export const ADAPTER_VERSION = 2;
export const sourceKey = (source,family,id,kind='') => JSON.stringify([source,family,kind,id]);
export const busQuery = (source,id) => `${source}/${encodeURIComponent(id)}`;
const table = (value,where) => {
  if(value===undefined)return {};
  if(!value||typeof value!=='object'||Array.isArray(value))throw new Error(`${where}: expected keyed objects`);
  return value;
};
const entries = value => Object.entries(value).sort(([a],[b])=>a<b?-1:a>b?1:0);
function terminals(value,allowed,where){
  if(!Array.isArray(value)||!value.length||new Set(value).size!==value.length||value.some(t=>!allowed.includes(t)))throw new Error(`${where}: invalid or unknown terminals`);
  return [...value];
}
function disabled(row,where){
  if('status' in row && ![0,1,true,false].includes(row.status))throw new Error(`${where}: unsupported status`);
  if('enabled' in row && typeof row.enabled!=='boolean')throw new Error(`${where}: unsupported enabled value`);
  if('status' in row && 'enabled' in row && Boolean(row.status)!==row.enabled)throw new Error(`${where}: contradictory state`);
  return row.status===0||row.status===false||row.enabled===false;
}
export function adaptBMOPF(raw,source){
  for(const key of Object.keys(raw))if(!['bus','line','linecode','load','shunt','voltage_source','transformer','switch','meta','extras','name','terminal_conventions'].includes(key)&&raw[key]&&typeof raw[key]==='object'&&Object.keys(raw[key]).length)throw new Error(`Unsupported BMOPF table: ${key}`);
  const busTable=table(raw.bus,'bus'), buses=[],index=new Map();
  let lonMin=Infinity,latMin=Infinity,lonMax=-Infinity,latMax=-Infinity;
  for(const [id,b] of entries(busTable)){
    const lon=b.longitude,lat=b.latitude;
    const absent=lon===undefined&&lat===undefined;
    if(!(absent&&source.allowMissingCoordinates)&&(!Number.isFinite(lon)||!Number.isFinite(lat)||Math.abs(lon)>180||Math.abs(lat)>90))throw new Error(`bus ${id}: missing/invalid longitude or latitude`);
    if(!Array.isArray(b.terminal_names)||!b.terminal_names.length||new Set(b.terminal_names).size!==b.terminal_names.length||b.terminal_names.some(t=>typeof t!=='string'))throw new Error(`bus ${id}: invalid terminal_names`);
    const grounded=b.perfectly_grounded_terminals||[];
    if(!Array.isArray(grounded)||grounded.some(t=>!b.terminal_names.includes(t)))throw new Error(`bus ${id}: unknown grounded terminal`);
    index.set(id,buses.length);buses.push({id,source:0,terminals:[...b.terminal_names],grounded:[...grounded],coordinates:absent?null:[lon,lat]});
    if(!absent){lonMin=Math.min(lonMin,lon);lonMax=Math.max(lonMax,lon);latMin=Math.min(latMin,lat);latMax=Math.max(latMax,lat);}
  }
  if(!buses.length)throw new Error('No buses');
  const hasCoordinates=buses.every(b=>b.coordinates!==null);
  if(!hasCoordinates&&buses.some(b=>b.coordinates!==null))throw new Error('Partial coordinate coverage is unsupported');
  // Local equirectangular projection in metres, north up (canvas y down).
  const origin=hasCoordinates?[(lonMin+lonMax)/2,(latMin+latMax)/2]:[0,0],factor=6371000*Math.PI/180,cos=Math.cos(origin[1]*Math.PI/180);
  const positions=new Float32Array(buses.length*3);
  if(hasCoordinates)for(let i=0;i<buses.length;i++){positions[i*3]=(buses[i].coordinates[0]-origin[0])*cos*factor;positions[i*3+1]=-(buses[i].coordinates[1]-origin[1])*factor;}
  const branches=[],attachments=[];
  function bus(id,where){if(!index.has(id))throw new Error(`${where}: unknown bus ${id}`);return index.get(id);}
  function addBranch(id,row,family,kind=''){
    const where=`${family}/${kind}/${id}`,a=bus(row.bus_from,where),b=bus(row.bus_to,where);
    const from=terminals(row.terminal_map_from,buses[a].terminals,where),to=terminals(row.terminal_map_to,buses[b].terminals,where);
    if(family!=='transformer'&&from.length!==to.length)throw new Error(`${where}: unequal conductor maps`);
    const outOfService=disabled(row,where);
    if(family==='switch'&&typeof row.open_switch!=='boolean')throw new Error(`${where}: open_switch must be a scalar boolean`);
    branches.push({id,source:0,family,kind,from:a,to:b,terminalMapFrom:from,terminalMapTo:to,disabled:outOfService||(family==='switch'&&row.open_switch),...(family==='switch'?{outOfService,openSwitch:row.open_switch,sourceOpenSwitch:row.open_switch}:{}),meaning:family==='switch'?'switch_connection':family==='transformer'?'equipment_association':'conductor_bundle'});
  }
  for(const [id,row] of entries(table(raw.line,'line')))addBranch(id,row,'line');
  for(const [kind,rows] of entries(table(raw.transformer,'transformer'))){
    const family=table(rows,`transformer ${kind}`);
    if(!['delta_wye','single_phase','center_tap'].includes(kind)&&Object.keys(family).length)throw new Error(`Unsupported transformer family: ${kind}`);
    for(const [id,row] of entries(family))addBranch(id,row,'transformer',kind);
  }
  for(const [id,row] of entries(table(raw.switch,'switch')))addBranch(id,row,'switch');
  for(const family of ['load','shunt','voltage_source'])for(const [id,row] of entries(table(raw[family],family))){
    const n=bus(row.bus,`${family}/${id}`);
    attachments.push({id,source:0,family,bus:n,terminalMap:terminals(row.terminal_map,buses[n].terminals,`${family}/${id}`),configuration:row.configuration??null,disabled:disabled(row,`${family}/${id}`)});
  }
  return {positions,buses,branches,attachments,hasCoordinates,sources:[{...source,projection:hasCoordinates?{kind:'local_equirectangular',input:'longitude/latitude degrees',origin,units:'metres',yAxis:'south',placement:'source coordinates'}:{kind:'unavailable',placement:'topology only'}}]};
}
export function combineModels(models){
  if(models.some(m=>m.hasCoordinates===false))throw new Error('Coordinate-free collection panels require a schematic layout');
  if(!models.length)throw new Error('Empty collection');
  const out={positions:new Float32Array(models.reduce((n,m)=>n+m.buses.length,0)*3),buses:[],branches:[],attachments:[],sources:[]};
  const boxes=models.map(m=>bounds(m.positions)),columns=Math.ceil(Math.sqrt(models.length));
  const width=Math.max(...boxes.map(b=>b[2]-b[0]))+1000,height=Math.max(...boxes.map(b=>b[3]-b[1]))+1000;
  for(let s=0;s<models.length;s++){
    const m=models[s];if(m.sources.length!==1)throw new Error('Collection members must be individual sources');
    if(out.sources.some(x=>x.id===m.sources[0].id))throw new Error('Duplicate source namespace');
    const offset=out.buses.length,box=boxes[s],dx=(s%columns)*width-box[0],dy=Math.floor(s/columns)*height-box[1];
    for(let i=0;i<m.buses.length;i++){out.positions[(offset+i)*3]=m.positions[i*3]+dx;out.positions[(offset+i)*3+1]=m.positions[i*3+1]+dy;}
    out.buses.push(...m.buses.map(b=>({...b,source:s})));
    for(const b of m.branches)out.branches.push({...b,source:s,from:b.from+offset,to:b.to+offset});
    for(const a of m.attachments)out.attachments.push({...a,source:s,bus:a.bus+offset});
    out.sources.push({...m.sources[0],displayTranslation:[dx,dy],displayBounds:[box[0]+dx,box[1]+dy,box[2]+dx,box[3]+dy]});
  }
  return out;
}
export function bounds(positions){let x0=Infinity,y0=Infinity,x1=-Infinity,y1=-Infinity;for(let i=0;i<positions.length;i+=3){x0=Math.min(x0,positions[i]);y0=Math.min(y0,positions[i+1]);x1=Math.max(x1,positions[i]);y1=Math.max(y1,positions[i+1]);}return [x0-30,y0-30,x1+30,y1+30];}
export function referenceData(model){return {version:1,sources:model.sources,buses:model.buses,branches:model.branches,attachments:model.attachments,scenario:model.scenario??null};}
export function datasetMetadata(model,id,label){
  const switches=model.branches.filter(b=>b.family==='switch');
  return {version:1,kind:'bmopf',adapterVersion:ADAPTER_VERSION,id,label,count:model.buses.length,branches:model.branches.length,
    lines:model.branches.filter(b=>b.family==='line').length,transformers:model.branches.filter(b=>b.family==='transformer').length,
    switches:switches.length,openSwitches:switches.filter(b=>b.openSwitch).length,closedSwitches:switches.filter(b=>!b.openSwitch).length,
    outOfServiceSwitches:switches.filter(b=>b.outOfService).length,attachments:model.attachments.length,regions:model.sources.length,
    openTies:model.branches.filter(b=>b.disabled).length,sourceCoordinatesAvailable:model.hasCoordinates!==false,scenario:model.scenario??null,
    sources:model.sources,file:`${id}.bin`,refsFile:`${id}.refs.json`};
}
export function toFixture(model,id,label){
  if(model.hasCoordinates===false)throw new Error('Source coordinates unavailable; use a schematic layout');
  const endpoints=new Uint32Array(model.branches.length*2),states=new Uint8Array(model.branches.length),ss=[],tt=[],owners=[];
  let coincidentBranches=0;
  function segment(a,b,owner){ss.push(...a);tt.push(...b);owners.push(owner);}
  model.branches.forEach((b,i)=>{
    endpoints.set([b.from,b.to],i*2);states[i]=b.disabled?1:0;
    const a=Array.from(model.positions.subarray(b.from*3,b.from*3+3)),z=Array.from(model.positions.subarray(b.to*3,b.to*3+3));
    const same=a[0]===z[0]&&a[1]===z[1];b.coincident=same;
    if(same){
      coincidentBranches++;
      // Display-only detour keeps co-located equipment selectable, never a new bus.
      const p=[a[0]+8,a[1]-8,0],q=[a[0]+16,a[1],0];segment(a,p,i);
      if(b.disabled){segment(p,[a[0]+11,a[1]-5,0],i);segment([a[0]+13,a[1]-3,0],q,i);}else segment(p,q,i);
      segment(q,z,i);
    }else if(b.disabled){const mid=t=>a.map((v,k)=>v+(z[k]-v)*t);segment(a,mid(.46),i);segment(mid(.54),z,i);}
    else segment(a,z,i);
  });
  const {buffer,sections}=packArrays({positions:model.positions,endpoints,states,segmentSources:new Float32Array(ss),segmentTargets:new Float32Array(tt),segmentBranches:new Uint32Array(owners)});
  const sceneBounds=bounds(model.positions),locationSet=new Set();for(let i=0;i<model.buses.length;i++)locationSet.add(`${model.positions[i*3]},${model.positions[i*3+1]}`);
  const refs=referenceData(model);
  const manifest={...datasetMetadata(model,id,label),segments:owners.length,bounds:sceneBounds,bytes:buffer.byteLength,sections,
    coincidentBuses:model.buses.length-locationSet.size,coincidentBranches,coordinateSpace:'projected geographic',
    placement:model.sources.length>1?'independent panels; no added connections':'source coordinates'};
  return {manifest,buffer,refs};
}
