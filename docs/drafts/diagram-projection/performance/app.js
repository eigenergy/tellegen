import {Deck,OrthographicView,COORDINATE_SYSTEM,VERSION} from '@deck.gl/core';
import {LineLayer,ScatterplotLayer,TextLayer} from '@deck.gl/layers';
import {decodeFixture} from './fixture.mjs';
import {busQuery,sourceKey} from './bmopf-adapter.mjs';
import {validateReferences} from './bmopf-references.mjs';

const $=id=>document.getElementById(id), fmt=n=>n.toLocaleString(), ms=n=>`${n.toFixed(1)} ms`;
const busName=i=>fixture?.refs?.buses[i].id??('B'+String(i+1).padStart(6,'0'));
const labelName=i=>busName(i).length>12?busName(i).slice(0,11)+'…':busName(i);
const escape=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const locationKey=(f,i)=>`${f.positions[i*3]},${f.positions[i*3+1]}`;
const identity=m=>({id:m.id,kind:m.kind??'synthetic',count:m.count,sha256:m.sha256,refsSha256:m.refsSha256??null,adapterVersion:m.adapterVersion??null,sources:m.sources??[],placement:m.placement??'synthetic schematic',stagingMs:m.stagingMs??null});
const palette={dark:{line:[108,137,150,255],node:[198,207,203,255],open:[229,170,84,255],label:[238,234,225,255],bg:[23/255,27/255,32/255,1]},paper:{line:[108,125,127,255],node:[48,69,78,255],open:[178,94,0,255],label:[32,36,43,255],bg:[252/255,251/255,247/255,1]}};
let theme='dark', fixture=null, entries=[], deck, ready=false, loading=false, request=0, abort;
let camera={target:[0,0,0],zoom:0,minZoom:-10,maxZoom:5}, selection=null, labelData=[], run=null;
let pendingLoad=null,pendingSelection=null,labelTimer=0,selectionMetric=null;
const report={version:1,started:new Date().toISOString(),renderer:`deck.gl ${VERSION} / WebGL2 / OrthographicView`,
  methodology:'Precomputed synthetic or BMOPF geometry; identity and placement recorded per dataset. No CPU culling or aggregation. Frame intervals from deck.gl onAfterRender; not GPU timer queries. First-draw callback is not a GPU completion fence.',
  environment:{userAgent:navigator.userAgent,hardwareConcurrency:navigator.hardwareConcurrency,deviceMemoryGiB:navigator.deviceMemory??null},loads:[],runs:[]};
