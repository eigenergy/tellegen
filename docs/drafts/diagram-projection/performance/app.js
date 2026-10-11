import {Deck,OrthographicView,COORDINATE_SYSTEM,VERSION} from '@deck.gl/core';
import {LineLayer,ScatterplotLayer,TextLayer} from '@deck.gl/layers';
import {decodeFixture} from './fixture.mjs';

const $=id=>document.getElementById(id), fmt=n=>n.toLocaleString(), ms=n=>`${n.toFixed(1)} ms`;
const busName=i=>'B'+String(i+1).padStart(6,'0');
const palette={dark:{line:[108,137,150,255],node:[198,207,203,255],open:[229,170,84,255],label:[238,234,225,255],bg:[23/255,27/255,32/255,1]},paper:{line:[108,125,127,255],node:[48,69,78,255],open:[178,94,0,255],label:[32,36,43,255],bg:[252/255,251/255,247/255,1]}};
let theme='dark', fixture=null, entries=[], deck, ready=false, loading=false, request=0, abort;
let camera={target:[0,0,0],zoom:0,minZoom:-10,maxZoom:5}, selection=null, labelData=[], run=null;
let pendingLoad=null,pendingSelection=null,labelTimer=0,selectionMetric=null;
const report={version:1,started:new Date().toISOString(),renderer:`deck.gl ${VERSION} / WebGL2 / OrthographicView`,
  methodology:'Synthetic precomputed single-line geometry. No CPU culling or aggregation. Frame intervals from deck.gl onAfterRender; not GPU timer queries. First-draw callback is not a GPU completion fence.',
  environment:{userAgent:navigator.userAgent,hardwareConcurrency:navigator.hardwareConcurrency,deviceMemoryGiB:navigator.deviceMemory??null},loads:[],runs:[]};
