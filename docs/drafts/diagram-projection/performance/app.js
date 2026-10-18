import {Deck,OrthographicView,COORDINATE_SYSTEM,VERSION} from '@deck.gl/core';
import {LineLayer,ScatterplotLayer,TextLayer} from '@deck.gl/layers';
import {resultIssues,selectionBus,voltageProfile,hoverDescription} from './engineering-workflow.mjs';
import {profileMarkup} from './voltage-profile-view.mjs';
import {bindResults} from './results-adapter.mjs';
import {prepareResults,visibleAnchors,flowGlyphs,resultDescription,loadingState,voltageState} from './results-overlay.mjs';
import {prepareEquipment,equipmentRow} from './equipment.mjs';
import {decodeFixture} from './fixture.mjs';
import {busQuery,sourceKey} from './bmopf-adapter.mjs';
import {validateSchematic,NO_BRANCH} from './schematic.mjs';
import {nonTreeBranches,inspectCircuit,branchSegments,overlayBounds} from './circuit-inspection.mjs';
import {buildOpenTiePresentation,incidentPresentation,compactBranch} from './open-ties.mjs';
import {validateReferences} from './bmopf-references.mjs';
import {rankIssues,snapshotSummary} from './workbench-summary.mjs';
import {buildSearchIndex,search,entityKey,syntheticBus} from './search-index.mjs';
import {encodeCamera,decodeCamera,withView} from './view-state.mjs';
import {placeLabels,busCandidate,branchCandidate,MIN_LABEL_ZOOM} from './value-labels.mjs';
import {viewportSvg,titleBlock,fitText,TITLE_BLOCK_HEIGHT} from './export-view.mjs';
import {tableModel} from './results-table.mjs';
import {inspectorMarkup} from './multiconductor-view.mjs';
import {feederEnvelope,zoneEnvelope} from './feeder-envelope.mjs';
import {MORPH_MS,MORPH_MAX_BUSES,ease,screenAnchored,lerpInto,chordsInto} from './layout-morph.mjs';
import {createTableView} from './table-view.mjs';
import {COLOUR_MODES,modeData,flowWidths,gradient,SEQUENTIAL_STOPS,DIVERGING_STOPS} from './result-colours.mjs';

const $=id=>document.getElementById(id), fmt=n=>n.toLocaleString(), ms=n=>`${n.toFixed(1)} ms`;
const busName=i=>fixture?.refs?.buses[i].id??('B'+String(i+1).padStart(6,'0'));
const labelName=i=>busName(i).length>12?busName(i).slice(0,11)+'…':busName(i);
const escape=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const locationKey=(f,i)=>`${f.positions[i*3]},${f.positions[i*3+1]}`;
const identity=m=>({id:m.id,layoutMode:m.layoutMode??'synthetic',routingMode:m.routingMode??'current',routing:m.routing??null,equipment:m.equipment??null,layout:m.layout??null,kind:m.kind??'synthetic',count:m.count,sha256:m.sha256,refsSha256:m.refsSha256??null,adapterVersion:m.adapterVersion??null,scenario:m.scenario??null,sources:m.sources??[],placement:m.placement??'synthetic schematic',stagingMs:m.stagingMs??null});
const palette={dark:{line:[108,137,150,255],node:[198,207,203,255],open:[229,170,84,255],label:[238,234,225,255],bg:[23/255,27/255,32/255,1]},paper:{line:[108,125,127,255],node:[48,69,78,255],open:[178,94,0,255],label:[32,36,43,255],bg:[252/255,251/255,247/255,1]}};
const resultParams=new URLSearchParams(location.search);
let resultMode=COLOUR_MODES[resultParams.get('results')]?resultParams.get('results'):'none';
// Threshold and continuous variants of one quantity share issue focus, warnings and presets.
const family=mode=>mode.startsWith('voltage')?'voltage':mode.startsWith('loading')?'loading':'none';
const inFamily=kind=>family(resultMode)===kind?resultMode:kind;
let flowMode=['static','motion'].includes(resultParams.get('flows'))?resultParams.get('flows'):'none';
if(matchMedia('(prefers-reduced-motion: reduce)').matches&&flowMode==='motion')flowMode='static';
let flowElapsed=0,flowStamp=null,playback=1,profile=null,profileTarget=null,profileNavigating=false,profileSegments=[],profileHover=null,voltageCursor=-1;
let resultCatalog={},flowScope={anchors:[],stats:{}},flowData=[],flowRaf=0,flowPeak=0,scopeMs=0;
let theme='dark', fixture=null, entries=[], deck, ready=false, loading=false, request=0, abort;
let camera={target:[0,0,0],zoom:0,minZoom:-24,maxZoom:5}, selection=null, labelData=[], valueData=[], run=null;
const requestedLayout=new URLSearchParams(location.search).get('layout');
let layoutMode=['source','tidy-tree'].includes(requestedLayout)?requestedLayout:'schematic',traceRoot=false,pendingRestoreSelection=null;
const requestedRouting=new URLSearchParams(location.search).get('routing');
let routingMode=['corridor','crossing-aware','equipment'].includes(requestedRouting)?requestedRouting:'current';
let openMode=new URLSearchParams(location.search).get('open')==='full'?'full':'compact',openScope=null,openCursor=-1;
const compactOpen=()=>openMode==='compact'&&!!fixture?.openTies.compact;
const openSelected=()=>selection?.kind==='branch'&&fixture?.openTies.branchSet.has(selection.index);
const viewKey=m=>`${m.id}/${m.layoutMode}/${m.routingMode??'current'}`;
const viewStates=new Map();
let pendingLoad=null,pendingSelection=null,labelTimer=0,selectionMetric=null;
let circuit=null,overlay=[],pathOverlay=[],pathBars=[],endpointData=[],meshMatches=[],meshCursor=-1;
const isInspecting=()=>!!circuit&&$('inspect-circuit').checked;
// Workbench: one search, a ranked issue list, snapshot KPIs and URL-restorable view state.
const searchCache=new WeakMap(),syncUrl=state=>history.replaceState(null,'',withView(location.href,state));
let issueCursor=-1,omniItems=[],omniActive=-1,omniTimer=0,viewTimer=0,initialView={dataset:resultParams.get('dataset'),sel:resultParams.get('sel'),camera:decodeCamera(resultParams.get('view'))};
const report={version:3,started:new Date().toISOString(),renderer:`deck.gl ${VERSION} / WebGL2 / OrthographicView`,
  methodology:'Precomputed synthetic or BMOPF geometry; identity and placement recorded per dataset. No viewport culling or network level of detail; optional open-switch presentation is prepared once on load. Frame intervals from deck.gl onAfterRender; not GPU timer queries. First-draw callback is not a GPU completion fence.',
  environment:{userAgent:navigator.userAgent,hardwareConcurrency:navigator.hardwareConcurrency,deviceMemoryGiB:navigator.deviceMemory??null},loads:[],runs:[],selections:[],presentations:[],morphs:[]};
function error(message){$('status').hidden=false;$('status').textContent=message;loading=false;pendingLoad=null;controls();}
function controls(){
  for(const b of document.querySelectorAll('[data-result-preset]'))b.disabled=loading||!!run||!fixture?.results;
  for(const id of ['profile-bus','profile-prev','profile-next','profile-fit','profile-path'])$(id).disabled=loading||!!run||!profile?.nodes?.length;
  for(const id of ['result-mode','flow-mode','flow-play','flow-speed','flow-p','flow-q','flow-width'])$(id).disabled=loading||!!run||!fixture?.results;
  for(const el of document.querySelectorAll('button[data-theme], #labels, #edge-picking, #fit, #detail, #zoom-in, #zoom-out, #export-png, #export-svg, #benchmark, #omni, #clear, #next-colocated, #trace-root, #focus-source'))el.disabled=loading||!!run||!ready||!fixture;
  for(const el of document.querySelectorAll('[data-size], #dataset'))el.disabled=loading||!!run||!ready;
  for(const el of document.querySelectorAll('[data-layout-quick]'))el.disabled=loading||!!run||!ready;
  for(const el of document.querySelectorAll('[data-layout]'))el.disabled=loading||!!run||!ready||fixture?.manifest.kind!=='bmopf'||(el.dataset.layout==='source'&&fixture?.manifest.sourceCoordinatesAvailable===false)||(el.dataset.layout!=='source'&&!entries.find(m=>m.id===fixture?.manifest.id)?.layouts?.[el.dataset.layout]);
  for(const el of document.querySelectorAll('[data-routing]'))el.disabled=loading||!!run||!ready||!fixture?.parent||!entries.find(m=>m.id===fixture?.manifest.id)?.layouts?.[layoutMode+(el.dataset.routing==='current'?'':'-'+el.dataset.routing)];
  $('trace-root').disabled=loading||!!run||!fixture?.parent||selection?.kind!=='bus';
  $('focus-source').disabled=loading||!!run||!fixture?.refs||selection===null;
  const unavailable=loading||!!run||!fixture?.meshBranches?.length;
  for(const id of ['inspect-circuit','mesh-query','mesh-search-button'])$(id).disabled=unavailable;
  for(const id of ['mesh-prev','mesh-next','mesh-results'])$(id).disabled=unavailable||!meshMatches.length;
  for(const id of ['show-tree-path','dim-context','fit-connection','fit-circuit'])$(id).disabled=loading||!!run||!isInspecting();
  $('fit-circuit').disabled||=!circuit?.connected;
  for(const el of document.querySelectorAll('[data-open-mode]'))el.disabled=loading||!!run||!fixture?.openTies.branchIndices.length;
  for(const id of ['open-results','open-prev','open-next'])$(id).disabled=loading||!!run||!fixture?.openTies.branchIndices.length;
  $('open-all').disabled=loading||!!run||openScope===null;
  const profileIndex=profile?.shown?.indexOf(Number($('profile-bus').value))??-1;
  $('profile-prev').disabled||=profileIndex<=0;
  $('profile-next').disabled||=profileIndex<0||profileIndex>=profile.shown.length-1;
  $('stop').disabled=!run;
}
function environment(){const r=$('canvas-wrap').getBoundingClientRect();return {width:Math.round(r.width),height:Math.round(r.height),devicePixelRatio:devicePixelRatio,renderPixelRatio:Math.min(devicePixelRatio,2),labels:$('labels').checked,wirePicking:$('edge-picking').checked,theme,results:fixture?.results?{kind:fixture.results.kind,sha256:fixture.manifest.resultSha256,mode:resultMode,flows:flowMode,playback,channels:{p:$('flow-p').checked,q:$('flow-q').checked},scales:fixture.results.flowScales,profilePoints:profile?.shown?.length??0,scope:'visible endpoint segments at detail zoom',peakFlowGlyphs:flowPeak,lastScope:flowScope.stats,maxScopeMs:scopeMs,arrayBytes:fixture.results.arrayBytes,prepareMs:fixture.results.prepareMs}:null,equipmentGlyphs:fixture?.manifest.equipment?.glyphs??0,equipmentStrokes:fixture?.manifest.equipment?.strokes??0,equipmentArrayBytes:fixture?.equipment?.arrayBytes??0,selectedOpenSwitch:!!openSelected(),selectionRouteSegments:overlay.length,openSwitchPresentation:openMode,openSwitches:fixture?.openTies.branchIndices.length??0,openEndpointMarkers:compactOpen()?fixture.openTies.markers.length:0,suppressedRouteSegments:compactOpen()?fixture.openTies.suppressedSegments:0,extraPresentationArrayBytes:fixture?.openArrayBytes??0,inspection:isInspecting()?{branch:circuit.branch,treePath:$('show-tree-path').checked,dimContext:$('dim-context').checked,treeEdges:circuit.edges.length,routeSegments:overlay.length,pathSegments:pathOverlay.length}:null};}
function fitCamera(){const b=fixture.manifest.bounds,r=$('canvas-wrap').getBoundingClientRect();return {...camera,target:[(b[0]+b[2])/2,(b[1]+b[3])/2,0],zoom:Math.log2(Math.min((r.width-70)/(b[2]-b[0]),(r.height-95)/(b[3]-b[1])))};}
function detailCamera(){const m=fixture.manifest,r=Math.floor(m.regions/2),i=m.kind==='bmopf'?Math.floor(m.count/2):r*m.regionSize+2;return {...camera,target:[fixture.positions[i*3]+200,fixture.positions[i*3+1],0],zoom:0.6};}
function setCamera(next,updateLabels=true){camera={target:next.target,zoom:next.zoom,minZoom:-24,maxZoom:5};deck.setProps({viewState:camera});refreshResults();$('zoom-readout').textContent=`${(2**camera.zoom).toPrecision(3)}× · PRECOMPUTED`;if(updateLabels)scheduleLabels();clearTimeout(viewTimer);if(!run&&fixture)viewTimer=setTimeout(()=>syncUrl({view:encodeCamera(camera)}),300);}
function prepare(f){
  const m=f.manifest;
  if(f.busbarSources)f.busbarData={length:m.count,attributes:{getSourcePosition:{value:f.busbarSources,size:3},getTargetPosition:{value:f.busbarTargets,size:3}}};
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
    f.locations=new Map();f.attachmentsByBus=new Map();
    f.refs.buses.forEach((b,i)=>{
      const k=locationKey(f,i);if(!f.locations.has(k))f.locations.set(k,[]);f.locations.get(k).push(i);
    });
    for(const a of f.refs.attachments){if(!f.attachmentsByBus.has(a.bus))f.attachmentsByBus.set(a.bus,[]);f.attachmentsByBus.get(a.bus).push(a);}
  }
  f.meshBranches=nonTreeBranches(f);f.ownedBytes+=f.meshBranches.byteLength;
  const opened=performance.now();f.openTies=buildOpenTiePresentation(f);f.openArrayBytes=f.openTies.arrayBytes;
  if(f.openTies.compact){
    const c=f.openTies.compact;f.compactColors=new Uint8Array(c.segments.length*4);f.openArrayBytes+=f.compactColors.byteLength;
    f.compactLineData={length:c.segments.length,attributes:{getSourcePosition:{value:c.sources,size:3},getTargetPosition:{value:c.targets,size:3},getColor:{value:f.compactColors,size:4,normalized:true}}};
  }
  f.openPrepareMs=performance.now()-opened;
  f.equipment=prepareEquipment(f);if(f.equipment){
    f.ownedBytes+=f.equipment.arrayBytes;
  }f.ownedBytes+=f.openArrayBytes;
  colour(f);
}
function colour(f){const p=palette[theme];
 if(f.equipment)for(const data of new Set([f.equipment.full,f.equipment.compact]))for(let i=0;i<data.owners.length;i++)data.colors.set(equipmentRow(f,data.owners[i]).row.disabled?p.open:p.node,i*4);
for(let s=0;s<f.manifest.segments;s++){const b=f.segmentBranches[s];f.segmentColors.set(f.states[b]?p.open:f.refs?.branches[b].family==='transformer'?[141,124,209,255]:f.refs?.branches[b].family==='switch'?[101,165,132,255]:f.branchRoles?.[b]===1?[66,159,181,255]:p.line,s*4);}if(f.openTies.compact)for(let i=0;i<f.openTies.compact.segments.length;i++){const s=f.openTies.compact.segments[i];f.compactColors.set(f.segmentColors.subarray(s*4,s*4+4),i*4);}for(let i=0;i<f.manifest.count;i++)f.nodeColors.set(!f.refs&&i%f.manifest.regionSize===0?p.open:p.node,i*4);}

