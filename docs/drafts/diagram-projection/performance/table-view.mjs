/** Virtualised, sortable results table. Renders only visible rows, so 100k-row snapshots stay responsive. */
import {query,toCsv} from './results-table.mjs';
const ROW=24,OVERSCAN=8;
const esc=v=>String(v).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
export function createTableView(root,{onSelect,csvName=()=>'results.csv',csvMeta=()=>[]}){
 root.innerHTML=`<div class="table-toolbar"><label class="visually-hidden" for="table-filter">Filter rows</label><input id="table-filter" type="search" placeholder="Filter by ID, source or endpoint" autocomplete="off"><label class="toggle"><input type="checkbox" id="table-issues"> Issues only</label><span class="fine table-count" aria-live="polite"></span><button class="action table-csv">Export CSV</button></div><div class="table-scroll" tabindex="0" role="grid" aria-label="Results table"><div class="table-head" role="row"></div><div class="table-spacer"><div class="table-rows" role="rowgroup"></div></div></div>`;
 const scroll=root.querySelector('.table-scroll'),head=root.querySelector('.table-head'),spacer=root.querySelector('.table-spacer'),rows=root.querySelector('.table-rows'),count=root.querySelector('.table-count');
 let model=null,indices=new Int32Array(),sortKey=null,descending=false,selected=-1,frame=0,timer=0;
 const template=()=>model.columns.map(c=>c.numeric?'minmax(78px,.8fr)':c.key==='id'||c.key==='from'||c.key==='to'?'minmax(140px,1.6fr)':'minmax(90px,1fr)').join(' ');
 function header(){
  head.style.gridTemplateColumns=template();
  head.innerHTML=model.columns.map(c=>`<button role="columnheader" data-key="${c.key}" class="${c.numeric?'num':''}" aria-sort="${sortKey===c.key?(descending?'descending':'ascending'):'none'}" title="${esc(c.title??'Sort by '+c.label)}">${esc(c.label)}${c.unit?`<small>${esc(c.unit)}</small>`:''}${sortKey===c.key?(descending?' ▾':' ▴'):''}</button>`).join('');
 }
 function draw(){
  frame=0;if(!model)return;
  const top=Math.max(0,Math.floor((scroll.scrollTop-head.offsetHeight)/ROW)-OVERSCAN),end=Math.min(indices.length,top+Math.ceil(scroll.clientHeight/ROW)+2*OVERSCAN),cols=template(),html=[];
  for(let k=top;k<end;k++){const i=indices[k],row=model.row(i);
   html.push(`<div role="row" class="table-row ${row.state??''}" data-i="${i}" aria-selected="${i===selected}" style="top:${k*ROW}px;grid-template-columns:${cols}">${model.columns.map(c=>`<span role="gridcell" class="${c.numeric?'num':''}">${esc(c.format(row[c.key]))}</span>`).join('')}</div>`);}
  rows.innerHTML=html.join('');
 }
 const schedule=()=>{if(!frame)frame=requestAnimationFrame(draw);};
 function refresh(){
  if(!model)return;
  indices=query(model,{text:root.querySelector('#table-filter').value,issuesOnly:root.querySelector('#table-issues').checked,sortKey,descending});
  spacer.style.height=`${indices.length*ROW}px`;count.textContent=`${indices.length.toLocaleString()} of ${model.count.toLocaleString()} rows`;header();draw();
 }
 function reveal(i){const k=indices.indexOf(i);if(k<0)return;const y=k*ROW+head.offsetHeight;if(y<scroll.scrollTop+head.offsetHeight||y+ROW>scroll.scrollTop+scroll.clientHeight)scroll.scrollTop=y-scroll.clientHeight/2;schedule();}
 scroll.addEventListener('scroll',schedule,{passive:true});
 head.addEventListener('click',e=>{const b=e.target.closest('[data-key]');if(!b)return;const key=b.dataset.key,col=model.columns.find(c=>c.key===key);
  // Numbers start largest-first, text A–Z; a second click reverses.
  descending=sortKey===key?!descending:!!col.numeric;sortKey=key;refresh();});
 root.querySelector('#table-filter').addEventListener('input',()=>{clearTimeout(timer);timer=setTimeout(refresh,model?.count>20000?120:0);});
 root.querySelector('#table-issues').addEventListener('change',refresh);
 rows.addEventListener('click',e=>{const r=e.target.closest('[data-i]');if(r)onSelect(model.kind,Number(r.dataset.i));});
 scroll.addEventListener('keydown',e=>{
  if(!model||!['ArrowDown','ArrowUp','Home','End'].includes(e.key))return;e.preventDefault();
  const k=indices.indexOf(selected),n=e.key==='Home'?0:e.key==='End'?indices.length-1:Math.max(0,Math.min(indices.length-1,(k<0?-1:k)+(e.key==='ArrowDown'?1:-1)));
  if(indices.length)onSelect(model.kind,indices[n]);
 });
 root.querySelector('.table-csv').addEventListener('click',()=>{if(!model)return;
  const url=URL.createObjectURL(new Blob([toCsv(model,indices,csvMeta())],{type:'text/csv;charset=utf-8'})),a=document.createElement('a');a.href=url;a.download=csvName(model.kind);a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);});
 return {
  setModel(next){model=next;selected=-1;sortKey=null;descending=false;scroll.scrollTop=0;if(!model){rows.innerHTML='';head.innerHTML='';spacer.style.height='0';count.textContent='';return;}refresh();},
  setSelection(kind,i){selected=model&&kind===model.kind?i:-1;if(selected>=0)reveal(selected);else schedule();},
  get kind(){return model?.kind??null;}
 };
}