function error(message){$('status').hidden=false;$('status').textContent=message;loading=false;pendingLoad=null;controls();}
function controls(){
  for(const el of document.querySelectorAll('[data-size], [data-theme], #labels, #edge-picking, #fit, #detail, #benchmark, #bus-id, #search button, #clear'))el.disabled=loading||!!run||!ready||!fixture;
  $('stop').disabled=!run;
}
function environment(){const r=$('canvas-wrap').getBoundingClientRect();return {width:Math.round(r.width),height:Math.round(r.height),devicePixelRatio:devicePixelRatio,renderPixelRatio:Math.min(devicePixelRatio,2),labels:$('labels').checked,wirePicking:$('edge-picking').checked,theme};}
function fitCamera(){const b=fixture.manifest.bounds,r=$('canvas-wrap').getBoundingClientRect();return {...camera,target:[(b[0]+b[2])/2,(b[1]+b[3])/2,0],zoom:Math.log2(Math.min((r.width-70)/(b[2]-b[0]),(r.height-95)/(b[3]-b[1])))};}
function detailCamera(){const m=fixture.manifest,r=Math.floor(m.regions/2),i=r*m.regionSize+2;return {...camera,target:[fixture.positions[i*3]+200,fixture.positions[i*3+1],0],zoom:0.6};}
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
  colour(f);
}
function colour(f){const p=palette[theme];for(let s=0;s<f.manifest.segments;s++)f.segmentColors.set(f.states[f.segmentBranches[s]]?p.open:p.line,s*4);for(let i=0;i<f.manifest.count;i++)f.nodeColors.set(i%f.manifest.regionSize===0?p.open:p.node,i*4);}
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
  return [
    new LineLayer({...common,id:`branches-${fixture.manifest.count}-${theme}`,data:fixture.lineData,getWidth:1,widthUnits:'pixels',pickable:$('edge-picking').checked,autoHighlight:false}),
    new ScatterplotLayer({...common,id:`buses-${fixture.manifest.count}-${theme}`,data:fixture.nodeData,getRadius:1.9,radiusUnits:'common',radiusMinPixels:.65,radiusMaxPixels:4,pickable:true,stroked:false}),
    new LineLayer({...common,id:'selection-lines',data:selectedSegments(),getSourcePosition:d=>d.source,getTargetPosition:d=>d.target,getColor:p.open,getWidth:2.5,pickable:false}),
    new ScatterplotLayer({...common,id:'selection-bus',data:nodeSelection,getPosition:d=>d.position,getRadius:6,radiusUnits:'pixels',getFillColor:p.open,pickable:false}),
    new TextLayer({...common,id:'labels',data:labelData,getPosition:d=>d.position,getText:d=>d.text,getColor:p.label,getSize:12,sizeUnits:'pixels',fontFamily:'Plex, monospace',fontSettings:{sdf:true},getTextAnchor:'start',getAlignmentBaseline:'bottom',getPixelOffset:[5,-5],pickable:false})
  ];
}
function render(){deck.setProps({layers:layers(),parameters:{clearColor:palette[theme].bg}});}
function scheduleLabels(){clearTimeout(labelTimer);labelTimer=setTimeout(updateLabels,90);}
function updateLabels(){
  if(!fixture)return;labelData=[];
  // Selection ID remains discoverable even when automatic labels are off.
  const reserved=selection?.kind==='bus'?selection.index:null;
  if(reserved!==null)labelData.push({position:fixture.positions.subarray(reserved*3,reserved*3+3),text:busName(reserved)});
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
      if(!reserve(px,py))continue;labelData.push({position:[px,py,0],text:busName(i)});if(labelData.length>=200)break;
    }
  }
  $('draw-count').textContent=`${fmt(fixture.manifest.count)} buses · ${fmt(fixture.manifest.segments)} segments submitted · ${labelData.length} labels`;
  render();
}
function select(kind,index,started=performance.now()){
  if(run||loading)return;selection={kind,index};pendingSelection=started;
  if(kind==='bus'){
    const degree=fixture.offsets[index+1]-fixture.offsets[index];
    $('selection').innerHTML=`<h2>${busName(index)}</h2><p>Region ${Math.floor(index/fixture.manifest.regionSize)+1} · ${degree} incident branches<br>Stable synthetic source: bus/${index+1}</p>`;
  }else{
    const a=fixture.endpoints[index*2],b=fixture.endpoints[index*2+1];
    $('selection').innerHTML=`<h2>Branch ${index+1}</h2><p>${busName(a)} → ${busName(b)}<br>${fixture.states[index]?'Open tie · explicit gap':'Closed branch'} · source branch/${index+1}</p>`;
  }
  updateLabels();
}
async function load(count){
  stopRun('fixture changed');const token=++request;abort?.abort();abort=new AbortController();loading=true;pendingLoad=null;selection=null;labelData=[];pendingSelection=null;selectionMetric=null;clearTimeout(labelTimer);controls();
  $('test-status').textContent='Ready';$('status').hidden=false;$('status').textContent=`Loading ${fmt(count)} precomputed buses…`;$('frame-time').textContent='Not measured';$('frame-detail').textContent='Run the six-second camera test';$('selection').innerHTML='<h2>Follow a connection.</h2><p>Select a bus or find it by ID.</p>';$('selection-time').textContent='Selection feedback: not measured';
  for(const b of document.querySelectorAll('[data-size]'))b.setAttribute('aria-pressed',String(+b.dataset.size===count));
  const start=performance.now();
  try{
    const m=entries.find(m=>m.count===count);if(!m)throw new Error('Fixture unavailable');
    const response=await fetch('fixtures/'+m.file,{signal:abort.signal});if(!response.ok)throw new Error(`Fixture HTTP ${response.status}; run generate.mjs`);
    const buffer=await response.arrayBuffer(),fetched=performance.now();if(token!==request)return;
    const next=decodeFixture(m,buffer),decoded=performance.now();prepare(next);const prepared=performance.now();
    fixture=next;pendingLoad={count,sha256:m.sha256,bytes:m.bytes,start,fetchMs:fetched-start,decodeMs:decoded-fetched,prepareMs:prepared-decoded,submittedAt:prepared,environment:environment()};
    $('network-count').textContent=fmt(count);$('network-detail').textContent=`${fmt(m.branches)} branches · ${fmt(m.openTies)} open ties`;
    $('memory').textContent=`${(fixture.ownedBytes/1048576).toFixed(2)} MiB`;$('memory-detail').textContent=`${(m.bytes/1048576).toFixed(2)} MiB binary fixture; excludes index objects`;
    $('canvas-title').textContent=`${fmt(count)} buses / ${m.regions} feeder region${m.regions===1?'':'s'}`;
    const url=new URL(location.href);url.searchParams.set('size',String(count));history.replaceState(null,'',url);
    // A dataset change retires its layers and buffers; camera/selection retain them.
    setCamera(fitCamera(),false);updateLabels();
  }catch(e){if(e.name!=='AbortError'&&token===request)error(e.message);}
}
function afterRender(){
  const now=performance.now();
  if(pendingLoad){
    const m=pendingLoad;pendingLoad=null;const result={...m,firstDrawMs:now-m.start,submitToDrawMs:now-m.submittedAt};delete result.start;delete result.submittedAt;report.loads.push(result);
    $('load-time').textContent=ms(result.firstDrawMs);$('load-detail').textContent=`Fetch ${ms(m.fetchMs)} · prepare ${ms(m.prepareMs)} · submit → draw ${ms(result.submitToDrawMs)}`;
    $('status').hidden=true;loading=false;controls();
  }
  if(pendingSelection!==null){selectionMetric=now-pendingSelection;pendingSelection=null;$('selection-time').textContent=`Selection → draw callback: ${ms(selectionMetric)}`;}
  if(run){if(run.last!==null)run.intervals.push(now-run.last);run.last=now;}
}
function percentile(values,p){const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.min(sorted.length-1,Math.floor((sorted.length-1)*p))]??null;}
function stopRun(reason='cancelled'){
  if(!run)return;const r=run;run=null;cancelAnimationFrame(r.raf);
  const valid=reason==='complete'&&r.intervals.length>=30;
  const result={count:r.count,sha256:r.sha256,started:r.started,durationMs:performance.now()-r.start,complete:valid,reason,environment:r.environment,frames:r.intervals.length,intervalsMs:r.intervals,
    p50Ms:valid?percentile(r.intervals,.5):null,p95Ms:valid?percentile(r.intervals,.95):null,maxMs:valid?Math.max(...r.intervals):null,selectionFeedbackMs:selectionMetric};
  report.runs.push(result);$('test-status').textContent=valid?'Complete':`Stopped: ${reason}`;
  if(valid){$('frame-time').textContent=ms(result.p95Ms);$('frame-detail').textContent=`p50 ${ms(result.p50Ms)} · max ${ms(result.maxMs)} · ${result.frames} intervals`;}
  if(!valid){$('frame-time').textContent='Not measured';$('frame-detail').textContent=`Incomplete: ${reason}`;}
  controls();setCamera(r.original);updateLabels();
}
function benchmark(){
  if(!fixture||loading||run||document.hidden)return;
  const overview=fitCamera(),detail=detailCamera();
  run={count:fixture.manifest.count,sha256:fixture.manifest.sha256,start:performance.now(),started:new Date().toISOString(),last:null,intervals:[],original:{...camera,target:[...camera.target]},environment:environment(),raf:0};controls();$('frame-time').textContent='Running…';$('frame-detail').textContent='Measuring camera frame intervals';
  function tick(now){if(!run)return;const t=(now-run.start)/6000;if(t>=1){stopRun('complete');return;}
    // overview → local detail → overview, with a small continuous pan
    const mix=(1-Math.cos(t*Math.PI*2))/2;
    setCamera({...overview,zoom:overview.zoom+(detail.zoom-overview.zoom)*mix,target:[overview.target[0]+(detail.target[0]-overview.target[0])*mix+Math.sin(t*4*Math.PI)*60,overview.target[1]+(detail.target[1]-overview.target[1])*mix,0]},false);
    if($('labels').checked)updateLabels();
    $('test-status').textContent=`Running · ${Math.round(t*100)}%`;run.raf=requestAnimationFrame(tick);
  }run.raf=requestAnimationFrame(tick);
}
function exportReport(){const data={...report,exported:new Date().toISOString(),current:fixture?{count:fixture.manifest.count,ownedCpuArrayBytes:fixture.ownedBytes,environment:environment()}:null};
  const url=URL.createObjectURL(new Blob([JSON.stringify(data,null,2)],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download='tellegen-diagram-performance.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}

for(const b of document.querySelectorAll('[data-size]'))b.addEventListener('click',()=>load(+b.dataset.size));
for(const b of document.querySelectorAll('[data-theme]'))b.addEventListener('click',()=>{theme=b.dataset.theme;document.documentElement.dataset.theme=theme;for(const x of document.querySelectorAll('[data-theme]'))if(x.tagName==='BUTTON')x.setAttribute('aria-pressed',String(x.dataset.theme===theme));colour(fixture);
  // Theme changes retire base layers in this baseline; production should patch colours.
  fixture.lineData={...fixture.lineData,attributes:{...fixture.lineData.attributes}};fixture.nodeData={...fixture.nodeData,attributes:{...fixture.nodeData.attributes}};render();});
$('labels').addEventListener('change',updateLabels);$('edge-picking').addEventListener('change',render);
$('fit').addEventListener('click',()=>setCamera(fitCamera()));$('detail').addEventListener('click',()=>setCamera(detailCamera()));
$('clear').addEventListener('click',()=>{selection=null;$('selection').innerHTML='<h2>Follow a connection.</h2><p>Select a bus or find it by ID.</p>';updateLabels();});
$('search').addEventListener('submit',e=>{e.preventDefault();if(!fixture||run||loading)return;const started=performance.now(),value=$('bus-id').value.trim().replace(/^B/i,''),id=Number(value);$('search-error').textContent='';
  if(!/^\d+$/.test(value)||!Number.isSafeInteger(id)||id<1||id>fixture.manifest.count){$('search-error').textContent=`Enter a bus from B000001 to ${busName(fixture.manifest.count-1)}.`;return;}
  const i=id-1;select('bus',i,started);setCamera({...camera,target:[fixture.positions[i*3],fixture.positions[i*3+1],0],zoom:1.5});});
$('benchmark').addEventListener('click',benchmark);$('stop').addEventListener('click',()=>stopRun());$('export-report').addEventListener('click',exportReport);
document.addEventListener('visibilitychange',()=>{if(document.hidden)stopRun('tab hidden');});
window.addEventListener('resize',()=>{stopRun('viewport resized');scheduleLabels();});
window.addEventListener('pagehide',()=>{abort?.abort();clearTimeout(labelTimer);deck?.finalize();});
$('network-canvas').addEventListener('pointerdown',()=>{if(run)stopRun('user interaction');});
async function start(){
  controls();const res=await fetch('fixtures/manifest.json');if(!res.ok)throw new Error('Missing fixtures: run generate.mjs');entries=await res.json();
  // Font availability is resolved before TextLayer creates its glyph atlas.
  await document.fonts.load('12px Plex');
  deck=new Deck({canvas:$('network-canvas'),parent:$('canvas-wrap'),width:'100%',height:'100%',deviceProps:{type:'webgl'},useDevicePixels:Math.min(devicePixelRatio,2),
    views:new OrthographicView({id:'schematic',flipY:true}),viewState:camera,controller:{doubleClickZoom:true},layers:[],pickingRadius:5,
    onViewStateChange:({viewState})=>{if(run)stopRun('user interaction');setCamera(viewState);},
    onClick:info=>{if(info.index<0||!fixture||loading||run)return;if(info.layer?.id.startsWith('buses-'))select('bus',info.index);else if(info.layer?.id.startsWith('branches-'))select('branch',fixture.segmentBranches[info.index]);},
    getCursor:({isDragging})=>isDragging?'grabbing':'crosshair',onAfterRender:afterRender,onError:e=>{stopRun('renderer error');error('Renderer error: '+e.message);},
    onLoad:()=>{ready=true;const requested=Number(new URLSearchParams(location.search).get('size'));load([1000,10000,100000].includes(requested)?requested:1000);}});
}
start().catch(e=>error(e.message));
