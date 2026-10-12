import {specimens,svg,svgStyle,sheetSVG,escape} from './symbols.mjs';
const params=new URLSearchParams(location.search),root=document.documentElement;
let mode=['equipment','equivalent','compare'].includes(params.get('view'))?params.get('view'):'compare';
root.dataset.theme=params.get('theme')==='dark'?'dark':'paper';root.dataset.colour=params.get('colour')==='mono'?'mono':'accent';
const style=document.createElement('style');style.id='symbol-palette';document.head.append(style);
function palette(){const css=getComputedStyle(root);return Object.fromEntries(['paper','ink','wire','muted','accent'].map(k=>[k,css.getPropertyValue('--'+k).trim()]));}
function repaint(){style.textContent=svgStyle(palette()).replace(/(^|})\s*([^{}]+)\{/g,(_,end,selectors)=>end+selectors.split(',').map(s=>'.specimen-svg '+s.trim()).join(',')+'{');}
function render(){
 const modes=mode==='compare'?['equipment','equivalent']:[mode],container=document.getElementById('specimens');container.dataset.view=mode;
 container.innerHTML=specimens.map(s=>`<article class="specimen-card" id="specimen-${s.id}" data-source="${s.id}" aria-labelledby="title-${s.id}"><header class="card-heading"><h2 id="title-${s.id}"><span class="specimen-id">${s.id}</span> ${escape(s.title)}</h2><span class="card-tag">${s.tag}</span></header><div class="specimen-views">${modes.map(m=>`<figure class="symbol-view"><div class="view-heading">${m==='equipment'?'EQUIPMENT':'EQUIVALENT CIRCUIT'}</div><div class="symbol-drawing">${svg(s,m)}</div><figcaption>${escape(s[m].caption)}</figcaption></figure>`).join('')}</div><p class="assumptions"><strong>Model boundary.</strong> ${escape(s.assumption)}</p></article>`).join('');
 update();
}
function update(){
 for(const key of ['view','theme','colour'])for(const b of document.querySelectorAll(`button[data-${key}]`))b.setAttribute('aria-pressed',String(b.dataset[key]===(key==='view'?mode:root.dataset[key])));
 const url=new URL(location.href);url.searchParams.set('view',mode);url.searchParams.set('theme',root.dataset.theme);url.searchParams.set('colour',root.dataset.colour);history.replaceState(null,'',url);
 document.getElementById('display-status').textContent=`${mode==='compare'?'Equipment and equivalent circuits':mode==='equipment'?'Equipment symbols':'Equivalent circuits'} · ${root.dataset.theme} · ${root.dataset.colour==='mono'?'monochrome':'Tellegen accents'}`;repaint();
}
for(const b of document.querySelectorAll('button[data-view]'))b.addEventListener('click',()=>{mode=b.dataset.view;render();});
for(const key of ['theme','colour'])for(const b of document.querySelectorAll(`button[data-${key}]`))b.addEventListener('click',()=>{root.dataset[key]=b.dataset[key];update();});
window.addEventListener('beforeprint',()=>{style.textContent=svgStyle({paper:'#fff',ink:'#111',wire:'#111',muted:'#444',accent:'#111'}).replace(/(^|})\s*([^{}]+)\{/g,(_,end,selectors)=>end+selectors.split(',').map(s=>'.specimen-svg '+s.trim()).join(',')+'{');});
window.addEventListener('afterprint',repaint);
document.getElementById('export-svg').addEventListener('click',()=>{
 const text=sheetSVG(mode,palette()),url=URL.createObjectURL(new Blob([text],{type:'image/svg+xml'})),a=document.createElement('a');a.href=url;a.download=`tellegen-symbols-${mode}-${root.dataset.theme}-${root.dataset.colour}.svg`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
});
render();