function error(message){$('status').hidden=false;$('status').textContent=message;loading=false;pendingLoad=null;controls();}
function controls(){
  for(const el of document.querySelectorAll('button[data-theme], #labels, #edge-picking, #fit, #detail, #benchmark, #bus-id, #search button, #clear, #next-colocated'))el.disabled=loading||!!run||!ready||!fixture;
  for(const el of document.querySelectorAll('[data-size], #dataset'))el.disabled=loading||!!run||!ready;
  $('stop').disabled=!run;
}
function environment(){const r=$('canvas-wrap').getBoundingClientRect();return {width:Math.round(r.width),height:Math.round(r.height),devicePixelRatio:devicePixelRatio,renderPixelRatio:Math.min(devicePixelRatio,2),labels:$('labels').checked,wirePicking:$('edge-picking').checked,theme};}
function fitCamera(){const b=fixture.manifest.bounds,r=$('canvas-wrap').getBoundingClientRect();return {...camera,target:[(b[0]+b[2])/2,(b[1]+b[3])/2,0],zoom:Math.log2(Math.min((r.width-70)/(b[2]-b[0]),(r.height-95)/(b[3]-b[1])))};}
function detailCamera(){const m=fixture.manifest,r=Math.floor(m.regions/2),i=m.kind==='bmopf'?Math.floor(m.count/2):r*m.regionSize+2;return {...camera,target:[fixture.positions[i*3]+200,fixture.positions[i*3+1],0],zoom:0.6};}
function setCamera(next,updateLabels=true){camera={...next,minZoom:-10,maxZoom:5};deck.setProps({viewState:camera});$('zoom-readout').textContent=`${(2**camera.zoom).toFixed(2)}× · PRECOMPUTED`;if(updateLabels)scheduleLabels();}
function prepare(f){
  const m=f.manifest;
  f.nodeData={length:m.count,attributes:{getPosition:{value:f.positions,size:3}}};
  f.segmentColors=new Uint8Array(m.segments*4); f.nodeColors=new Uint8Array(m.count*4);
  f.lineData={length:m.segments,attributes:{getSourcePosition:{value:f.segmentSources,size:3},getTargetPosition:{value:f.segmentTargets,size:3},getColor:{value:f.segmentColors,size:4,normalized:true}}};
  f.nodeData.attributes.getFillColor={value:f.nodeColors,size:4,normalized:true};
  // CSR adjacency keeps selection proportional to local degree, not network size.
  f.offsets=new Uint32Array(m.count+1);
  for(const n of f.endpoints)f.offsets[n+1]++;
  for(let i=1;i<f.offsets.length;i++)f.offsets[i]+=f.offsets[i-1];
  f.incidents=new Uint32Array(m.branches*2);const cursors=f.offsets.slice();
  for(let b=0;b<m.branches;b++){f.incidents[cursors[f.endpoints[b*2]]++]=b;f.incidents[cursors[f.endpoints[b*2+1]]++]=b;}
  f.firstSegment=new Uint32Array(m.branches+1);let b=0;
  for(let s=0;s<m.segments;s++){while(b<=f.segmentBranches[s])f.firstSegment[b++]=s;}f.firstSegment[m.branches]=m.segments;
  // Fixed-size spatial buckets for label candidates only; base geometry stays resident.
  f.cells=new Map();
  for(let i=0;i<m.count;i++){const key=`${Math.floor(f.positions[i*3]/128)},${Math.floor(f.positions[i*3+1]/128)}`;if(!f.cells.has(key))f.cells.set(key,[]);f.cells.get(key).push(i);}
  f.ownedBytes=f.buffer.byteLength+f.segmentColors.byteLength+f.nodeColors.byteLength+f.offsets.byteLength+f.incidents.byteLength+f.firstSegment.byteLength;
  if(f.refs){
    f.rawBusIds=new Map();f.qualifiedBusIds=new Map();f.locations=new Map();f.attachmentsByBus=new Map();
    f.refs.buses.forEach((b,i)=>{const q=busQuery(f.refs.sources[b.source].id,b.id);f.qualifiedBusIds.set(q,i);
      if(!f.rawBusIds.has(b.id))f.rawBusIds.set(b.id,[]);f.rawBusIds.get(b.id).push(i);
      const k=locationKey(f,i);if(!f.locations.has(k))f.locations.set(k,[]);f.locations.get(k).push(i);
    });
    for(const a of f.refs.attachments){if(!f.attachmentsByBus.has(a.bus))f.attachmentsByBus.set(a.bus,[]);f.attachmentsByBus.get(a.bus).push(a);}
  }
  colour(f);
}
function colour(f){const p=palette[theme];for(let s=0;s<f.manifest.segments;s++){const b=f.segmentBranches[s];f.segmentColors.set(f.states[b]?p.open:f.refs?.branches[b].family==='transformer'?[141,124,209,255]:p.line,s*4);}for(let i=0;i<f.manifest.count;i++)f.nodeColors.set(!f.refs&&i%f.manifest.regionSize===0?p.open:p.node,i*4);}