function rootPath(index){const edges=[];let u=index;while(fixture.parent&&fixture.parent[u]!==u){edges.push(fixture.parentBranch[u]);u=fixture.parent[u];if(edges.length>fixture.manifest.count)throw new Error('Invalid parent cycle');}return {edges,root:u};}
function focusSource(){const row=selection.kind==='bus'?fixture.refs.buses[selection.index]:selection.kind==='attachment'?fixture.refs.attachments[selection.index]:fixture.refs.branches[selection.index],source=row.source;
 const b=fixture.manifest.panels?.find(p=>p.source===source)?.bounds??fixture.refs.sources[source].displayBounds??fixture.manifest.bounds,r=$('canvas-wrap').getBoundingClientRect();
 setCamera({...camera,target:[(b[0]+b[2])/2,(b[1]+b[3])/2,0],zoom:Math.log2(Math.min((r.width-70)/(b[2]-b[0]+80),(r.height-95)/(b[3]-b[1]+80)))});
}
function refreshOverlay(){
  circuit=null;overlay=[];pathOverlay=[];pathBars=[];endpointData=[];
  if(!selection||selection.kind==='attachment')return;
  const branches=selection.kind==='branch'?[selection.index]:traceRoot&&fixture.parent?rootPath(selection.index).edges:fixture.incidents.subarray(fixture.offsets[selection.index],fixture.offsets[selection.index+1]);
  overlay=branchSegments(fixture,selection.kind==='bus'?incidentPresentation(branches,fixture.openTies,compactOpen()):branches);
  if(selection.kind==='branch'&&fixture.parent&&fixture.branchRoles[selection.index]===1&&!fixture.states[selection.index]){
    circuit=inspectCircuit(fixture,selection.index);
    pathOverlay=branchSegments(fixture,circuit.edges);
    if(fixture.busbarSources)pathBars=circuit.nodes.map(i=>({source:fixture.busbarSources.subarray(i*3,i*3+3),target:fixture.busbarTargets.subarray(i*3,i*3+3)}));
  }
  if(circuit||openSelected()){
    const ends=circuit?.endpoints??[fixture.endpoints[selection.index*2],fixture.endpoints[selection.index*2+1]];
    endpointData=[...new Set(ends)].map((i,j)=>({position:fixture.positions.subarray(i*3,i*3+3),end:j===1,text:`${ends[0]===ends[1]?'A / B':j?'B':'A'} · ${busName(i).length>20?busName(i).slice(0,8)+'…'+busName(i).slice(-6):busName(i)}`}));
  }
}
function fitInspection(includePath){
  if(!circuit)return;
  const bounds=overlayBounds(fixture,includePath&&circuit.connected?circuit.nodes:circuit.endpoints,includePath?[...overlay,...pathOverlay]:overlay),[x0,y0,x1,y1]=bounds,r=$('canvas-wrap').getBoundingClientRect();
  setCamera({...camera,target:[(x0+x1)/2,(y0+y1)/2,0],zoom:Math.min(1.5,Math.log2(Math.min(Math.max(50,r.width-130)/(x1-x0+50),Math.max(50,r.height-150)/(y1-y0+50))))});
}
function meshLabel(b){const row=fixture.refs.branches[b];return `${fixture.refs.sources[row.source].label} / ${row.family}${row.kind?' / '+row.kind:''} / ${row.id} · ${busName(row.from)} ↔ ${busName(row.to)}`;}
function showMeshPage(){
  const start=Math.max(0,Math.floor(Math.max(0,meshCursor)/40)*40),end=Math.min(meshMatches.length,start+40),select=$('mesh-results');
  select.replaceChildren();
  const placeholder=document.createElement('option');placeholder.value='';placeholder.textContent='Choose a non-tree connection';select.append(placeholder);
  for(let i=start;i<end;i++){const option=document.createElement('option');option.value=String(i);option.textContent=`${i+1}. ${meshLabel(meshMatches[i])}`;select.append(option);}
  select.value=meshCursor<0?'':String(meshCursor);
  $('mesh-count').textContent=!fixture?.parent?'Choose a schematic BMOPF layout to inspect its display tree.':!fixture.meshBranches.length?'This display forest has no enabled non-tree connections.':`${fmt(meshMatches.length)} matching / ${fmt(fixture.meshBranches.length)} non-tree connections · ${meshCursor<0?'none selected':'selected '+(meshCursor+1)} · results ${meshMatches.length?start+1:0}–${end}. Previous / next visits every match.`;
  controls();
}
function filterMesh(){
  const query=$('mesh-query').value.trim().toLocaleLowerCase();
  meshMatches=Array.from(fixture?.meshBranches??[]).filter(b=>!query||meshLabel(b).toLocaleLowerCase().includes(query));
  meshCursor=selection?.kind==='branch'?meshMatches.indexOf(selection.index):-1;showMeshPage();
}
function chooseMesh(i){
  if(!meshMatches.length||loading||run)return;
  meshCursor=(i+meshMatches.length)%meshMatches.length;$('inspect-circuit').checked=true;
  select('branch',meshMatches[meshCursor]);fitInspection($('show-tree-path').checked);
}
function openMatches(){return !fixture?[]:openScope===null?fixture.openTies.branchIndices:fixture.openTies.byBus.get(openScope)??[];}
function showOpenPage(){
  const matches=openMatches();openCursor=selection?.kind==='branch'?matches.indexOf(selection.index):-1;
  const start=Math.max(0,Math.floor(Math.max(0,openCursor)/40)*40),end=Math.min(matches.length,start+40),list=$('open-results');
  list.replaceChildren();const placeholder=document.createElement('option');placeholder.value='';placeholder.textContent='Choose an open switch';list.append(placeholder);
  for(let i=start;i<end;i++){const option=document.createElement('option');option.value=String(i);option.textContent=`${i+1}. ${meshLabel(matches[i])}`;list.append(option);}
  list.value=openCursor<0?'':String(openCursor);
  const ties=fixture?.openTies;
  $('open-summary').textContent=!ties?.branchIndices.length?'No open switches in this dataset; both presentations are identical.':`${ties.branchIndices.length} open switches at ${ties.markers.length} endpoint buses. ${openMode==='compact'?'Endpoint rings replace '+ties.suppressedSegments+' route segments; selecting one switch reveals its full route.':'All open routes shown.'} ${openScope===null?'All open switches': 'At bus '+busName(openScope)} · ${matches.length?start+1:0}–${end} of ${matches.length}.`;
  for(const b of document.querySelectorAll('[data-open-mode]'))b.setAttribute('aria-pressed',String(b.dataset.openMode===openMode));
  controls();
}
function fitSelectedRoute(){
  if(selection?.kind!=='branch')return;
  const b=fixture.refs.branches[selection.index],[x0,y0,x1,y1]=overlayBounds(fixture,[b.from,b.to],overlay),r=$('canvas-wrap').getBoundingClientRect();
  setCamera({...camera,target:[(x0+x1)/2,(y0+y1)/2,0],zoom:Math.min(1.5,Math.log2(Math.min(Math.max(50,r.width-130)/(x1-x0+50),Math.max(50,r.height-150)/(y1-y0+50))))});
}
function chooseOpen(index,fit=true){
  const matches=openMatches();if(!matches.length||loading||run)return;
  const i=(index+matches.length)%matches.length;select('branch',matches[i]);if(fit)fitSelectedRoute();
}
function setOpenMode(mode){
  if(!fixture||loading||run)return;
  const started=performance.now();openMode=mode;
  syncUrl({open:mode});
  refreshOverlay();showOpenPage();updateLabels();
  report.presentations.push({dataset:fixture.manifest.id,layoutMode:fixture.manifest.layoutMode,routingMode:fixture.manifest.routingMode,mode,updateMs:performance.now()-started,extraArrayBytes:fixture.openArrayBytes});
}
function equipmentLayers(common,p,opacity){
 if(!fixture.equipment)return [];
 const eq=fixture.equipment,data=compactOpen()?eq.compact:eq.full;
 const owner=selection?.kind==='branch'?selection.index:selection?.kind==='attachment'?fixture.manifest.branches+selection.index:null;
 const range=eq.ranges.get(owner),selected=range?{length:range[1]-range[0],attributes:{getSourcePosition:{value:fixture.equipmentSources.subarray(range[0]*3,range[1]*3),size:3},getTargetPosition:{value:fixture.equipmentTargets.subarray(range[0]*3,range[1]*3),size:3}}}:null;
 return [new ScatterplotLayer({...common,id:'equipment-junctions',data:{length:fixture.attachmentJunctions.length/3,attributes:{getPosition:{value:fixture.attachmentJunctions,size:3}}},opacity,getRadius:1.3,radiusUnits:'pixels',getFillColor:p.node,pickable:false}),
  new LineLayer({...common,id:`equipment-strokes-${fixture.manifest.sha256}-${theme}-${openMode}`,data:data.lines,opacity,getWidth:1.5,widthUnits:'pixels',pickable:false}),
  new ScatterplotLayer({...common,id:`equipment-hit-${fixture.manifest.sha256}-${openMode}`,data:data.hits,getRadius:16,radiusUnits:'common',radiusMinPixels:2,getFillColor:[0,0,0,0],pickable:true}),
  ...(selected?[new LineLayer({...common,id:'equipment-selection',data:selected,getColor:p.open,getWidth:2.5,pickable:false})]:[])];
}
// ---- Layout morph: buses glide between layouts of the same source identities; routed geometry returns at the end ----
let morph=null;
function startMorph(from){
 cancelAnimationFrame(morph?.raf);morph=null;
 // Same reference file means the same bus and branch indices; anything else is a different network.
 if(!from||from.refsSha256!==fixture.manifest.refsSha256||from.positions.length!==fixture.positions.length||fixture.manifest.count>MORPH_MAX_BUSES&&resultParams.get('morph')!=='always'||run||matchMedia('(prefers-reduced-motion: reduce)').matches)return;
 const n=fixture.manifest.branches;
 morph={from:screenAnchored(from.positions,from.camera,camera),cur:new Float32Array(fixture.positions.length),s:new Float32Array(n*3),t:new Float32Array(n*3),start:null,raf:0,last:null,intervals:[]};
 // One warm-up frame draws the hidden target layers (their one-time GPU upload) before the clock starts.
 const tick=now=>{if(!morph)return;if(!morph.warm){morph.warm=true;render();morph.raf=requestAnimationFrame(tick);return;}morph.start??=now;if(morph.last!==null)morph.intervals.push(now-morph.last);morph.last=now;const k=ease(Math.min(1,(now-morph.start)/MORPH_MS));lerpInto(morph.cur,morph.from,fixture.positions,k);chordsInto(morph.s,morph.t,fixture.endpoints,morph.cur);
  if(k<1){render();morph.raf=requestAnimationFrame(tick);}else{const v=morph.intervals;report.morphs.push({dataset:fixture.manifest.id,layoutMode:fixture.manifest.layoutMode,buses:fixture.manifest.count,frames:v.length,p50Ms:percentile(v,.5),p95Ms:percentile(v,.95),maxMs:v.length?Math.max(...v):null,intervalsMs:v,environment:environment()});morph=null;render();}};
 morph.raf=requestAnimationFrame(tick);
}
function morphLayers(p,common){
 // Fresh data wrappers each frame tell deck.gl the in-place buffers changed.
 return [new LineLayer({...common,id:'morph-chords',data:{length:fixture.manifest.branches,attributes:{getSourcePosition:{value:morph.s,size:3},getTargetPosition:{value:morph.t,size:3}}},getColor:p.line,getWidth:1,widthUnits:'pixels',pickable:false}),
  new ScatterplotLayer({...common,id:'morph-buses',data:{length:fixture.manifest.count,attributes:{getPosition:{value:morph.cur,size:3}}},getFillColor:p.node,getRadius:1.6,radiusUnits:'pixels',pickable:false})];
}
function layers(){
  if(!fixture)return [];
  // Target layers stay resident but hidden while morphing, so their GPU buffers are not rebuilt at the end.
  if(morph)return [...baseLayers().map(l=>l.clone({visible:false})),...morphLayers(palette[theme],{coordinateSystem:COORDINATE_SYSTEM.CARTESIAN,parameters:{depthCompare:'always'}})];
  return baseLayers();
}
function baseLayers(){
  const inspecting=isInspecting(),openInspection=openSelected(),endpointsVisible=inspecting||openInspection,contextOpacity=inspecting&&$('dim-context').checked ? 0.2 : 1;
  const p=palette[theme],common={coordinateSystem:COORDINATE_SYSTEM.CARTESIAN,parameters:{depthCompare:'always'}};
  const nodeSelection=selection?.kind==='bus'?[{position:fixture.positions.subarray(selection.index*3,selection.index*3+3)}]:[];
  const panels=fixture.manifest.panels??(fixture.refs?.sources??[]).filter(s=>s.displayBounds).map(s=>({...s,bounds:s.displayBounds}));
  const layout=fixture.manifest.layout,sourceRoots=new Set(layout?.sourceBuses??[]);
  const roots=[...new Set([...(layout?.sourceBuses??[]),...(layout?.components??[]).map(c=>c.root)])].map(i=>({position:fixture.positions.subarray(i*3,i*3+3),label:sourceRoots.has(i)?'Source':'Layout root'}));
  return [
    new TextLayer({...common,id:`panel-labels-${fixture.manifest.id}-${theme}`,data:panels.filter(p=>(p.bounds[2]-p.bounds[0])*2**camera.zoom>170),getPosition:s=>[s.bounds[0],s.bounds[1]-30,0],getText:s=>s.label,getColor:p.label,getSize:13,getTextAnchor:'start',getAlignmentBaseline:'bottom',fontFamily:'Plex, monospace',characterSet:'auto',pickable:false}),
    new LineLayer({...common,id:`branches-${fixture.manifest.id}-${fixture.manifest.sha256}-${theme}`,data:resultData('lineData'),visible:!compactOpen(),opacity:contextOpacity,getWidth:1,widthUnits:'pixels',pickable:$('edge-picking').checked,autoHighlight:false}),
    ...(fixture.compactLineData?[new LineLayer({...common,id:`compact-branches-${fixture.manifest.id}-${fixture.manifest.sha256}-${theme}`,data:resultData('compactLineData'),visible:compactOpen(),opacity:contextOpacity,getWidth:1,widthUnits:'pixels',pickable:$('edge-picking').checked,autoHighlight:false})]:[]),
    ...(fixture.busbarData?[new LineLayer({...common,id:`busbars-${fixture.manifest.id}-${fixture.manifest.sha256}-${theme}`,data:fixture.busbarData,opacity:contextOpacity,getColor:p.node,getWidth:2,widthUnits:'pixels',pickable:true})]:[]),
    new ScatterplotLayer({...common,id:`buses-${fixture.manifest.id}-${fixture.manifest.sha256}-${theme}`,data:resultData('nodeData'),opacity:contextOpacity,getRadius:1.9,radiusUnits:'common',radiusMinPixels:.65,radiusMaxPixels:4,pickable:true,stroked:false}),
    new ScatterplotLayer({...common,id:`open-endpoints-${fixture.manifest.id}-${fixture.manifest.sha256}`,data:fixture.openTies.markers,visible:compactOpen(),opacity:contextOpacity,getPosition:d=>d.position,getRadius:7,radiusUnits:'common',radiusMinPixels:2.5,radiusMaxPixels:6,filled:false,stroked:true,getLineColor:p.open,lineWidthUnits:'pixels',getLineWidth:1.5,pickable:true}),
    new ScatterplotLayer({...common,id:'layout-roots',data:roots,opacity:contextOpacity,getPosition:d=>d.position,getRadius:4,radiusUnits:'pixels',getFillColor:p.open,pickable:false}),
    new TextLayer({...common,id:'root-labels',data:!inspecting&&camera.zoom>-2?roots.slice(0,100):[],getPosition:d=>d.position,getText:d=>d.label,getColor:p.open,getSize:11,getPixelOffset:[-8,0],getTextAnchor:'end',fontFamily:'Plex, monospace',characterSet:'auto',pickable:false}),
    new LineLayer({...common,id:'profile-path',data:$('profile-path').checked?profileSegments:[],getSourcePosition:d=>d.source,getTargetPosition:d=>d.target,getColor:[70,155,185,190],getWidth:3,pickable:false}),
    new ScatterplotLayer({...common,id:'profile-hover',data:profileHover===null?[]:[fixture.positions.subarray(profileHover*3,profileHover*3+3)],getPosition:d=>d,getRadius:8,radiusUnits:'pixels',filled:false,stroked:true,getLineColor:p.open,getLineWidth:2,lineWidthUnits:'pixels',pickable:false}),
    new LineLayer({...common,id:'circuit-tree-path',data:inspecting&&$('show-tree-path').checked?pathOverlay:[],getSourcePosition:d=>d.source,getTargetPosition:d=>d.target,getColor:p.open,getWidth:2.5,pickable:false}),
    new LineLayer({...common,id:'circuit-tree-bars',data:inspecting&&$('show-tree-path').checked?pathBars:[],getSourcePosition:d=>d.source,getTargetPosition:d=>d.target,getColor:p.open,getWidth:3,pickable:false}),
    new LineLayer({...common,id:'connection-halo',data:endpointsVisible?overlay:[],getSourcePosition:d=>d.source,getTargetPosition:d=>d.target,getColor:p.bg.map((x,i)=>i===3?255:Math.round(x*255)),getWidth:7,pickable:false}),
    new LineLayer({...common,id:'selection-lines',data:overlay,getSourcePosition:d=>d.source,getTargetPosition:d=>d.target,getColor:inspecting?[66,159,181,255]:p.open,getWidth:inspecting?3:2.5,pickable:false}),
    new ScatterplotLayer({...common,id:'circuit-endpoints',data:endpointsVisible?endpointData:[],getPosition:d=>d.position,getRadius:6,radiusUnits:'pixels',getFillColor:openInspection?p.open:[66,159,181,255],stroked:true,getLineColor:p.label,lineWidthUnits:'pixels',getLineWidth:1.5,pickable:false}),
    new TextLayer({...common,id:'circuit-endpoint-labels',data:endpointsVisible?endpointData:[],getPosition:d=>d.position,getText:d=>d.text,getColor:p.label,getSize:12,getPixelOffset:d=>[d.position[0]>camera.target[0]?-10:10,d.end?22:-12],getTextAnchor:d=>d.position[0]>camera.target[0]?'end':'start',getAlignmentBaseline:'bottom',updateTriggers:{getPixelOffset:camera.target[0],getTextAnchor:camera.target[0]},fontFamily:'Plex, monospace',characterSet:'auto',background:true,getBackgroundColor:p.bg.map((x,i)=>i===3?255:Math.round(x*255)),backgroundPadding:[4,3],pickable:false}),
    ...equipmentLayers(common,p,contextOpacity),
    new TextLayer({...common,id:'result-flows',data:flowData,getPosition:d=>d.position,getText:d=>d.text,getAngle:d=>d.angle,getColor:d=>d.color,getSize:22,fontFamily:'Plex, monospace',characterSet:['▸','›'],getTextAnchor:'middle',getAlignmentBaseline:'center',pickable:false}),
    new TextLayer({...common,id:'selected-result-labels',data:fixture.results&&selection?.kind==='branch'&&flowMode!=='none'&&camera.zoom>=-.2?flowScope.anchors.filter(a=>a.b===selection.index).map(a=>({position:a.position,text:(a.side?'To':'From')+' '+fixture.results.branches[a.b].ends[a.side].p.toFixed(2)+' kW / '+fixture.results.branches[a.b].ends[a.side].q.toFixed(2)+' kvar'})):[],getPosition:d=>d.position,getText:d=>d.text,getColor:p.label,getSize:12,getPixelOffset:[0,28],getTextAnchor:'middle',background:true,getBackgroundColor:p.bg.map((x,i)=>i===3?255:Math.round(x*255)),backgroundPadding:[3,3],fontFamily:'Plex, monospace',characterSet:'auto',pickable:false}),
    new TextLayer({...common,id:'result-warning-labels',data:resultWarnings(),getPosition:d=>d.position,getText:d=>d.text,getColor:[225,91,88,255],getSize:13,getPixelOffset:[0,-32],fontFamily:'Plex, monospace',characterSet:'auto',getTextAnchor:'middle',pickable:false}),
    new ScatterplotLayer({...common,id:'selection-bus',data:nodeSelection,getPosition:d=>d.position,getRadius:6,radiusUnits:'pixels',getFillColor:p.open,pickable:false}),
    new TextLayer({...common,id:'value-labels',data:valueData,getPosition:d=>d.position,getText:d=>d.text,getColor:d=>d.state==='violation'?[225,91,88,255]:d.state==='warning'?[219,158,61,255]:p.label,getSize:11,sizeUnits:'pixels',fontFamily:'Plex, monospace',characterSet:'auto',getTextAnchor:'start',getAlignmentBaseline:'top',getPixelOffset:[6,13],background:true,getBackgroundColor:p.bg.map((x,i)=>i===3?225:Math.round(x*255)),backgroundPadding:[3,1],updateTriggers:{getColor:theme,getBackgroundColor:theme},pickable:false}),
    new TextLayer({...common,id:'labels',data:labelData,getPosition:d=>d.position,getText:d=>d.text,getColor:p.label,getSize:12,sizeUnits:'pixels',fontFamily:'Plex, monospace',characterSet:'auto',fontSettings:{sdf:true},getTextAnchor:d=>d.anchor??'start',getAlignmentBaseline:'bottom',getPixelOffset:[5,-5],pickable:false})
  ];
}
const widthCache=new WeakMap();
/** Base geometry with the active colour mode and, in Power flow, |P| widths; no geometry is copied. */
function resultData(key){
 const base=fixture.results?modeData(fixture,fixture.results,resultMode)[key]??fixture[key]:fixture[key];
 if(!base||!fixture.results||flowMode==='none'||!$('flow-width').checked||key==='nodeData')return base;
 if(!widthCache.has(base)){const w=flowWidths(fixture,fixture.results);widthCache.set(base,{...base,attributes:{...base.attributes,getWidth:{value:key==='lineData'?w.full:w.compact,size:1}}});}
 return widthCache.get(base);
}
function resultWarnings(){
 if(!fixture?.results||camera.zoom<-.2||resultMode==='none')return [];
 if(family(resultMode)==='voltage'){
  const rect=$('canvas-wrap').getBoundingClientRect(),scale=2**camera.zoom,x0=camera.target[0]-rect.width/2/scale,y0=camera.target[1]-rect.height/2/scale,x1=x0+rect.width/scale,y1=y0+rect.height/scale,data=[];let visited=0;
  if((Math.floor(x1/128)-Math.floor(x0/128)+1)*(Math.floor(y1/128)-Math.floor(y0/128)+1)>4096)return [];
  for(let y=Math.floor(y0/128);y<=Math.floor(y1/128);y++)for(let x=Math.floor(x0/128);x<=Math.floor(x1/128);x++)for(const i of fixture.cells.get(`${x},${y}`)??[]){if(++visited>4096||data.length>=24)return data;if(voltageState(fixture.results.buses[i])==='violation')data.push({position:fixture.positions.subarray(i*3,i*3+3),text:'! V'});}
  return data;
 }
 return flowScope.anchors.filter(a=>a.side===0&&loadingState(fixture.results.branches[a.b])==='violation').slice(0,24).map(a=>({position:a.position,text:'! '+fixture.results.branches[a.b].loading.toFixed(1)+'%'}));
}
function flowSeconds(now=performance.now()){
 if(flowMode==='motion'&&!document.hidden&&flowScope.anchors.length){if(flowStamp!==null)flowElapsed+=(now-flowStamp)/1000*playback;flowStamp=now;}else flowStamp=null;
 return flowElapsed;
}
function refreshResults(){
 cancelAnimationFrame(flowRaf);flowRaf=0;flowData=[];flowScope={anchors:[],stats:{}};
 if(!fixture?.results)return;
 const start=performance.now(),rect=$('canvas-wrap').getBoundingClientRect();
 if(flowMode!=='none'||family(resultMode)==='loading')flowScope=visibleAnchors(fixture.results,camera,rect.width,rect.height);
 scopeMs=Math.max(scopeMs,performance.now()-start);
 if(flowMode!=='none')flowData=flowGlyphs(fixture.results,flowScope.anchors,camera.zoom,flowSeconds(),{p:$('flow-p').checked,q:$('flow-q').checked});
 flowPeak=Math.max(flowPeak,flowData.length);
 if(flowMode==='motion'&&flowData.length&&!document.hidden){
  function tick(now){if(!fixture?.results||flowMode!=='motion'||document.hidden)return;flowData=flowGlyphs(fixture.results,flowScope.anchors,camera.zoom,flowSeconds(now),{p:$('flow-p').checked,q:$('flow-q').checked});flowPeak=Math.max(flowPeak,flowData.length);render();flowRaf=requestAnimationFrame(tick);}
  flowRaf=requestAnimationFrame(tick);
 }
 if(!flowData.length)flowStamp=null;
 $('flow-hint').textContent=flowMode==='none'?'Hover for values. Click equipment to pin details and link the voltage profile.':flowData.length?`${flowData.length} local P/Q markers · hover wires for both end values · click to link the voltage profile.`:'Zoom in or choose Power flow / Highest loading to see local P/Q markers. Overview keeps the diagram quiet.';
 if(ready)render();
}
function legendMarkup(){
 const bar=(stops,lo,hi,note)=>`<span class="legend-ramp"><span>${lo}</span><i style="background:${gradient(stops)}"></i><span>${hi}</span></span> ${note}`;
 if(resultMode==='loading-heat')return 'Loading '+bar(SEQUENTIAL_STOPS,'0 %','≥ 100 %','· red ! &gt; 100 % · grey: no rating');
 if(resultMode==='voltage-band')return 'Voltage '+bar(DIVERGING_STOPS,'min','max','declared band, worst pair · ! outside · grey: no limits');
 return resultMode==='loading'?'Loading: neutral normal · amber ≥90% · red ! >100% · missing neutral':'Voltage: neutral within declared limits · red ! outside limits · missing neutral';
}
function resultStatus(){
 $('topology-legend').hidden=resultMode!=='none'&&!!fixture?.results;$('overlay-legend').hidden=!$('topology-legend').hidden;$('overlay-legend').innerHTML=legendMarkup();
 $('result-mode').value=resultMode;$('flow-mode').value=flowMode;
 const r=fixture?.results;
 $('flow-play').textContent=flowMode==='motion'?'Pause flows':'Animate flows';$('flow-play').setAttribute('aria-pressed',String(flowMode==='motion'));
 for(const b of document.querySelectorAll('[data-result-preset]'))b.setAttribute('aria-pressed',String(b.dataset.resultPreset===(family(resultMode)!=='none'?family(resultMode):flowMode==='none'?'topology':'power')));
 const scaleText=(label,max,unit)=>max>0?`${label} ${ (max*.25).toLocaleString(undefined,{maximumFractionDigits:2})} / ${max.toLocaleString(undefined,{maximumFractionDigits:2})} ${unit} → 36 / 72 px/s`:`${label}: no nonzero values`;
 $('flow-scale').textContent=r?`Fixed snapshot scales at 1×: ${scaleText('P ▸',r.flowScales.p,'kW')}; ${scaleText('Q ›',r.flowScales.q,'kvar')}. Playback ${playback}×. ${flowMode==='motion'?'Moving':'Paused / off'}.`:'';
 $('result-status').textContent=r?r.label+(r.kind==='solved'?` · ${r.provenance.iterations} iterations · saved ${r.provenance.created}`:' · values are deliberately nonphysical'):fixture?.resultError??'No staged results for this dataset. Run stage-results.mjs for the French feeder and SAPN rendering stress test.';
 $('result-status').classList.toggle('stress',r?.kind==='stress');
 $('result-watermark').textContent=r&&(resultMode!=='none'||flowMode!=='none')?(r.kind==='stress'?'ARTIFICIAL STRESS · NOT A SOLUTION':'SOLVED · Tellegen MC PF')+' · '+(resultMode==='none'?'P/Q flows':resultMode==='loading-heat'?'loading: continuous, % of rating':resultMode==='voltage-band'?'voltage: position in declared band':resultMode==='loading'?'loading: ≥90% amber · >100% red !':'voltage: declared pair limits · red = violation'):'';
 $('result-inspector').textContent=selection?resultDescription(r,selection.kind,selection.index):'Select a bus or branch to inspect exact results.';
 $('result-inspector').closest('details').hidden=!selection;
 $('mc-inspector').innerHTML=selection&&r&&fixture.refs?inspectorMarkup(r,selection.kind,selectionBus(fixture,selection.kind,selection.index),selection.index):'';
 $('result-banner').textContent=fixture?.resultError?`Results not shown · ${fixture.resultError}`:'';$('result-banner').hidden=!fixture?.resultError;
 $('pinned-result').textContent=selection&&fixture.refs?hoverDescription(fixture,selection.kind,selection.index):'Click the diagram to pin equipment results.';
}
function render(){deck.setProps({layers:layers(),parameters:{clearColor:palette[theme].bg}});}
function scheduleLabels(){clearTimeout(labelTimer);labelTimer=setTimeout(updateLabels,90);}
/** Result values where they matter most for the current view family; bounded scan and budget. */
function valueLabels(){
 const r=fixture.results,fam=family(resultMode)!=='none'?family(resultMode):flowMode!=='none'?'power':null;
 if(!r||!fam||!$('value-labels').checked||camera.zoom<MIN_LABEL_ZOOM)return [];
 const rect=$('canvas-wrap').getBoundingClientRect(),scale=2**camera.zoom,x0=camera.target[0]-rect.width/2/scale,y0=camera.target[1]-rect.height/2/scale,x1=x0+rect.width/scale,y1=y0+rect.height/scale;
 const screen=(px,py)=>[(px-x0)*scale,(py-y0)*scale],selectedBus=selection?selectionBus(fixture,selection.kind,selection.index):-1,candidates=[];
 if(fam==='voltage'){
  if((Math.floor(x1/128)-Math.floor(x0/128)+1)*(Math.floor(y1/128)-Math.floor(y0/128)+1)>4096)return [];
  let visited=0;
  outer:for(let y=Math.floor(y0/128);y<=Math.floor(y1/128);y++)for(let x=Math.floor(x0/128);x<=Math.floor(x1/128);x++)for(const i of fixture.cells.get(`${x},${y}`)??[]){
   if(++visited>4096)break outer;const px=fixture.positions[i*3],py=fixture.positions[i*3+1];if(px<x0||px>x1||py<y0||py>y1)continue;
   const c=busCandidate(r,i);if(c)candidates.push({...c,position:[px,py,0],screen:screen(px,py),selected:i===selectedBus});
  }
 }else{
  const seen=new Set();
  for(const a of visibleAnchors(r,camera,rect.width,rect.height).anchors){if(seen.has(a.b))continue;seen.add(a.b);
   if(flowMode!=='none'&&selection?.kind==='branch'&&selection.index===a.b)continue;
   const c=branchCandidate(r,a.b,fam);if(c)candidates.push({...c,position:a.position,screen:screen(a.position[0],a.position[1]),selected:selection?.kind==='branch'&&selection.index===a.b});}
 }
 return placeLabels(candidates,{width:rect.width,height:rect.height});
}
function updateLabels(){
  if(!fixture)return;labelData=[];
  // Selection ID remains discoverable even when automatic labels are off.
  const reserved=selection?.kind==='bus'?selection.index:null;
  if(selection?.kind==='attachment'&&fixture.equipment){const owner=fixture.manifest.branches+selection.index,i=fixture.equipment.glyphIndex.get(owner)??-1;if(i>=0)labelData.push({position:[fixture.equipmentCenters[i*3],fixture.equipmentCenters[i*3+1]-16,0],text:fixture.refs.attachments[selection.index].id,anchor:'middle'});}
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
  valueData=valueLabels();
  $('draw-count').textContent=`${fmt(fixture.manifest.count)} buses · ${fmt(compactOpen()?fixture.openTies.compact.segments.length:fixture.manifest.segments)} base wire segments${compactOpen()?' + '+fixture.openTies.markers.length+' open endpoint rings':''} + ${overlay.length} selection segments${fixture.busbarData?' + '+fmt(fixture.manifest.count)+' busbars':''}${fixture.equipment?' + '+fmt((compactOpen()?fixture.equipment.compact:fixture.equipment.full).owners.length)+' equipment strokes':''} submitted · ${labelData.length} labels`;
  render();
}
function select(kind,index,started=performance.now()){
  if(run||loading)return;if(selection?.kind!==kind||selection?.index!==index)traceRoot=false;selection={kind,index};pendingSelection=started;
  refreshOverlay();meshCursor=circuit?meshMatches.indexOf(index):-1;showMeshPage();
  $('next-colocated').hidden=true;
  if(fixture.refs){
    const refs=fixture.refs,row=kind==='bus'?refs.buses[index]:kind==='attachment'?refs.attachments[index]:refs.branches[index],source=refs.sources[row.source];
    let content=`<h2>${escape(row.id)}</h2><p>${escape(source.label)} · ${escape(kind==='bus'?'bus':row.family+(row.kind?' / '+row.kind:''))}</p>`;
    content+=`<p class="fine">Source identity: <code>${escape(sourceKey(source.id,kind==='bus'?'bus':row.family,row.id,row.kind??''))}</code></p>`;
    if(kind==='bus'){
      if(fixture.parent){const path=rootPath(index),component=fixture.manifest.layout.components[fixture.component[index]];content+=`<p>Depth ${fixture.depth[index]} · layout root ${escape(busName(path.root))}<br>${escape(component.rootReason)} · ${path.edges.length} steps to root${traceRoot?' · root path highlighted':''}</p>`;}
      const members=fixture.locations.get(locationKey(fixture,index));
      content+=`<p>${fixture.offsets[index+1]-fixture.offsets[index]} incident branches · terminals ${escape(row.terminals.join(', '))}<br>Grounded: ${escape(row.grounded.join(', ')||'none declared')}<br>Longitude / latitude: ${row.coordinates?.join(', ')??'unavailable in source'}<br>${members.length} buses at this display position</p><p class="fine">Find: <code>${escape(busQuery(source.id,row.id))}</code></p>`;
      const attachments=fixture.attachmentsByBus.get(index)??[];
      content+=`<details><summary>${attachments.length} source attachments</summary>${attachments.slice(0,100).map(a=>`<p>${escape(a.family)} / ${escape(a.id)} · terminals ${escape(a.terminalMap.join(', '))}${a.disabled?' · disabled':''}</p>`).join('')}${attachments.length>100?'<p>First 100 shown; all retained in reference data.</p>':''}</details>`;
      $('next-colocated').hidden=members.length<2;
    }else if(kind==='attachment')content+=`<p>Bus: ${escape(busName(row.bus))}<br>Terminal map: ${escape(row.terminalMap.join(', '))}<br>Configuration: ${escape(row.configuration??'not specified')}<br>${row.disabled?'Out of service':'Enabled'}</p><p>Bus-owned equipment attachment; individual conductor connections are not expanded.${row.family==='shunt'?' General shunt symbol: no capacitive subtype or earth bond is inferred.':''}</p>`;
    else content+=`<p>${escape(busName(row.from))} → ${escape(busName(row.to))}<br>Terminal maps: ${escape(row.terminalMapFrom.join(', '))} → ${escape(row.terminalMapTo.join(', '))}<br>${row.family==='switch'?`${row.openSwitch?'Open · display gap':'Closed'} · ${row.outOfService?'out of service · display gap':'in service'}<br>Imported state: ${row.sourceOpenSwitch?'open':'closed'} · Scenario: ${escape(fixture.manifest.scenario?.label??'imported state')}`:row.disabled?'Out of service · display gap':'Enabled'}${fixture.positions[row.from*3]===fixture.positions[row.to*3]&&fixture.positions[row.from*3+1]===fixture.positions[row.to*3+1]?' · coincident endpoints, display detour':''}${fixture.branchRoles?'<br>Layout role: '+['tree edge','non-tree connection','inactive link (excluded from display tree)'][fixture.branchRoles[index]]:''}</p><p>${row.family==='transformer'?'Equipment association; does not imply interwinding conductor continuity.':row.family==='switch'?'Switch connection; terminal order preserved. State applies to the whole switch, not individual poles.':'Conductor bundle; terminal order preserved.'}</p>`;
    if(openSelected())content+='<p class="fine">A / B mark this open switch’s endpoints. Only its selected route is restored in compact view; the gap remains. It is excluded from the enabled display tree.</p>';
    if(isInspecting())content+=`<div class="circuit-description"><p><strong>A</strong> · ${escape(busName(circuit.endpoints[0]))}<br><strong>B</strong> · ${escape(busName(circuit.endpoints[1]))}</p><p>${circuit.connected?`${circuit.edges.length} tree branches connect the endpoints via common ancestor ${escape(busName(circuit.ancestor))}.`:'Endpoints belong to different display components; no connecting tree path.'}</p><p class="fine">Cyan: selected connection · amber: connecting tree path. This is a circuit in the display spanning forest, not a power-flow or conductor-continuity result. Crossings are not junctions.</p></div>`;
    $('selection').innerHTML=content;
  }else if(kind==='bus'){
    const degree=fixture.offsets[index+1]-fixture.offsets[index];
    $('selection').innerHTML=`<h2>${busName(index)}</h2><p>Region ${Math.floor(index/fixture.manifest.regionSize)+1} · ${degree} incident branches<br>Stable synthetic source: bus/${index+1}</p>`;
  }else{
    const a=fixture.endpoints[index*2],b=fixture.endpoints[index*2+1];
    $('selection').innerHTML=`<h2>Branch ${index+1}</h2><p>${busName(a)} → ${busName(b)}<br>${fixture.states[index]?'Open tie · explicit gap':'Closed branch'} · source branch/${index+1}</p>`;
  }
  if(fixture.refs)syncUrl({sel:entityKey(fixture.refs,kind,index)});
  issueCursor=fixture.issueIndex?.get(kind+':'+index)??issueCursor;markIssue();tableView.setSelection(kind,index);
  if(fixture.results){if(!profileNavigating){profileTarget=selectionBus(fixture,kind,index);profileHover=null;}updateProfile();}resultStatus();refreshResults();showOpenPage();controls();$('trace-root').textContent=traceRoot?'Show incident branches':'Trace to layout root';updateLabels();
}
async function load(id,mode=layoutMode,routing=routingMode){
  const previous=fixture,keep=previous?.manifest.id===id&&selection?{...selection}:null;
  const morphFrom=previous?.manifest.id===id?{positions:previous.positions,refsSha256:previous.manifest.refsSha256,camera:{target:[...camera.target],zoom:camera.zoom}}:null;
  if(previous)viewStates.set(viewKey(previous.manifest),{...camera,target:[...camera.target]});
  layoutMode=mode;routingMode=routing;traceRoot=false;pendingRestoreSelection=keep;
  stopRun('dataset changed');const token=++request;abort?.abort();abort=new AbortController();loading=true;pendingLoad=null;selection=null;refreshOverlay();meshMatches=[];meshCursor=-1;showMeshPage();labelData=[];pendingSelection=null;selectionMetric=null;clearTimeout(labelTimer);controls();
  cancelAnimationFrame(flowRaf);flowStamp=null;flowElapsed=0;profile=null;profileTarget=null;profileSegments=[];profileHover=null;voltageCursor=-1;fixture=null;updateProfile();render();$('next-colocated').hidden=true;
  $('test-status').textContent='Ready';$('status').hidden=false;$('status').textContent='Loading dataset…';$('frame-time').textContent='Not measured';$('frame-detail').textContent='Run the six-second camera test';$('selection').innerHTML='<h2>Follow a connection.</h2><p>Select something on the diagram, an issue, or press <kbd>/</kbd> to search.</p>';$('selection-time').textContent='Selection feedback: not measured';
  for(const b of document.querySelectorAll('[data-size]'))b.setAttribute('aria-pressed',String(`synthetic-${b.dataset.size}`===id));
  $('dataset').value=id;const start=performance.now();
  try{
    const entry=entries.find(m=>m.id===id);if(!entry)throw new Error('Dataset unavailable; stage the external sources first');
    const actualMode=entry.kind==='bmopf'&&entry.layouts?.[mode]?mode:entry.kind==='bmopf'?(entry.sourceCoordinatesAvailable===false?'tidy-tree':'source'):'synthetic';
    if(entry.kind==='bmopf')layoutMode=actualMode;
    const variant=actualMode+(routing==='current'?'':'-'+routing);
    const geometry=entry.layouts?.[variant]??entry.layouts?.[actualMode];
    const actualRouting=geometry?(geometry.routingMode??'current'):'unavailable';
    const m={...entry,...(geometry??{}),layoutMode:actualMode,routingMode:actualRouting};
    for(const b of document.querySelectorAll('[data-routing]'))b.setAttribute('aria-pressed',String(b.dataset.routing===actualRouting));
    const reuse=previous?.manifest.id===id&&previous.manifest.refsSha256===m.refsSha256?previous.refs:null;
    for(const b of document.querySelectorAll('[data-layout]'))b.setAttribute('aria-pressed',String(b.dataset.layout===actualMode));
    // Workbench shortcut between the geographic reference and the schematic, when both exist.
    const quick=m.kind==='bmopf'&&m.sourceCoordinatesAvailable!==false&&!!entry.layouts?.['tidy-tree'];$('layout-quick').hidden=!quick;
    for(const b of document.querySelectorAll('[data-layout-quick]'))b.setAttribute('aria-pressed',String(b.dataset.layoutQuick==='source'?actualMode==='source':actualMode!=='source'));
    const base=m.kind==='bmopf'?'local-data/':'fixtures/';
    async function fetchBytes(file){const r=await fetch(base+file,{signal:abort.signal});if(!r.ok)throw new Error(`Dataset HTTP ${r.status}; regenerate or stage fixtures`);return r.arrayBuffer();}
    const resultMeta=resultCatalog[id];
    let resultError=null;
    const [buffer,refBuffer,resultBuffer]=await Promise.all([fetchBytes(m.file),m.refsFile&&!reuse?fetchBytes(m.refsFile):null,resultMeta?fetchBytes(resultMeta.file).catch(e=>{resultError='Results unavailable: '+e.message;return null;}):null]);
    const fetched=performance.now();if(token!==request)return;
    async function verify(bytes,expected){const digest=await crypto.subtle.digest('SHA-256',bytes);if(Array.from(new Uint8Array(digest),x=>x.toString(16).padStart(2,'0')).join('')!==expected)throw new Error('Dataset hash mismatch; finish staging and reload');}
    await verify(buffer,m.sha256);if(refBuffer)await verify(refBuffer,m.refsSha256);if(token!==request)return;
    const next=decodeFixture(m,buffer);
    if(refBuffer||reuse){next.refs=reuse??JSON.parse(new TextDecoder().decode(refBuffer));validateReferences(next);}
    if(m.coordinateSpace==='schematic')validateSchematic(next);
    const decoded=performance.now();prepare(next);
    if(resultBuffer)try{await verify(resultBuffer,resultMeta.sha256);next.results=prepareResults(next,bindResults(JSON.parse(new TextDecoder().decode(resultBuffer)),next.refs));next.results.issues=resultIssues(next.results);next.ownedBytes+=next.results.arrayBytes;m.resultSha256=resultMeta.sha256;}catch(e){resultError='Results rejected: '+e.message;}
    if(token!==request)return;
    if(next.refs){if(!searchCache.has(next.refs))searchCache.set(next.refs,buildSearchIndex(next.refs));next.search=searchCache.get(next.refs);}
    if(next.results){next.issues=rankIssues(next.results);next.summary=snapshotSummary(next.results,next.issues);next.issueIndex=new Map(next.issues.map((x,i)=>[x.target.kind+':'+x.target.index,i]));}
    next.resultError=resultError;const prepared=performance.now();
    fixture=next;flowPeak=0;scopeMs=0;issueCursor=-1;resultStatus();renderIssues();renderKpis();setDockTab(dockTab);openScope=null;showOpenPage();if(previous?.manifest.id!==id)$('mesh-query').value='';filterMesh();pendingLoad={...identity(m),bytes:m.bytes,refsBytes:m.refsBytes??0,referenceCacheHit:!!reuse,resultBytes:resultMeta?.bytes??0,resultError,start,fetchMs:fetched-start,decodeAndValidateMs:decoded-fetched,prepareMs:prepared-decoded,openPresentationPrepareMs:next.openPrepareMs,submittedAt:prepared,environment:environment()};
    $('network-count').textContent=fmt(m.count);$('network-detail').textContent=m.kind==='bmopf'?`${fmt(m.lines)} lines · ${fmt(m.transformers)} transformer objects${m.switches?' · '+fmt(m.switches)+' switches':''}`:`${fmt(m.branches)} branches · ${fmt(m.openTies)} open ties`;
    $('memory').textContent=`${(fixture.ownedBytes/1048576).toFixed(2)} MiB`;$('memory-detail').textContent=`${(m.bytes/1048576).toFixed(2)} MiB geometry + ${((m.refsBytes??0)/1048576).toFixed(2)} MiB reference JSON${next.openArrayBytes?' + '+(next.openArrayBytes/1048576).toFixed(2)+' MiB compact arrays':''}${resultMeta?' + '+(resultMeta.bytes/1048576).toFixed(2)+' MiB result JSON (not included in arrays)':''}; excludes JS objects`;
    $('canvas-title').textContent=m.label+' / '+({'schematic':'Leaf order','tidy-tree':'Tidy tree','source':'Source coordinates'}[actualMode]??actualMode)+(actualRouting==='unavailable'?'':' / '+({'corridor':'Fan-out + lanes','crossing-aware':'Crossing-aware lanes','equipment':'Equipment + clearance','current':'Current routes'}[actualRouting]));$('source-detail').textContent=m.kind==='bmopf'?`${m.placement}. ${m.coordinateSpace==='schematic'?'Diagram units; coordinates computed from topology, independent of geography.':'Projected geographic coordinates in metres.'} ${fmt(m.coincidentBuses)} additional co-located buses. `+m.sources.map(s=>s.label+': '+s.note).join(' '):'Synthetic schematic geometry; no electrical study data.';
    $('layout-summary').textContent=m.layout?`${fmt(m.layout.treeEdges)} tree edges · ${fmt(m.layout.crossLinks)} non-tree connections · ${m.layout.components.length} components · ${ms(m.layout.totalMs)} offline layout + routing.${m.routingMode!=='current'?' '+m.routing.maxLanes+' peak corridor lanes.':''}${m.routing.ordering?' Lane-order crossings '+fmt(m.routing.ordering.before)+' → '+fmt(m.routing.ordering.after)+' in eligible corridors.':''} Roots choose layout direction, not power flow.`:actualMode==='source'?'Source-coordinate reference. Choose Leaf order or Tidy tree to inspect the network structure.':'Precomputed synthetic schematic.';
    $('scenario-detail').textContent=m.scenario?`${m.scenario.label}: ${m.openSwitches} open / ${m.closedSwitches} closed switches; ${m.outOfServiceSwitches} out of service. ${m.scenario.changedFromSource} states changed from the imported BMOPF. ${m.scenario.policy}${m.sourceCoordinatesAvailable===false?' Source coordinates unavailable; schematic views only.':''}`:'';
    $('equipment-summary').textContent=m.equipment?`${fmt(m.equipment.glyphs)} equipment symbols · ${fmt(m.equipment.strokes)} batched strokes. Inline slots and attachment space reserved before routing. Click a symbol or find its exact source ID. Shunts use a general Y block; lines remain conductor bundles.`:'Choose Equipment + clearance in a schematic layout to show source-owned equipment symbols.';
    syncUrl({size:null,dataset:id,layout:actualMode==='synthetic'?'schematic':actualMode,routing:routingMode,open:openMode,sel:null});
    const restored=viewStates.get(viewKey(m));
    let nextCamera=restored??fitCamera(),target=keep;
    // A shared URL restores its selection by source identity and its camera once, on the first matching load.
    if(initialView&&initialView.dataset===id){const entry=next.search?.byKey.get(initialView.sel);if(entry)target=pendingRestoreSelection={kind:entry.kind,index:entry.index};if(initialView.camera)nextCamera={...nextCamera,...initialView.camera};}
    if(target&&!(initialView?.dataset===id&&initialView.camera)){const i=next.refs?selectionBus(next,target.kind,target.index):target.kind==='bus'?target.index:next.endpoints[target.index*2];nextCamera={...nextCamera,target:[next.positions[i*3],next.positions[i*3+1],0],zoom:restored?.zoom??.6};}
    initialView=null;
    setCamera(nextCamera,false);startMorph(morphFrom);updateLabels();
  }catch(e){if(e.name!=='AbortError'&&token===request)error(e.message);}
}