function selectedSegments(){
  if(!selection)return [];
  const branches=selection.kind==='branch'?[selection.index]:fixture.incidents.subarray(fixture.offsets[selection.index],fixture.offsets[selection.index+1]);
  const segments=[];
  for(const b of branches)for(let s=fixture.firstSegment[b];s<fixture.firstSegment[b+1];s++)segments.push({source:fixture.segmentSources.subarray(s*3,s*3+3),target:fixture.segmentTargets.subarray(s*3,s*3+3)});
  return segments;
}
function layers(){
  if(!fixture)return [];
  const p=palette[theme],common={coordinateSystem:COORDINATE_SYSTEM.CARTESIAN,parameters:{depthCompare:'always'}};
  const nodeSelection=selection?.kind==='bus'?[{position:fixture.positions.subarray(selection.index*3,selection.index*3+3)}]:[];
  const panels=(fixture.refs?.sources??[]).filter(s=>s.displayBounds);
  return [
    new TextLayer({...common,id:`panel-labels-${fixture.manifest.id}-${theme}`,data:panels,getPosition:s=>[s.displayBounds[0],s.displayBounds[1]-150,0],getText:s=>s.label,getColor:p.label,getSize:13,getTextAnchor:'start',getAlignmentBaseline:'bottom',fontFamily:'Plex, monospace',characterSet:'auto',pickable:false}),
    new LineLayer({...common,id:`branches-${fixture.manifest.id}-${fixture.manifest.sha256}-${theme}`,data:fixture.lineData,getWidth:1,widthUnits:'pixels',pickable:$('edge-picking').checked,autoHighlight:false}),
    new ScatterplotLayer({...common,id:`buses-${fixture.manifest.id}-${fixture.manifest.sha256}-${theme}`,data:fixture.nodeData,getRadius:1.9,radiusUnits:'common',radiusMinPixels:.65,radiusMaxPixels:4,pickable:true,stroked:false}),
    new LineLayer({...common,id:'selection-lines',data:selectedSegments(),getSourcePosition:d=>d.source,getTargetPosition:d=>d.target,getColor:p.open,getWidth:2.5,pickable:false}),
    new ScatterplotLayer({...common,id:'selection-bus',data:nodeSelection,getPosition:d=>d.position,getRadius:6,radiusUnits:'pixels',getFillColor:p.open,pickable:false}),
    new TextLayer({...common,id:'labels',data:labelData,getPosition:d=>d.position,getText:d=>d.text,getColor:p.label,getSize:12,sizeUnits:'pixels',fontFamily:'Plex, monospace',characterSet:'auto',fontSettings:{sdf:true},getTextAnchor:'start',getAlignmentBaseline:'bottom',getPixelOffset:[5,-5],pickable:false})
  ];
}
function render(){deck.setProps({layers:layers(),parameters:{clearColor:palette[theme].bg}});}
function scheduleLabels(){clearTimeout(labelTimer);labelTimer=setTimeout(updateLabels,90);}
function updateLabels(){
  if(!fixture)return;labelData=[];
  // Selection ID remains discoverable even when automatic labels are off.
  const reserved=selection?.kind==='bus'?selection.index:null;
  if(reserved!==null)labelData.push({position:fixture.positions.subarray(reserved*3,reserved*3+3),text:labelName(reserved)});
  if($('labels').checked && camera.zoom>=-0.2){
    const r=$('canvas-wrap').getBoundingClientRect(),scale=2**camera.zoom;
    const x0=camera.target[0]-r.width/2/scale,y0=camera.target[1]-r.height/2/scale,x1=x0+r.width/scale,y1=y0+r.height/scale;
    const occupied=new Set();
    function reserve(px,py,force=false){
      const sx=(px-x0)*scale+5,sy=(py-y0)*scale-20;
      if(!force&&(sx<0||sx+82>r.width||sy<45||sy+20>r.height-30))return false;
      const keys=[];for(let gy=Math.floor(sy/22);gy<=Math.floor((sy+18)/22);gy++)for(let gx=Math.floor(sx/90);gx<=Math.floor((sx+82)/90);gx++)keys.push(`${gx},${gy}`);
      if(!force&&keys.some(k=>occupied.has(k)))return false;
      keys.forEach(k=>occupied.add(k));return true;
    }
    if(reserved!==null)reserve(fixture.positions[reserved*3],fixture.positions[reserved*3+1],true);
    for(let y=Math.floor(y0/128);y<=Math.floor(y1/128)&&labelData.length<200;y++)for(let x=Math.floor(x0/128);x<=Math.floor(x1/128)&&labelData.length<200;x++)for(const i of fixture.cells.get(`${x},${y}`)||[]){
      const px=fixture.positions[i*3],py=fixture.positions[i*3+1];
      if(i===reserved||px<x0||px>x1||py<y0||py>y1)continue;
      if(!reserve(px,py))continue;labelData.push({position:[px,py,0],text:labelName(i)});if(labelData.length>=200)break;
    }
  }
  $('draw-count').textContent=`${fmt(fixture.manifest.count)} buses · ${fmt(fixture.manifest.segments)} segments submitted · ${labelData.length} labels`;
  render();
}
function select(kind,index,started=performance.now()){
  if(run||loading)return;selection={kind,index};pendingSelection=started;
  $('next-colocated').hidden=true;
  if(fixture.refs){
    const refs=fixture.refs,row=kind==='bus'?refs.buses[index]:refs.branches[index],source=refs.sources[row.source];
    let content=`<h2>${escape(row.id)}</h2><p>${escape(source.label)} · ${escape(kind==='bus'?'bus':row.family+(row.kind?' / '+row.kind:''))}</p>`;
    content+=`<p class="fine">Source identity: <code>${escape(sourceKey(source.id,kind==='bus'?'bus':row.family,row.id,row.kind??''))}</code></p>`;
    if(kind==='bus'){
      const members=fixture.locations.get(locationKey(fixture,index));
      content+=`<p>${fixture.offsets[index+1]-fixture.offsets[index]} incident branches · terminals ${escape(row.terminals.join(', '))}<br>Grounded: ${escape(row.grounded.join(', ')||'none declared')}<br>Longitude / latitude: ${row.coordinates.join(', ')}<br>${members.length} buses at this display position</p><p class="fine">Find: <code>${escape(busQuery(source.id,row.id))}</code></p>`;
      const attachments=fixture.attachmentsByBus.get(index)??[];
      content+=`<details><summary>${attachments.length} source attachments</summary>${attachments.slice(0,100).map(a=>`<p>${escape(a.family)} / ${escape(a.id)} · terminals ${escape(a.terminalMap.join(', '))}${a.disabled?' · disabled':''}</p>`).join('')}${attachments.length>100?'<p>First 100 shown; all retained in reference data.</p>':''}</details>`;
      $('next-colocated').hidden=members.length<2;
    }else content+=`<p>${escape(busName(row.from))} → ${escape(busName(row.to))}<br>Terminal maps: ${escape(row.terminalMapFrom.join(', '))} → ${escape(row.terminalMapTo.join(', '))}<br>${row.disabled?'Disabled · display gap':'Enabled'}${row.coincident?' · coincident endpoints, display detour':''}</p><p>${row.family==='transformer'?'Equipment association; does not imply interwinding conductor continuity.':'Conductor bundle; terminal order preserved.'}</p>`;
    $('selection').innerHTML=content;
  }else if(kind==='bus'){
    const degree=fixture.offsets[index+1]-fixture.offsets[index];
    $('selection').innerHTML=`<h2>${busName(index)}</h2><p>Region ${Math.floor(index/fixture.manifest.regionSize)+1} · ${degree} incident branches<br>Stable synthetic source: bus/${index+1}</p>`;
  }else{
    const a=fixture.endpoints[index*2],b=fixture.endpoints[index*2+1];
    $('selection').innerHTML=`<h2>Branch ${index+1}</h2><p>${busName(a)} → ${busName(b)}<br>${fixture.states[index]?'Open tie · explicit gap':'Closed branch'} · source branch/${index+1}</p>`;
  }
  updateLabels();
}
async function load(id){
  stopRun('dataset changed');const token=++request;abort?.abort();abort=new AbortController();loading=true;pendingLoad=null;selection=null;labelData=[];pendingSelection=null;selectionMetric=null;clearTimeout(labelTimer);controls();
  fixture=null;render();$('next-colocated').hidden=true;$('search-error').textContent='';
  $('test-status').textContent='Ready';$('status').hidden=false;$('status').textContent='Loading dataset…';$('frame-time').textContent='Not measured';$('frame-detail').textContent='Run the six-second camera test';$('selection').innerHTML='<h2>Follow a connection.</h2><p>Select a bus or find it by ID.</p>';$('selection-time').textContent='Selection feedback: not measured';
  for(const b of document.querySelectorAll('[data-size]'))b.setAttribute('aria-pressed',String(`synthetic-${b.dataset.size}`===id));
  $('dataset').value=id;const start=performance.now();
  try{
    const m=entries.find(m=>m.id===id);if(!m)throw new Error('Dataset unavailable; stage the external sources first');
    const base=m.kind==='bmopf'?'local-data/':'fixtures/';
    async function fetchBytes(file){const r=await fetch(base+file,{signal:abort.signal});if(!r.ok)throw new Error(`Dataset HTTP ${r.status}; regenerate or stage fixtures`);return r.arrayBuffer();}
    const [buffer,refBuffer]=await Promise.all([fetchBytes(m.file),m.refsFile?fetchBytes(m.refsFile):null]);
    const fetched=performance.now();if(token!==request)return;
    async function verify(bytes,expected){const digest=await crypto.subtle.digest('SHA-256',bytes);if(Array.from(new Uint8Array(digest),x=>x.toString(16).padStart(2,'0')).join('')!==expected)throw new Error('Dataset hash mismatch; finish staging and reload');}
    await verify(buffer,m.sha256);if(refBuffer)await verify(refBuffer,m.refsSha256);if(token!==request)return;
    const next=decodeFixture(m,buffer);
    if(refBuffer){next.refs=JSON.parse(new TextDecoder().decode(refBuffer));validateReferences(next);}
    const decoded=performance.now();prepare(next);const prepared=performance.now();
    fixture=next;pendingLoad={...identity(m),bytes:m.bytes,refsBytes:m.refsBytes??0,start,fetchMs:fetched-start,decodeAndValidateMs:decoded-fetched,prepareMs:prepared-decoded,submittedAt:prepared,environment:environment()};
    $('network-count').textContent=fmt(m.count);$('network-detail').textContent=m.kind==='bmopf'?`${fmt(m.lines)} lines · ${fmt(m.transformers)} transformer objects`:`${fmt(m.branches)} branches · ${fmt(m.openTies)} open ties`;
    $('memory').textContent=`${(fixture.ownedBytes/1048576).toFixed(2)} MiB`;$('memory-detail').textContent=`${(m.bytes/1048576).toFixed(2)} MiB geometry + ${((m.refsBytes??0)/1048576).toFixed(2)} MiB reference JSON; excludes JS objects`;
    $('canvas-title').textContent=m.label;$('source-detail').textContent=m.kind==='bmopf'?`${m.placement}. Projected geographic coordinates in metres; no automatic schematic layout. ${fmt(m.coincidentBuses)} additional co-located buses. `+m.sources.map(s=>s.label+': '+s.note).join(' '):'Synthetic schematic geometry; no electrical study data.';
    $('bus-id').value=next.refs?busQuery(next.refs.sources[0].id,next.refs.buses[0].id):'B000501';$('bus-id').placeholder=next.refs?'Exact source ID or source/id':'B000001';
    const url=new URL(location.href);url.searchParams.delete('size');url.searchParams.set('dataset',id);history.replaceState(null,'',url);
    setCamera(fitCamera(),false);updateLabels();
  }catch(e){if(e.name!=='AbortError'&&token===request)error(e.message);}
}

function afterRender(){
  const now=performance.now();
  if(pendingLoad){
    const m=pendingLoad;pendingLoad=null;const result={...m,firstDrawMs:now-m.start,submitToDrawMs:now-m.submittedAt};delete result.start;delete result.submittedAt;report.loads.push(result);
    $('load-time').textContent=ms(result.firstDrawMs);$('load-detail').textContent=`Fetch ${ms(m.fetchMs)} · decode/validate ${ms(m.decodeAndValidateMs)} · prepare ${ms(m.prepareMs)} · submit → draw ${ms(result.submitToDrawMs)}`;
    $('status').hidden=true;loading=false;controls();
  }
  if(pendingSelection!==null){selectionMetric=now-pendingSelection;pendingSelection=null;$('selection-time').textContent=`Selection → draw callback: ${ms(selectionMetric)}`;}
  if(run){if(run.last!==null)run.intervals.push(now-run.last);run.last=now;}
}
function percentile(values,p){const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.min(sorted.length-1,Math.floor((sorted.length-1)*p))]??null;}
function stopRun(reason='cancelled'){
  if(!run)return;const r=run;run=null;cancelAnimationFrame(r.raf);
  const valid=reason==='complete'&&r.intervals.length>=30;
  const result={...r.dataset,started:r.started,durationMs:performance.now()-r.start,complete:valid,reason,environment:r.environment,frames:r.intervals.length,intervalsMs:r.intervals,
    p50Ms:valid?percentile(r.intervals,.5):null,p95Ms:valid?percentile(r.intervals,.95):null,maxMs:valid?Math.max(...r.intervals):null,selectionFeedbackMs:selectionMetric};
  report.runs.push(result);$('test-status').textContent=valid?'Complete':`Stopped: ${reason}`;
  if(valid){$('frame-time').textContent=ms(result.p95Ms);$('frame-detail').textContent=`p50 ${ms(result.p50Ms)} · max ${ms(result.maxMs)} · ${result.frames} intervals`;}
  if(!valid){$('frame-time').textContent='Not measured';$('frame-detail').textContent=`Incomplete: ${reason}`;}
  controls();setCamera(r.original);updateLabels();
}
function benchmark(){
  if(!fixture||loading||run||document.hidden)return;
  const overview=fitCamera(),detail=detailCamera();
  run={dataset:identity(fixture.manifest),start:performance.now(),started:new Date().toISOString(),last:null,intervals:[],original:{...camera,target:[...camera.target]},environment:environment(),raf:0};controls();$('frame-time').textContent='Running…';$('frame-detail').textContent='Measuring camera frame intervals';
  function tick(now){if(!run)return;const t=(now-run.start)/6000;if(t>=1){stopRun('complete');return;}
    // overview → local detail → overview, with a small continuous pan
    const mix=(1-Math.cos(t*Math.PI*2))/2;
    setCamera({...overview,zoom:overview.zoom+(detail.zoom-overview.zoom)*mix,target:[overview.target[0]+(detail.target[0]-overview.target[0])*mix+Math.sin(t*4*Math.PI)*60,overview.target[1]+(detail.target[1]-overview.target[1])*mix,0]},false);
    if($('labels').checked)updateLabels();
    $('test-status').textContent=`Running · ${Math.round(t*100)}%`;run.raf=requestAnimationFrame(tick);
  }run.raf=requestAnimationFrame(tick);
}
function exportReport(){const data={...report,exported:new Date().toISOString(),current:fixture?{...identity(fixture.manifest),ownedCpuArrayBytes:fixture.ownedBytes,environment:environment()}:null};
  const url=URL.createObjectURL(new Blob([JSON.stringify(data,null,2)],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download='tellegen-diagram-performance.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}

for(const b of document.querySelectorAll('[data-size]'))b.addEventListener('click',()=>load(`synthetic-${b.dataset.size}`));
for(const b of document.querySelectorAll('button[data-theme]'))b.addEventListener('click',()=>{theme=b.dataset.theme;document.documentElement.dataset.theme=theme;for(const x of document.querySelectorAll('button[data-theme]'))if(x.tagName==='BUTTON')x.setAttribute('aria-pressed',String(x.dataset.theme===theme));colour(fixture);
  // Theme changes retire base layers in this baseline; production should patch colours.
  fixture.lineData={...fixture.lineData,attributes:{...fixture.lineData.attributes}};fixture.nodeData={...fixture.nodeData,attributes:{...fixture.nodeData.attributes}};render();});
$('dataset').addEventListener('change',()=>load($('dataset').value));
$('next-colocated').addEventListener('click',()=>{if(selection?.kind!=='bus')return;const members=fixture.locations.get(locationKey(fixture,selection.index));select('bus',members[(members.indexOf(selection.index)+1)%members.length]);});
$('labels').addEventListener('change',updateLabels);$('edge-picking').addEventListener('change',render);
$('fit').addEventListener('click',()=>setCamera(fitCamera()));$('detail').addEventListener('click',()=>setCamera(detailCamera()));
$('clear').addEventListener('click',()=>{selection=null;$('next-colocated').hidden=true;$('selection').innerHTML='<h2>Follow a connection.</h2><p>Select a bus or find it by ID.</p>';updateLabels();});
$('search').addEventListener('submit',e=>{e.preventDefault();if(!fixture||run||loading)return;const started=performance.now(),value=$('bus-id').value.trim();$('search-error').textContent='';let i;
  if(fixture.refs){
    const raw=fixture.rawBusIds.get(value)??[];
    if(raw.length===1)i=raw[0];else if(raw.length>1){$('search-error').textContent='Ambiguous source ID. Use '+raw.slice(0,5).map(n=>busQuery(fixture.refs.sources[fixture.refs.buses[n].source].id,value)).join(' or ');return;}else i=fixture.qualifiedBusIds.get(value);
    if(i===undefined){$('search-error').textContent='No exact source bus ID found. Use source/id to qualify an ID.';return;}
  }else{const digits=value.replace(/^B/i,''),id=Number(digits);if(!/^\d+$/.test(digits)||!Number.isSafeInteger(id)||id<1||id>fixture.manifest.count){$('search-error').textContent=`Enter a bus from B000001 to ${busName(fixture.manifest.count-1)}.`;return;}i=id-1;}
  select('bus',i,started);setCamera({...camera,target:[fixture.positions[i*3],fixture.positions[i*3+1],0],zoom:1.5});});
$('benchmark').addEventListener('click',benchmark);$('stop').addEventListener('click',()=>stopRun());$('export-report').addEventListener('click',exportReport);
document.addEventListener('visibilitychange',()=>{if(document.hidden)stopRun('tab hidden');});
window.addEventListener('resize',()=>{stopRun('viewport resized');scheduleLabels();});
window.addEventListener('pagehide',()=>{abort?.abort();clearTimeout(labelTimer);deck?.finalize();});
$('network-canvas').addEventListener('pointerdown',()=>{if(run)stopRun('user interaction');});
async function start(){
  controls();const res=await fetch('fixtures/manifest.json');if(!res.ok)throw new Error('Missing fixtures: run generate.mjs');entries=(await res.json()).map(m=>({...m,id:`synthetic-${m.count}`,label:`Synthetic · ${fmt(m.count)} buses`}));
  try{const local=await fetch('local-data/manifest.json');if(local.ok){const catalog=await local.json();entries.push(...catalog);$('dataset-note').textContent='Local BMOPF sources staged · raw files remain external';}else if(local.status!==404)throw new Error(`HTTP ${local.status}`);}catch(e){$('dataset-note').textContent='Local catalog unavailable: '+e.message;}
  for(const m of entries){const option=document.createElement('option');option.value=m.id;option.textContent=m.label;$('dataset').append(option);}

  // Font availability is resolved before TextLayer creates its glyph atlas.
  await document.fonts.load('12px Plex');
  deck=new Deck({canvas:$('network-canvas'),parent:$('canvas-wrap'),width:'100%',height:'100%',deviceProps:{type:'webgl'},useDevicePixels:Math.min(devicePixelRatio,2),
    views:new OrthographicView({id:'schematic',flipY:true}),viewState:camera,controller:{doubleClickZoom:true},layers:[],pickingRadius:5,
    onViewStateChange:({viewState})=>{if(run)stopRun('user interaction');setCamera(viewState);},
    onClick:info=>{if(info.index<0||!fixture||loading||run)return;if(info.layer?.id.startsWith('buses-'))select('bus',info.index);else if(info.layer?.id.startsWith('branches-'))select('branch',fixture.segmentBranches[info.index]);},
    getCursor:({isDragging})=>isDragging?'grabbing':'crosshair',onAfterRender:afterRender,onError:e=>{stopRun('renderer error');error('Renderer error: '+e.message);},
    onLoad:()=>{ready=true;const params=new URLSearchParams(location.search),requested=params.get('dataset')??`synthetic-${params.get('size')??1000}`;load(entries.some(m=>m.id===requested)?requested:'synthetic-1000');}});
}
start().catch(e=>error(e.message));