function afterRender(){
  const now=performance.now();let restore=null;
  if(pendingLoad){
    const m=pendingLoad;pendingLoad=null;const result={...m,firstDrawMs:now-m.start,submitToDrawMs:now-m.submittedAt};delete result.start;delete result.submittedAt;report.loads.push(result);
    $('load-time').textContent=ms(result.firstDrawMs);$('load-detail').textContent=`Fetch ${ms(m.fetchMs)} · decode/validate ${ms(m.decodeAndValidateMs)} · prepare ${ms(m.prepareMs)} · submit → draw ${ms(result.submitToDrawMs)}`;
    $('status').hidden=true;loading=false;controls();
    if(pendingRestoreSelection){restore=pendingRestoreSelection;pendingRestoreSelection=null;}
  }
  if(pendingSelection!==null){selectionMetric=now-pendingSelection;pendingSelection=null;report.selections.push({dataset:fixture.manifest.id,layoutMode:fixture.manifest.layoutMode,routingMode:fixture.manifest.routingMode,kind:selection?.kind,index:selection?.index,feedbackMs:selectionMetric,inspection:environment().inspection});$('selection-time').textContent=`Selection → draw callback: ${ms(selectionMetric)}`;}
  if(run){if(run.last!==null)run.intervals.push(now-run.last);run.last=now;}
  if(restore)select(restore.kind,restore.index);
}
function percentile(values,p){const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.min(sorted.length-1,Math.floor((sorted.length-1)*p))]??null;}
function stopRun(reason='cancelled'){
  if(!run)return;const r=run;run=null;cancelAnimationFrame(r.raf);
  const valid=reason==='complete'&&r.intervals.length>=30;
  const result={...r.dataset,resultsEnd:environment().results,started:r.started,durationMs:performance.now()-r.start,complete:valid,reason,environment:r.environment,frames:r.intervals.length,intervalsMs:r.intervals,
    p50Ms:valid?percentile(r.intervals,.5):null,p95Ms:valid?percentile(r.intervals,.95):null,maxMs:valid?Math.max(...r.intervals):null,selectionFeedbackMs:selectionMetric};
  report.runs.push(result);$('test-status').textContent=valid?'Complete':`Stopped: ${reason}`;
  if(valid){$('frame-time').textContent=ms(result.p95Ms);$('frame-detail').textContent=`p50 ${ms(result.p50Ms)} · max ${ms(result.maxMs)} · ${result.frames} intervals`;}
  if(!valid){$('frame-time').textContent='Not measured';$('frame-detail').textContent=`Incomplete: ${reason}`;}
  controls();setCamera(r.original);updateLabels();
}
function benchmark(){
  flowPeak=0;scopeMs=0;
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
function showTheme(){document.documentElement.dataset.theme=theme;for(const x of document.querySelectorAll('button[data-theme]'))x.setAttribute('aria-pressed',String(x.dataset.theme===theme));}
for(const b of document.querySelectorAll('button[data-theme]'))b.addEventListener('click',()=>{theme=b.dataset.theme;showTheme();syncUrl({theme});colour(fixture);
  // Theme changes retire base layers in this baseline; production should patch colours.
  if(fixture.compactLineData)fixture.compactLineData={...fixture.compactLineData,attributes:{...fixture.compactLineData.attributes}};fixture.lineData={...fixture.lineData,attributes:{...fixture.lineData.attributes}};fixture.nodeData={...fixture.nodeData,attributes:{...fixture.nodeData.attributes}};if(fixture.equipment)for(const d of new Set([fixture.equipment.full,fixture.equipment.compact]))d.lines={...d.lines,attributes:{...d.lines.attributes}};render();});
for(const b of document.querySelectorAll('[data-layout]'))b.addEventListener('click',()=>load(fixture.manifest.id,b.dataset.layout));
for(const b of document.querySelectorAll('[data-layout-quick]'))b.addEventListener('click',()=>{const target=b.dataset.layoutQuick==='source'?'source':layoutMode==='source'?'tidy-tree':layoutMode;if(fixture&&target!==layoutMode)load(fixture.manifest.id,target);});
document.querySelectorAll('[data-routing]').forEach(b=>b.addEventListener('click',()=>load(fixture.manifest.id,layoutMode,b.dataset.routing)));
document.querySelectorAll('[data-open-mode]').forEach(b=>b.addEventListener('click',()=>setOpenMode(b.dataset.openMode)));
$('open-results').addEventListener('change',()=>{if($('open-results').value!=='')chooseOpen(Number($('open-results').value));});
$('open-prev').addEventListener('click',()=>chooseOpen(openCursor<0?openMatches().length-1:openCursor-1));
$('open-next').addEventListener('click',()=>chooseOpen(openCursor+1));
$('open-all').addEventListener('click',()=>{openScope=null;showOpenPage();});
$('focus-source').addEventListener('click',focusSource);
$('trace-root').addEventListener('click',()=>{traceRoot=!traceRoot;select('bus',selection.index);
 if(traceRoot){const path=rootPath(selection.index),indices=[selection.index,...path.edges.flatMap(b=>[fixture.endpoints[b*2],fixture.endpoints[b*2+1]])];let x0=Infinity,y0=Infinity,x1=-Infinity,y1=-Infinity;
 for(const i of indices){x0=Math.min(x0,fixture.positions[i*3]);x1=Math.max(x1,fixture.positions[i*3]);y0=Math.min(y0,fixture.positions[i*3+1]);y1=Math.max(y1,fixture.positions[i*3+1]);}
 const r=$('canvas-wrap').getBoundingClientRect();setCamera({...camera,target:[(x0+x1)/2,(y0+y1)/2,0],zoom:Math.min(1.5,Math.log2(Math.min((r.width-100)/(x1-x0+100),(r.height-100)/(y1-y0+100))))});}
});
$('mesh-search').addEventListener('submit',e=>{e.preventDefault();if(!loading&&!run)filterMesh();});
$('mesh-results').addEventListener('change',()=>{if($('mesh-results').value!=='')chooseMesh(Number($('mesh-results').value));});
$('mesh-prev').addEventListener('click',()=>chooseMesh(meshCursor<0?meshMatches.length-1:meshCursor-1));
$('mesh-next').addEventListener('click',()=>chooseMesh(meshCursor+1));
$('inspect-circuit').addEventListener('change',()=>{
  if($('inspect-circuit').checked&&!circuit){chooseMesh(meshCursor<0?0:meshCursor);return;}
  if(selection)select(selection.kind,selection.index);else {controls();render();}
});
for(const id of ['show-tree-path','dim-context'])$(id).addEventListener('change',()=>{pendingSelection=performance.now();render();});
$('fit-connection').addEventListener('click',()=>fitInspection(false));
$('fit-circuit').addEventListener('click',()=>fitInspection(true));
$('dataset').addEventListener('change',()=>load($('dataset').value));
$('next-colocated').addEventListener('click',()=>{if(selection?.kind!=='bus')return;const members=fixture.locations.get(locationKey(fixture,selection.index));select('bus',members[(members.indexOf(selection.index)+1)%members.length]);});
$('labels').addEventListener('change',updateLabels);$('value-labels').addEventListener('change',updateLabels);$('edge-picking').addEventListener('change',render);
$('fit').addEventListener('click',()=>setCamera(fitCamera()));$('detail').addEventListener('click',()=>setCamera(detailCamera()));
$('clear').addEventListener('click',clearSelection);
function clearSelection(){selection=null;syncUrl({sel:null});issueCursor=-1;markIssue();tableView.setSelection(null,-1);traceRoot=false;refreshOverlay();profile=null;profileTarget=null;profileSegments=[];profileHover=null;updateProfile();resultStatus();meshCursor=-1;showMeshPage();openScope=null;showOpenPage();controls();$('next-colocated').hidden=true;$('selection').innerHTML='<h2>Follow a connection.</h2><p>Select something on the diagram, an issue, or press <kbd>/</kbd> to search.</p>';updateLabels();}
$('benchmark').addEventListener('click',benchmark);$('stop').addEventListener('click',()=>stopRun());$('export-report').addEventListener('click',exportReport);
document.addEventListener('visibilitychange',()=>{if(document.hidden)stopRun('tab hidden');refreshResults();});
window.addEventListener('resize',()=>{stopRun('viewport resized');scheduleLabels();});
window.addEventListener('pagehide',()=>{abort?.abort();clearTimeout(labelTimer);cancelAnimationFrame(flowRaf);deck?.finalize();});
$('network-canvas').addEventListener('pointerdown',()=>{if(run)stopRun('user interaction');});
function changeResults(mode=resultMode,flows=flowMode){
 flowSeconds();flowStamp=null;resultMode=mode;flowMode=flows;
 syncUrl({results:resultMode,flows:flowMode});
 resultStatus();refreshResults();controls();updateLabels();
}
function focusResultBranch(b){
 if(!(b>=0))return;const anchor=fixture.results.anchors.find(a=>a.b===b);if(!anchor)return;
 select('branch',b);setCamera({...camera,target:anchor.position,zoom:1.1});
}
function focusVoltage(){
 const indices=fixture?.results?.issues.voltage??[];if(!indices.length)return;
 voltageCursor=(voltageCursor+1)%indices.length;const i=indices[voltageCursor];changeResults(inFamily('voltage'),flowMode==='none'?'static':flowMode);
 select('bus',i);setCamera({...camera,target:Array.from(fixture.positions.subarray(i*3,i*3+3)),zoom:1.1});
}
for(const id of ['result-mode','flow-mode'])$(id).addEventListener('change',()=>changeResults($('result-mode').value,$('flow-mode').value));
for(const b of document.querySelectorAll('[data-result-preset]'))b.addEventListener('click',()=>{
 if(!fixture?.results)return;const preset=b.dataset.resultPreset;
 if(preset==='voltage'){changeResults(inFamily('voltage'),'static');if(fixture.results.issues.voltage.length)focusVoltage();}
 if(preset==='loading'){changeResults(inFamily('loading'),'static');focusResultBranch(fixture.results.issues.highest);}
 // Direction by arrow, magnitude by width; motion is an opt-in cue, not the magnitude carrier.
 if(preset==='power'){$('flow-width').checked=true;changeResults('none','static');focusResultBranch(fixture.results.issues.power);}
 if(preset==='topology')changeResults('none','none');
});
$('flow-play').addEventListener('click',()=>changeResults(resultMode,flowMode==='motion'?'static':'motion'));
$('flow-speed').addEventListener('change',()=>{flowSeconds();playback=Number($('flow-speed').value);flowStamp=null;resultStatus();refreshResults();});
for(const id of ['flow-p','flow-q'])$(id).addEventListener('change',refreshResults);
$('flow-width').addEventListener('change',render);
function updateProfile(){
 if(!fixture?.results||profileTarget===null){profile=null;profileSegments=[];showProfileBody(false);$('profile-charts').replaceChildren();$('profile-bus').replaceChildren();$('profile-summary').textContent='Select a bus, line or equipment symbol to follow its root-to-bus display-tree path.';$('profile-readout').textContent='Hover a chart point for values; click or press Enter to locate its bus.';return;}
 profile=voltageProfile(fixture,fixture.results,profileTarget);profileSegments=branchSegments(fixture,profile.edges);showProfileBody(!profileClosed);
 $('profile-summary').textContent=(fixture.results.kind==='stress'?'ARTIFICIAL STRESS — NOT A SOLUTION. ':'')+(profile.nodes.length?`${busName(profile.root)} → ${busName(profile.target)} · ${profile.nodes.length} buses. `:'')+profile.notice+' Path uses the display spanning tree; mesh alternatives are not inferred.';
 if(fixture.envelope===undefined)fixture.envelope=feederEnvelope(fixture,fixture.results);
 $('profile-charts').innerHTML=profileMarkup(profile,fixture.refs,$('profile-envelope').checked?profile.zones.map(z=>zoneEnvelope(fixture.envelope,fixture,z)):null);
 $('profile-bus').replaceChildren();for(const i of profile.shown??[]){const option=document.createElement('option');option.value=String(i);option.textContent=busName(i);$('profile-bus').append(option);}
 const selected=selection?selectionBus(fixture,selection.kind,selection.index):profileTarget;$('profile-bus').value=String(selected);profileCursor(selected,false);
}
function profileCursor(bus,hover=true){
 if(hover)profileHover=bus;
 for(const p of $('profile-charts').querySelectorAll('[data-profile-bus]'))p.classList.toggle('linked',bus!==null&&Number(p.dataset.profileBus)===bus);
 if(bus!==null&&fixture?.results)$('profile-readout').textContent=busName(bus)+'\n'+resultDescription(fixture.results,'bus',bus);
 if(hover)render();
}
function navigateProfile(bus,focusPair=null){
 if(run||loading||!profile?.shown?.includes(bus))return;
 profileNavigating=true;try{select('bus',bus);}finally{profileNavigating=false;}
 if(focusPair!==null)($('profile-charts').querySelector(`[data-profile-bus="${bus}"][data-profile-pair="${focusPair}"]`)??$('profile-charts').querySelector(`[data-profile-bus="${bus}"]`))?.focus();
 setCamera({...camera,target:Array.from(fixture.positions.subarray(bus*3,bus*3+3)),zoom:1.1});
}
$('profile-charts').addEventListener('pointerover',e=>{const point=e.target.closest('[data-profile-bus]');if(point)profileCursor(Number(point.dataset.profileBus));});
$('profile-charts').addEventListener('pointerleave',()=>profileCursor(null));
$('profile-charts').addEventListener('focusin',e=>{const point=e.target.closest('[data-profile-bus]');if(point)profileCursor(Number(point.dataset.profileBus));});
$('profile-charts').addEventListener('click',e=>{const point=e.target.closest('[data-profile-bus]');if(point)navigateProfile(Number(point.dataset.profileBus));});
$('profile-charts').addEventListener('keydown',e=>{const point=e.target.closest('[data-profile-bus]');if(!point)return;const bus=Number(point.dataset.profileBus);if(e.key==='Enter'||e.key===' '){e.preventDefault();navigateProfile(bus,Number(point.dataset.profilePair));}else if(['ArrowLeft','ArrowRight'].includes(e.key)){e.preventDefault();const n=profile.shown.indexOf(bus)+(e.key==='ArrowLeft'?-1:1),next=profile.shown[n];if(next!==undefined){navigateProfile(next,Number(point.dataset.profilePair));}}});
$('profile-bus').addEventListener('change',()=>navigateProfile(Number($('profile-bus').value)));
for(const [id,delta] of [['profile-prev',-1],['profile-next',1]])$(id).addEventListener('click',()=>{const i=profile.shown.indexOf(Number($('profile-bus').value)),next=profile.shown[i+delta];if(next!==undefined)navigateProfile(next);});
$('profile-path').addEventListener('change',render);
$('profile-envelope').addEventListener('change',()=>{if(profile)updateProfile();});
$('profile-fit').addEventListener('click',()=>{if(!profile?.nodes.length)return;const b=overlayBounds(fixture,profile.nodes,profileSegments),rect=$('canvas-wrap').getBoundingClientRect();setCamera({...camera,target:[(b[0]+b[2])/2,(b[1]+b[3])/2,0],zoom:Math.min(1.1,Math.log2(Math.min((rect.width-100)/(b[2]-b[0]+80),(rect.height-100)/(b[3]-b[1]+80))))});});
function pickedEquipment(info){
 if(info.index<0||!fixture?.refs)return null;const id=info.layer?.id??'';
 if(id.startsWith('equipment-hit-')){const d=compactOpen()?fixture.equipment.compact:fixture.equipment.full;return equipmentRow(fixture,d.glyphOwners[info.index]);}
 if(id.startsWith('buses-')||id.startsWith('busbars-'))return {kind:'bus',index:info.index};
 if(id.startsWith('compact-branches-'))return {kind:'branch',index:compactBranch(fixture,fixture.openTies,info.index)};
 if(id.startsWith('branches-'))return {kind:'branch',index:fixture.segmentBranches[info.index]};return null;
}

// ---- Workbench: issues, KPIs, omnisearch, keyboard and docks ----
const volts=v=>Number.isFinite(v)?v.toLocaleString(undefined,{maximumFractionDigits:v>=1000?0:2}):'—';
const plural=(n,word)=>`${fmt(n)} ${word}${n===1?'':'s'}`;
/** Compact range for KPI tiles: kV above 1 kV, four significant figures; exact values stay in tooltips and the inspector. */
const range=(a,b)=>{const k=Math.max(Math.abs(a),Math.abs(b))>=1000,f=v=>Number((k?v/1000:v).toPrecision(4)).toLocaleString();return `${f(a)}–${f(b)} ${k?'kV':'V'}`;};
const zoneLabel=z=>({'phase–neutral':'V P–N','phase–phase':'V P–P'}[z]??'V '+z);
const rowOf=(kind,index)=>kind==='bus'?fixture.refs.buses[index]:kind==='branch'?fixture.refs.branches[index]:fixture.refs.attachments[index];
function issueText(x){
 const id=fixture.refs?rowOf(x.target.kind,x.target.index).id:busName(x.target.index);
 if(x.kind==='voltage')return {id,value:`${volts(x.value)} V`,detail:`${x.pair.join('–')} ${x.low?'below':'above'} ${volts(x.limit)} V by ${x.score.toFixed(2)} %`};
 const row=fixture.refs?.branches[x.target.index];
 return {id,value:`${x.value.toFixed(1)} %`,detail:`${row?row.family+' · ':''}${x.severity==='violation'?'over rating':'≥ 90 % of rating'}`};
}
function renderIssues(){
 const list=$('issue-list'),issues=fixture?.issues??[],shown=issues.slice(0,100);list.replaceChildren();
 $('issue-count').textContent=fixture?.results?`${fmt(issues.length)}`:'';
 if(!fixture?.results){$('issue-more').textContent=fixture?.resultError??'No result snapshot for this dataset.';return;}
 if(!issues.length){const li=document.createElement('li');li.className='issue-ok';li.textContent='No limit violations or loading warnings in this snapshot.';list.append(li);}
 shown.forEach((x,i)=>{const t=issueText(x),li=document.createElement('li'),b=document.createElement('button');li.className=x.severity;b.dataset.issue=String(i);b.title=`${t.id} · ${t.detail}`;
  b.innerHTML=`<span class="sev" aria-label="${x.severity}">${x.severity==='violation'?'!':'▲'}</span><span class="issue-id">${escape(t.id)}</span><span class="issue-value">${escape(t.value)}</span><span class="issue-detail">${escape(t.detail)}</span>`;
  li.append(b);list.append(li);});
 $('issue-more').textContent=(fixture.results.kind==='stress'?'Artificial stress values: these are not real issues. ':'')+(issues.length>shown.length?`Showing the worst ${shown.length} of ${fmt(issues.length)}; N walks all of them.`:'');
 markIssue();
}
function markIssue(){for(const b of $('issue-list').querySelectorAll('button[data-issue]'))b.setAttribute('aria-current',String(Number(b.dataset.issue)===issueCursor));// Scroll only the list: scrollIntoView would also scroll the page on narrow screens.
 const list=$('issue-list'),b=list.querySelector('[aria-current=true]');if(b){const top=b.offsetTop-list.offsetTop;if(top<list.scrollTop||top+b.offsetHeight>list.scrollTop+list.clientHeight)list.scrollTop=top-list.clientHeight/2;}}
function focusIssue(i){
 const issues=fixture?.issues;if(!issues?.length||loading||run)return;
 issueCursor=(i+issues.length)%issues.length;const x=issues[issueCursor];
 changeResults(inFamily(x.kind),flowMode);reveal(x.target.kind,x.target.index);
}
function kpi({label,value,sub='',cls='',target=null,title=''}){
 const el=document.createElement(target?'button':'div');el.className=`kpi ${cls}`;if(title)el.title=title;
 el.innerHTML=`<span class="kpi-label">${escape(label)}</span><span class="kpi-value">${escape(value)}</span>${sub?`<span class="kpi-sub">${escape(sub)}</span>`:''}`;
 if(target)el.addEventListener('click',target);return el;
}
function renderKpis(){
 const strip=$('kpis');strip.replaceChildren();if(!fixture)return;
 const r=fixture.results,s=fixture.summary,name=(kind,i)=>fixture.refs?rowOf(kind,i).id:busName(i);
 if(!r){strip.append(kpi({label:'Snapshot',value:'No results',sub:`${fmt(fixture.manifest.count)} buses · topology only`}));return;}
 if(r.kind==='stress')strip.append(kpi({label:'Artificial stress',value:'NOT A SOLUTION',sub:'rendering test values',cls:'stress',title:r.label}));
 const {voltage,overload,warning}=s.counts;
 strip.append(kpi({label:'Violations',value:voltage+overload?fmt(voltage+overload):'None',sub:warning?`+ ${plural(warning,'warning')}`:'within declared limits',cls:voltage+overload?'violation':warning?'warning':'',target:fixture.issues.length?()=>focusIssue(0):null,title:'Go to the worst issue (N)'}));
 strip.append(kpi({label:'Losses',value:`${volts(s.loss.p)} kW`,sub:`${volts(s.loss.q)} kvar`,title:'Sum of both into-equipment end powers over active branches'}));
 for(const z of s.zones){
  const bad=[z.min,z.max].find(e=>e.severity==='violation'),worst=bad??z.min;
  strip.append(kpi({label:zoneLabel(z.zone),value:range(z.min.volts,z.max.volts),sub:bad?'limit violated':`${fmt(z.buses)} buses in limits`,cls:bad?'violation':'',target:()=>reveal('bus',worst.bus),title:`Min at ${name('bus',z.min.bus)} (${z.min.pair.join('–')}); max at ${name('bus',z.max.bus)} (${z.max.pair.join('–')}). Click to locate ${bad?'the violation':'the minimum'}.`}));
 }
 if(s.loading)strip.append(kpi({label:'Max loading',value:`${s.loading.value.toFixed(1)} %`,sub:name('branch',s.loading.branch),cls:s.loading.value>100?'violation':s.loading.value>=90?'warning':'',target:()=>{changeResults(inFamily('loading'),flowMode);reveal('branch',s.loading.branch);},title:r.branches[s.loading.branch].basis??''}));
 if(s.unbalance)strip.append(kpi({label:'Max unbalance',value:`${s.unbalance.percent.toFixed(2)} %`,sub:s.unbalance.definition,target:()=>reveal('bus',s.unbalance.bus),title:'Magnitude-only unbalance: max deviation from the mean of three same-reference pair voltages'}));
}
/** Select and bring into view without assuming which camera the user wants beyond "visible and legible". */
function reveal(kind,index){
 if(!fixture||loading||run)return;select(kind,index);
 if(kind==='branch'&&fixture.refs){fitSelectedRoute();return;}
 const g=kind==='attachment'?fixture.equipment?.glyphIndex.get(fixture.manifest.branches+index)??-1:-1;
 const at=g>=0?fixture.equipmentCenters.subarray(g*3,g*3+3):kind==='branch'?fixture.positions.subarray(fixture.endpoints[index*2]*3,fixture.endpoints[index*2]*3+3):fixture.positions.subarray(selectionBus(fixture,kind,index)*3,selectionBus(fixture,kind,index)*3+3);
 setCamera({...camera,target:[at[0],at[1],0],zoom:Math.max(camera.zoom,g>=0?2:1.1)});
}
function hint(kind,index){
 const r=fixture.results;if(!r)return {text:'',cls:''};
 if(kind==='branch'){const b=r.branches[index],st=loadingState(b);return {text:Number.isFinite(b.loading)?`${b.loading.toFixed(1)} %`:'',cls:st};}
 const bus=r.buses[selectionBus(fixture,kind,index)],st=voltageState(bus);
 if(st==='missing')return {text:'',cls:''};
 const worst=bus.pairs.reduce((a,p)=>Math.min(p.volts-p.min,p.max-p.volts)<Math.min(a.volts-a.min,a.max-a.volts)?p:a);
 return {text:`${st==='violation'?'! ':''}${volts(worst.volts)} V`,cls:st};
}
function omniUpdate(){
 const q=$('omni').value,list=$('omni-results');omniItems=[];list.replaceChildren();
 if(!fixture){omniClose();return;}
 const add=(item,label,meta,h={text:'',cls:''})=>{const li=document.createElement('li'),i=omniItems.length;omniItems.push(item);li.id=`omni-${i}`;li.setAttribute('role','option');li.dataset.i=String(i);
  li.innerHTML=`<span class="omni-kind">${escape(item.kindLabel)}</span><span class="omni-id">${escape(label)}</span><span class="omni-hint ${h.cls}">${escape(h.text)}</span><span class="omni-meta">${escape(meta)}</span>`;list.append(li);};
 const section=text=>{const li=document.createElement('li');li.className='omni-section';li.setAttribute('role','presentation');li.textContent=text;list.append(li);};
 let total=0;
 if(!q.trim()){
  if(!fixture.issues?.length){omniClose();return;}
  section('WORST ISSUES');fixture.issues.slice(0,5).forEach((x,i)=>{const t=issueText(x);add({issue:i,kindLabel:x.kind},t.id,t.detail,{text:t.value,cls:x.severity});});
 }else if(fixture.search){
  const found=search(fixture.search,q,{limit:12});total=found.total;
  for(const m of found.matches)add({kind:m.kind,index:m.index,kindLabel:m.kind==='bus'?'bus':m.family.split(' / ')[0]},m.id,`${m.source}${m.kind==='bus'?'':' · '+m.family} · ${m.qualified}`,hint(m.kind,m.index));
 }else{const i=syntheticBus(q,fixture.manifest.count);if(i!==null){total=1;add({kind:'bus',index:i,kindLabel:'bus'},busName(i),'Synthetic fixture bus',{text:'',cls:''});}}
 if(!omniItems.length){const li=document.createElement('li');li.className='omni-empty';li.textContent=fixture.search?'No source identity contains that text.':`Synthetic buses are B000001–${busName(fixture.manifest.count-1)}.`;list.append(li);}
 else if(total>omniItems.length){const li=document.createElement('li');li.className='omni-section';li.textContent=`${fmt(total)} matches · keep typing to narrow`;list.append(li);}
 $('omni-status').textContent=q.trim()?`${fmt(total)} matches`:'';
 omniActive=omniItems.length?0:-1;list.hidden=false;$('omni').setAttribute('aria-expanded','true');omniMark();
}
function omniMark(){for(const li of $('omni-results').querySelectorAll('[role=option]'))li.setAttribute('aria-selected',String(Number(li.dataset.i)===omniActive));
 if(omniActive>=0){$('omni').setAttribute('aria-activedescendant',`omni-${omniActive}`);$(`omni-${omniActive}`)?.scrollIntoView({block:'nearest'});}else $('omni').removeAttribute('aria-activedescendant');}
function omniClose(){$('omni-results').hidden=true;$('omni').setAttribute('aria-expanded','false');omniActive=-1;}
function omniChoose(i){
 const item=omniItems[i];if(!item)return;omniClose();$('omni').value='';$('omni').blur();
 if(item.issue!==undefined)focusIssue(item.issue);else reveal(item.kind,item.index);
 $('network-canvas').focus({preventScroll:true});
}
$('omni').addEventListener('input',()=>{clearTimeout(omniTimer);omniTimer=setTimeout(omniUpdate,fixture?.search?.entries.length>20000?80:0);});
$('omni').addEventListener('focus',omniUpdate);
$('omni').addEventListener('blur',()=>setTimeout(omniClose,150));
$('omni').addEventListener('keydown',e=>{
 if(e.key==='ArrowDown'||e.key==='ArrowUp'){e.preventDefault();if(omniItems.length){omniActive=(omniActive+(e.key==='ArrowDown'?1:-1)+omniItems.length)%omniItems.length;omniMark();}}
 else if(e.key==='Enter'){e.preventDefault();clearTimeout(omniTimer);if(omniTimer)omniUpdate();omniChoose(Math.max(0,omniActive));}
 else if(e.key==='Escape'){e.preventDefault();if($('omni').value)$('omni').value='',omniUpdate();else{omniClose();$('omni').blur();}}
});
$('omni-results').addEventListener('pointerdown',e=>{const li=e.target.closest('[role=option]');if(li){e.preventDefault();omniChoose(Number(li.dataset.i));}});
$('issue-list').addEventListener('click',e=>{const b=e.target.closest('button[data-issue]');if(b)focusIssue(Number(b.dataset.issue));});
function setMode(mode){$('shell').dataset.mode=mode;for(const b of document.querySelectorAll('[data-app-mode]'))b.setAttribute('aria-pressed',String(b.dataset.appMode===mode));syncUrl({mode:mode==='lab'?'lab':null});}
for(const b of document.querySelectorAll('[data-app-mode]'))b.addEventListener('click',()=>setMode(b.dataset.appMode));
// The profile dock opens with a profile unless the user collapsed it; an empty dock never steals canvas space.
let profileClosed=false;
function showProfileBody(open){if(!open&&dockTab!=='profile')return;$('profile-body').hidden=!open;$('profile-toggle').setAttribute('aria-expanded',String(open));}
// ---- Results tables: one lazily built model per kind and snapshot; selection is shared with the diagram ----
let dockTab=['bus','branch'].includes(resultParams.get('tab'))?resultParams.get('tab'):'profile';
const provenance=()=>{const r=fixture.results;return [`Tellegen diagram workbench export ${new Date().toISOString()}`,`Dataset: ${fixture.manifest.label} (${fixture.manifest.id})`,
 r.kind==='stress'?'ARTIFICIAL STRESS VALUES: NOT A POWER-FLOW SOLUTION':`${r.label}; ${r.provenance.iterations} iterations; saved ${r.provenance.created}`,`Result sidecar SHA-256: ${fixture.manifest.resultSha256}`,
 'P/Q are positive into the equipment at each end; voltages are declared terminal-pair magnitudes'];};
const tableView=createTableView($('table-pane'),{onSelect:(kind,i)=>reveal(kind,i),csvName:kind=>`${fixture.manifest.id}-${kind==='bus'?'buses':'branches'}.csv`,csvMeta:provenance});
function setDockTab(tab){
 const available=!!(fixture?.results&&fixture.refs);if(tab!=='profile'&&!available)tab='profile';dockTab=tab;
 $('voltage-profile').dataset.tab=tab;$('table-pane').hidden=tab==='profile';
 for(const b of document.querySelectorAll('[data-dock-tab]')){b.setAttribute('aria-selected',String(b.dataset.dockTab===tab));b.disabled=b.dataset.dockTab!=='profile'&&!available;}
 if(tab==='profile'){tableView.setModel(null);showProfileBody(!!profile?.zones?.length&&!profileClosed);}
 else{fixture.tables??={};tableView.setModel(fixture.tables[tab]??=tableModel(fixture,fixture.results,tab));if(selection)tableView.setSelection(selection.kind,selection.index);profileClosed=false;$('profile-body').hidden=false;$('profile-toggle').setAttribute('aria-expanded','true');}
 syncUrl({tab:tab==='profile'?null:tab});
}
for(const b of document.querySelectorAll('[data-dock-tab]'))b.addEventListener('click',()=>setDockTab(b.dataset.dockTab));
function toggleProfile(open=$('profile-body').hidden){profileClosed=!open;$('profile-body').hidden=!open;$('profile-toggle').setAttribute('aria-expanded',String(open));}
$('profile-toggle').addEventListener('click',()=>toggleProfile());
$('help-open').addEventListener('click',()=>$('help').showModal());$('help-close').addEventListener('click',()=>$('help').close());
$('help').addEventListener('click',e=>{if(e.target===$('help'))$('help').close();});
const zoomBy=d=>{if(fixture&&!run)setCamera({...camera,zoom:Math.min(5,Math.max(-24,camera.zoom+d))});};
$('zoom-in').addEventListener('click',()=>zoomBy(.5));$('zoom-out').addEventListener('click',()=>zoomBy(-.5));
document.addEventListener('keydown',e=>{
 const typing=e.target.closest?.('input,select,textarea,[contenteditable=true]');
 if(e.key==='/'&&!typing&&!$('help').open){e.preventDefault();$('omni').focus();return;}
 if(typing||e.metaKey||e.ctrlKey||e.altKey||$('help').open||!fixture)return;
 const click=id=>{if(!$(id).disabled)$(id).click();},preset=['power','voltage','loading','topology'][Number(e.key)-1];
 if(e.key==='Escape'){if(selection)clearSelection();}
 else if(e.key==='?'){$('help').showModal();}
 else if(e.key==='n'||e.key==='N'){focusIssue(issueCursor<0?(e.shiftKey?-1:0):issueCursor+(e.shiftKey?-1:1));}
 else if(preset){document.querySelector(`[data-result-preset=${preset}]`)?.click();}
 else if(e.key==='['||e.key===']'){click(e.key==='['?'profile-prev':'profile-next');}
 else if(e.key==='f'||e.key==='F'){click('fit');}
 else if(e.key==='+'||e.key==='='){zoomBy(.5);}
 else if(e.key==='-'||e.key==='_'){zoomBy(-.5);}
 else if(e.key===' '&&!e.target.closest('button,summary,[role=button]')){click('flow-play');}
 else if(e.key==='p'||e.key==='P'){toggleProfile();}
 else return;
 e.preventDefault();
});
// ---- Export: PNG of the rendered canvas, or vector SVG of the visible view, both with a title block ----
const css=c=>`rgb(${c[0]} ${c[1]} ${c[2]})`,stamp=()=>new Date().toISOString().replace(/[:.]/g,'-').slice(0,19);
function download(blob,name){const url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
function exportTitle(){
 const m=fixture.manifest,r=fixture.results,flows=flowMode==='none'?'':` · P/Q arrows${$('flow-width').checked?', width by |P|':''}`;
 return titleBlock({dataset:m.label,layout:$('canvas-title').textContent.split(' / ').slice(1).join(' / ')||'Synthetic schematic',view:COLOUR_MODES[resultMode].label+flows,
  result:r?{kind:r.kind,label:r.label,iterations:r.provenance?.iterations,sha256:m.resultSha256}:null,camera,exported:new Date().toISOString()});
}
function exportPng(){
 if(!fixture||run)return;deck.redraw('export');
 // Read the drawing buffer in the same task as the forced draw, before the browser composites and clears it.
 const src=$('network-canvas'),k=src.width/src.clientWidth,p=palette[theme],c=document.createElement('canvas'),ctx=c.getContext('2d'),t=exportTitle(),h=TITLE_BLOCK_HEIGHT*k;
 c.width=src.width;c.height=src.height+h;ctx.fillStyle=css(p.bg.map(x=>x*255));ctx.fillRect(0,0,c.width,c.height);ctx.drawImage(src,0,0);
 ctx.strokeStyle=css(p.line);ctx.beginPath();ctx.moveTo(0,src.height+.5);ctx.lineTo(c.width,src.height+.5);ctx.stroke();
 for(const [col,lines] of [[0,t.left],[1,t.right]])lines.forEach((line,i)=>{const lead=col===0&&i===0;ctx.font=`${lead?'600 ':''}${(lead?12:11)*k}px Plex, monospace`;ctx.fillStyle=css(lead?p.label:p.line);ctx.fillText(fitText(line,src.clientWidth/2),(col*c.width/2)+12*k,src.height+(18+i*15)*k);});
 c.toBlob(b=>download(b,`${fixture.manifest.id}-${stamp()}.png`),'image/png');
}
function exportSvg(){
 if(!fixture||run)return;
 const rect=$('canvas-wrap').getBoundingClientRect(),scale=2**camera.zoom,x0=camera.target[0]-rect.width/2/scale,y0=camera.target[1]-rect.height/2/scale,p=palette[theme];
 const lines=resultData(compactOpen()?'compactLineData':'lineData'),attr=lines.attributes,sets=[{sources:attr.getSourcePosition.value,targets:attr.getTargetPosition.value,colors:attr.getColor.value,widths:attr.getWidth?.value,count:lines.length}];
 if(fixture.busbarSources)sets.push({sources:fixture.busbarSources,targets:fixture.busbarTargets,colors:new Uint8Array(fixture.manifest.count*4).map((_,i)=>p.node[i%4]),count:fixture.manifest.count,width:2});
 if(fixture.equipment){const d=compactOpen()?fixture.equipment.compact:fixture.equipment.full;sets.push({sources:d.sources,targets:d.targets,colors:d.colors,count:d.owners.length,width:1.5});}
 if(overlay.length)sets.push({sources:Float32Array.from(overlay.flatMap(o=>[...o.source])),targets:Float32Array.from(overlay.flatMap(o=>[...o.target])),colors:new Uint8Array(overlay.length*4).map((_,i)=>p.open[i%4]),count:overlay.length,width:2.5});
 const nodes=resultData('nodeData'),warn=[225,91,88],amber=[219,158,61];
 try{
  const svg=viewportSvg({bounds:[x0,y0,x0+rect.width/scale,y0+rect.height/scale],scale,width:Math.round(rect.width),height:Math.round(rect.height),background:css(p.bg.map(x=>x*255)),ink:css(p.label),muted:css(p.line),lineSets:sets,
   points:{positions:fixture.positions,colors:nodes.attributes.getFillColor.value,count:fixture.manifest.count,radius:1.5},glyphs:flowData.map(g=>({...g,color:css(g.color)})),
   markers:selection?.kind==='bus'?[{position:fixture.positions.subarray(selection.index*3,selection.index*3+3),radius:6,fill:css(p.open)}]:[],
   texts:[...labelData.map(l=>({...l,dx:5,dy:-5})),...valueData.map(l=>({...l,dx:6,dy:22,color:css(l.state==='violation'?warn:l.state==='warning'?amber:p.label)})),...resultWarnings().map(l=>({...l,dy:-28,color:css(warn)}))],title:exportTitle()});
  download(new Blob([svg],{type:'image/svg+xml'}),`${fixture.manifest.id}-${stamp()}.svg`);
 }catch(e){$('status').hidden=false;$('status').textContent=e.message;setTimeout(()=>{$('status').hidden=true;},3000);}
}
$('export-png').addEventListener('click',exportPng);$('export-svg').addEventListener('click',exportSvg);
async function start(){
 if(palette[resultParams.get('theme')]){theme=resultParams.get('theme');showTheme();}
 if(resultParams.get('mode')==='lab')setMode('lab');
 try{const response=await fetch('local-data/results-manifest.json',{cache:'no-store'});if(response.ok)resultCatalog=(await response.json()).datasets;}catch{}
  controls();const res=await fetch('fixtures/manifest.json');if(!res.ok)throw new Error('Missing fixtures: run generate.mjs');entries=(await res.json()).map(m=>({...m,id:`synthetic-${m.count}`,label:`Synthetic · ${fmt(m.count)} buses`}));
  try{const local=await fetch('local-data/manifest.json',{cache:'no-store'});if(local.ok){const catalog=await local.json();entries.push(...catalog);$('dataset-note').textContent='Local BMOPF sources staged · raw files remain external';}else if(local.status!==404)throw new Error(`HTTP ${local.status}`);}catch(e){$('dataset-note').textContent='Local catalog unavailable: '+e.message;}
  for(const m of entries){const option=document.createElement('option');option.value=m.id;option.textContent=m.label;$('dataset').append(option);}

  // Font availability is resolved before TextLayer creates its glyph atlas.
  await document.fonts.load('12px Plex');
  deck=new Deck({canvas:$('network-canvas'),parent:$('canvas-wrap'),width:'100%',height:'100%',deviceProps:{type:'webgl'},useDevicePixels:Math.min(devicePixelRatio,2),
    views:new OrthographicView({id:'schematic',flipY:true}),viewState:camera,controller:{doubleClickZoom:true},layers:[],pickingRadius:5,
    onViewStateChange:({viewState})=>{if(run)stopRun('user interaction');setCamera({...viewState,zoom:viewState.zoomX??viewState.zoom});},
    onClick:info=>{if(info.index<0||!fixture||loading||run)return;if(info.layer?.id.startsWith('equipment-hit-')){const d=compactOpen()?fixture.equipment.compact:fixture.equipment.full;const {kind,index}=equipmentRow(fixture,d.glyphOwners[info.index]);select(kind,index);}else if(info.layer?.id.startsWith('buses-')||info.layer?.id.startsWith('busbars-'))select('bus',info.index);else if(info.layer?.id.startsWith('open-endpoints-')){openScope=fixture.openTies.markers[info.index].bus;chooseOpen(0,false);}else if(info.layer?.id.startsWith('compact-branches-')){const b=compactBranch(fixture,fixture.openTies,info.index);if(b!==null)select('branch',b);}else if(info.layer?.id.startsWith('branches-'))select('branch',fixture.segmentBranches[info.index]);},
    onHover:info=>{if(loading||run)return;const hit=pickedEquipment(info),bus=hit&&hit.index!==null?selectionBus(fixture,hit.kind,hit.index):null;if(bus!==profileHover&&profile?.shown?.includes(bus))profileCursor(bus);else if(!profile?.shown?.includes(bus)&&profileHover!==null)profileCursor(null);},
    getTooltip:info=>{if(loading||run)return null;const hit=pickedEquipment(info);if(hit&&hit.index!==null)return {text:hoverDescription(fixture,hit.kind,hit.index),style:{background:theme==='paper'?'#fcfbf7':'#20262d',color:theme==='paper'?'#20242b':'#eeeae1',fontSize:'12px',maxWidth:'min(360px, 80vw)',transform:`translate(${info.x+12}px, ${info.y+12}px) ${info.x>$('canvas-wrap').clientWidth/2?'translateX(calc(-100% - 24px))':''} ${info.y>$('canvas-wrap').clientHeight/2?'translateY(calc(-100% - 24px))':''}`,whiteSpace:'pre-wrap',padding:'12px',border:'1px solid #7d888c',borderRadius:'5px'}};return info.layer?.id.startsWith('open-endpoints-')&&info.object?`${busName(info.object.bus)} · ${info.object.branches.length} open switches · click to inspect`:null;},
    getCursor:({isDragging})=>isDragging?'grabbing':'crosshair',onAfterRender:afterRender,onError:e=>{stopRun('renderer error');error('Renderer error: '+e.message);},
    onLoad:()=>{ready=true;const params=new URLSearchParams(location.search),requested=params.get('dataset')??`synthetic-${params.get('size')??1000}`;load(entries.some(m=>m.id===requested)?requested:'synthetic-1000');}});
}
start().catch(e=>error(e.message));
