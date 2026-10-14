/* Reference renderer: authored geometry over the proposed diagram projections. */
(() => {
  'use strict';
  const { connectivity, singleLine, multiWire } = window.GALLERY_DATA;
  const state = { view: 'single_line', theme: 'paper', colour: 'phases', selected: { kind: 'equipment', equipment: { family: 'line', key: 'L1' } } };
  const host = document.getElementById('diagram');
  const eqKey = ref => ref?.equipment?.key;
  const esc = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const text = (x,y,value,cls='',anchor='start') => `<text x="${x}" y="${y}" class="${cls}" text-anchor="${anchor}">${esc(value)}</text>`;
  const circle = (x,y,r=3.5,cls='dot') => `<circle cx="${x}" cy="${y}" r="${r}" class="${cls}"/>`;
  const rect = (x,y,w,h,cls='surface',r=5) => `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${r}" class="${cls}"/>`;
  const path = (d,cls='glyph') => `<path d="${d}" class="${cls}"/>`;
  const equalEq = (a,b) => a && b && a.family===b.family && a.key===b.key;
  function selected(sources) {
    const s = state.selected;
    if (!s) return false;
    return sources.some(r => {
      if (s.kind==='bus') return (r.kind==='bus'||r.kind==='terminal') && r.bus===s.bus;
      if (!equalEq(s.equipment,r.equipment)) return false;
      return s.kind==='equipment' || r.kind==='equipment' || (r.kind==='part' && r.part_kind===s.part_kind && r.part===s.part);
    });
  }
  function wrap(item, body, attrs='') {
    const first = item.sources[0];
    let label = item.label || item.id;
    return `<g class="pick${selected(item.sources)?' selected':''}" data-id="${esc(item.id)}" data-source="${esc(JSON.stringify(first))}" tabindex="0" role="button" aria-label="Inspect ${esc(label)}" aria-pressed="${selected(item.sources)}" ${attrs}>${body}</g>`;
  }
  function roundRoute(points, radius=7) {
    if (points.length===2) return `M${points[0]} L${points[1]}`;
    let d=`M${points[0]}`;
    for(let i=1;i<points.length-1;i++) {
      const a=points[i-1],b=points[i],c=points[i+1];
      const ab=Math.hypot(b[0]-a[0],b[1]-a[1]),bc=Math.hypot(c[0]-b[0],c[1]-b[1]);
      if (!ab||!bc) { d+=` L${b}`;continue; }
      const r=Math.min(radius,ab/2,bc/2);
      const before=[b[0]-(b[0]-a[0])/ab*r,b[1]-(b[1]-a[1])/ab*r];
      const after=[b[0]+(c[0]-b[0])/bc*r,b[1]+(c[1]-b[1])/bc*r];
      d+=` L${before} Q${b} ${after}`;
    }
    return d+` L${points.at(-1)}`;
  }
  function singleLayout() {
    const ports={'U.out':[164,210],'H.bundle':[228,210],'T1.p':[314,210],'T1.s':[446,210],
      'S.bundle':[536,210],'F.bundle':[940,210],'L.bundle':[1268,210],
      'Q3.in':[940,344],'Q1.in':[1268,344],'S.neutral':[536,274],'GS.ref':[536,344]};
    return {ports,routes:{},top:50,height:410,boxes:{}};
  }
  function multiLayout() {
    const ports={},routes={},boxes={};
    const rows={A:266,B:312,C:358,N:418};
    for(const [bus,x,ts] of [['H',210,['A','B','C']],['S',660,['A','B','C','N']],['F',940,['A','B','C','N']],['L',1268,['A','N']]]) {
      for(const t of ts) ports[`${bus}.${t}`]=[x,rows[t]];
    }
    for(const t of ['A','B','C']) ports[`U.${t}`]=[164,rows[t]];
    const coilPorts={pAB:[[294,282],[294,342]],pBC:[[328,345],[380,325]],pCA:[[380,299],[328,279]],
      sAN:[[550,266],[490,266]],sBN:[[550,312],[490,312]],sCN:[[550,358],[490,358]]};
    for(const [coil,[a,b]] of Object.entries(coilPorts)) {
      ports[`T1.${coil}.a`]=a;ports[`T1.${coil}.b`]=b;
      boxes[`T1.${coil}`]={a,b};
    }
    const primary={
      'pAB.a':[[210,266],[294,266],[294,282]],
      'pAB.b':[[210,312],[248,312],[248,358],[294,358],[294,342]],
      'pBC.a':[[210,312],[248,312],[248,358],[294,358],[328,345]],
      'pBC.b':[[210,358],[234,358],[234,442],[414,442],[414,312],[380,325]],
      'pCA.a':[[210,358],[234,358],[234,442],[414,442],[414,312],[380,299]],
      'pCA.b':[[210,266],[294,266],[328,279]],
    };
    for(const [id,points] of Object.entries(primary)) routes[`T1.${id}.lead`]=points;
    for(const t of ['A','B','C']) {
      routes[`T1.s${t}N.a.lead`]=[[660,rows[t]],[550,rows[t]]];
      routes[`T1.s${t}N.b.lead`]=[[660,418],[468,418],[468,rows[t]],[490,rows[t]]];
    }
    for(const [i,t] of ['A','B','C'].entries()) {
      const x=1000+i*40,branch=964+i*24;
      ports[`Q3.${t}N.a`]=[x,588];ports[`Q3.${t}N.b`]=[x,658];
      routes[`Q3.${t}N.a.lead`]=[[940,rows[t]],[branch,rows[t]],[branch,510+i*20],[x,510+i*20],[x,588]];
      routes[`Q3.${t}N.b.lead`]=[[940,418],[908,418],[908,700],[x,700],[x,658]];
    }
    ports['Q1.AN.a']=[1268,302];ports['Q1.AN.b']=[1268,380];
    ports['GS.ref']=[660,490];
    return {ports,routes,boxes,top:140,height:610};
  }
  function coil(a,b) {
    const len=Math.hypot(b[0]-a[0],b[1]-a[1]),angle=Math.atan2(b[1]-a[1],b[0]-a[0])*180/Math.PI;
    const margin=(len-40)/2;
    let d=`M0,0 H${margin}`;
    for(let i=0;i<4;i++) d+=' a5,7 0 0 1 10,0';
    d+=` H${len}`;
    return `<g transform="translate(${a}) rotate(${angle})">${rect(-4,-15,len+8,30,'halo symbol-halo',4)}${path(d)}</g>`;
  }
  function ground(x,y) {
    return path(`M${x},${y} v12 m-17,0 h34 m-28,7 h22 m-16,7 h10`);
  }
  function singleSymbol(s) {
    if(['H','S','F','L'].includes(s.id)) {
      const x={H:228,S:536,F:940,L:1268}[s.id];
      let out=rect(x-18,178,36,65,'halo symbol-halo',4)+path(`M${x},188 V232`,'glyph busbar')+text(x,158,s.id,'id','middle');
      if(s.id==='S') out+=circle(536,274,4,'port-dot')+text(550,280,'N','small');
      return out;
    }
    if(s.id==='U') return rect(78,167,88,87,'halo symbol-halo',6)+`<circle cx="122" cy="210" r="31" class="glyph"/>`+path('M104,210 C111,187 116,187 122,210 S133,233 140,210')+path('M153,210 H164')+text(122,131,'U','id','middle')+text(122,152,'Source','secondary','middle');
    if(s.id==='T1') return rect(308,166,143,88,'halo symbol-halo',6)+path('M314,210 H332 M428,210 H446')+`<circle cx="364" cy="210" r="32" class="glyph"/><circle cx="396" cy="210" r="32" class="glyph"/>`+text(380,114,'T1','id','middle')+text(380,141,'Δ / Yg','heading','middle')+text(380,291,'Magnetic coupling','secondary','middle');
    if(s.id==='GS') return rect(510,336,52,42,'halo symbol-halo',3)+ground(536,344)+text(565,364,'GS · neutral bond','secondary');
    const x=s.id==='Q3'?940:1268;
    return rect(x-28,336,56,48,'halo symbol-halo',4)+path(`M${x},344 H${x-18} L${x},375 L${x+18},344 Z`)+text(x,408,s.id,'id','middle')+text(x,434,s.id==='Q3'?'Wye load · ABCN':'Single-phase load · A–N','secondary','middle');
  }
  function multiSymbol(s,layout) {
    const ts={H:['A','B','C'],S:['A','B','C','N'],F:['A','B','C','N'],L:['A','N']};
    if(ts[s.id]) {
      const coords=ts[s.id].map(t=>layout.ports[`${s.id}.${t}`]);const x=coords[0][0];
      let out=rect(x-19,coords[0][1]-17,s.id==='F'?108:38,coords.at(-1)[1]-coords[0][1]+34,'halo symbol-halo',5)+text(x,220,s.id,'id','middle');
      for(const [i,t] of ts[s.id].entries()) {
        const [px,py]=coords[i];
        if(s.id==='F') out+=path(`M${px-8},${py} H${px+12}`,`glyph busbar phase-${t}`);
        out+=circle(px,py,4.5,`port-dot phase-${t}`)+text(px+(s.id==='F'?0:0),py-12,t,'small','middle');
      }
      return out;
    }
    if(s.id==='U') {
      let out=rect(52,236,112,148,'surface')+rect(48,232,120,156,'halo symbol-halo',7)+text(108,200,'U','id','middle')+text(108,222,'Source','secondary','middle')+`<circle cx="106" cy="312" r="26" class="glyph"/>`+path('M91,312 C97,294 101,294 106,312 S116,330 121,312');
      for(const t of ['A','B','C']) {const [x,y]=layout.ports[`U.${t}`];out+=circle(x,y,4,`port-dot phase-${t}`);}
      return out;
    }
    if(s.glyph==='coil') return coil(layout.boxes[s.id].a,layout.boxes[s.id].b);
    if(s.id==='GS') return rect(634,486,52,44,'halo symbol-halo',4)+ground(660,490)+text(691,512,'GS · neutral bond','secondary');
    const a=layout.ports[`${s.id}.a`],b=layout.ports[`${s.id}.b`],x=a[0],mid=(a[1]+b[1])/2;
    let out=rect(x-15,a[1]-4,30,b[1]-a[1]+8,'halo symbol-halo',4)+path(`M${x},${a[1]} V${mid-19} M${x},${mid+19} V${b[1]}`)+rect(x-9,mid-19,18,38,'surface',1)+path(`M${x-9},${mid-19} H${x+9} V${mid+19} H${x-9} Z`);
    if(s.id.startsWith('Q3'))out+=text(x,577,s.id.split('.')[1],'small','middle');
    else out+=text(x+30,mid,'Q1','id')+text(x+30,mid+23,'A–N load','secondary');
    return out;
  }
  function phaseOf(connection,projection) {
    if(connection.meaning.kind==='conductor' && connection.meaning.paths.length===1)return connection.meaning.paths[0];
    if(state.view==='multi_wire' && connection.meaning.kind==='attachment') {
      const p=projection.ports.find(p=>p.id===connection.from);
      return p?.points[0]?.split('.').at(-1);
    }
    return null;
  }
  function render() {
    const p=state.view==='single_line'?singleLine:multiWire;
    const layout=state.view==='single_line'?singleLayout():multiLayout();
    document.documentElement.dataset.theme=state.theme;document.documentElement.dataset.colour=state.colour;
    for(const type of ['view','theme','colour'])for(const b of document.querySelectorAll(`button[data-${type}]`))b.setAttribute('aria-pressed',String(b.dataset[type]===state[type]));
    document.getElementById('drawing-title').textContent=`Four-wire feeder / ${state.view==='single_line'?'single-line':'multi-wire'}`;
    document.getElementById('drawing-note').textContent=state.view==='single_line'?'Bundle labels preserve conductor membership.':'Dots join. Crossings without dots do not connect.';
    let body='';
    if(state.view==='multi_wire') {
      body+=rect(258,186,370,288,'group-surface',7)+text(277,211,'T1','id')+text(606,211,'Δ / Yg','secondary','end')+text(336,244,'Primary Δ','secondary','middle')+text(550,244,'Secondary Y','secondary','middle');
      body+=rect(956,548,164,174,'group-surface',6)+text(1143,621,'Q3','id')+text(1143,646,'Wye load','secondary');
      body+=path('M448,249 V388','coupling')+text(443,468,'Shared transformer · polarity unspecified','small','middle');
      body+=text(798,207,'L1','id','middle')+text(798,232,'A  B  C  N','small','middle')+text(1135,207,'L2','id','middle')+text(1135,232,'A  N','small','middle');
    } else {
      body+=text(738,154,'L1','id','middle')+text(738,178,'A  B  C  N','small','middle')+text(1103,154,'L2','id','middle')+text(1103,178,'A  N','small','middle');
    }
    // Electrical wires come only from the reference projection, never from glyph proximity.
    for(const c of p.connections) {
      const points=layout.routes[c.id]||[layout.ports[c.from],layout.ports[c.to]];
      if(points.some(point=>!point))throw new Error(`Missing layout for ${c.id}`);
      const d=roundRoute(points,c.id.startsWith('T1.p')?0:7),phase=phaseOf(c,p);
      const mask=state.view==='multi_wire'&&c.id.startsWith('Q3.')?path(d,'crossing-mask'):'';
      const source=c.meaning.kind==='conductor'&&c.meaning.paths.length>1?
        [{kind:'equipment',equipment:c.meaning.equipment}]:c.sources;
      body+=wrap({...c,sources:source},mask+path(d,'halo')+path(d,`wire${phase?' phase-'+phase:''}`)+path(d,'hit'),`data-connection="${esc(c.id)}"`);
    }
    for(const s of p.symbols)body+=wrap(s,state.view==='single_line'?singleSymbol(s):multiSymbol(s,layout),`data-symbol="${esc(s.id)}"`);
    if(state.view==='multi_wire') {
      // These are same-point branch locations, not geometric intersections.
      body+=circle(294,266)+circle(294,358)+circle(414,312)+circle(468,312,3.2,'dot phase-N')+circle(468,358,3.2,'dot phase-N')+circle(660,418,4,'dot phase-N');
      for(const [i,t] of ['A','B','C'].entries())body+=circle(964+i*24,{A:266,B:312,C:358}[t],3.5,`dot phase-${t}`);
      for(const x of [1000,1040])body+=circle(x,700,3.3,'dot phase-N');
    }
    host.innerHTML=`<svg xmlns="http://www.w3.org/2000/svg" class="diagram-svg" viewBox="0 ${layout.top} 1440 ${layout.height}" role="group" aria-labelledby="svg-title svg-desc"><title id="svg-title">Four-wire feeder, ${state.view==='single_line'?'single-line':'multi-wire'} projection</title><desc id="svg-desc">A delta–wye transformer supplies a four-wire feeder and an A–N lateral. Only the source neutral is grounded. The primary and secondary windings have no wire connection. Select equipment or conductors to inspect the source mapping.</desc>${body}</svg>`;
    for(const el of host.querySelectorAll('.pick')) {
      const choose=()=>{state.selected=JSON.parse(el.dataset.source);updateSelection();};
      el.addEventListener('click',choose);
      el.addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();choose();}});
    }
    updateSelection();
  }
  function updateSelection() {
    const p=state.view==='single_line'?singleLine:multiWire;
    for(const el of host.querySelectorAll('.pick')) {
      const item=[...p.symbols,...p.connections].find(x=>x.id===el.dataset.id);
      const sources=item.meaning?.kind==='conductor'&&item.meaning.paths.length>1?
        [{kind:'equipment',equipment:item.meaning.equipment}]:item.sources;
      const on=selected(sources);el.classList.toggle('selected',on);el.setAttribute('aria-pressed',String(on));
    }
    inspect();
  }
  function table(rows,heads=['Part','From','To']) {
    return `<table><thead><tr>${heads.map(h=>`<th scope="col">${esc(h)}</th>`).join('')}</tr></thead><tbody>${rows.map(row=>`<tr>${row.map(c=>`<td>${esc(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  }
  function inspect() {
    const el=document.getElementById('inspector-content'),s=state.selected;
    document.getElementById('clear-selection').disabled=!s;
    if(!s){el.innerHTML='<div class="inspector-grid"><div><h2>Follow a connection.</h2><p class="description">Select a symbol or wire in either view to inspect its source identity.</p></div><p class="meta">Selection is shared between views. A bundle keeps the identities of all of its conductors.</p></div>';return;}
    let title='',description='',rows=[],meta='',heads;
    if(s.kind==='bus') {
      const bus=connectivity.buses.find(x=>x.id===s.bus);title=`Bus ${bus.id}`;description='A group of distinct electrical terminals. Grouping does not connect phases together.';
      rows=bus.points.map(id=>{const p=connectivity.points.find(x=>x.id===id);return [p.label,p.id,p.role.kind];});heads=['Terminal','Identity','Role'];meta='Source bus identity is preserved in both diagram representations.';
    } else {
      const e=connectivity.equipment.find(x=>equalEq(x.id,s.equipment));title=e.id.key+(s.kind==='part'?` / ${s.part}`:'');
      const at=pin=>e.pins.find(p=>p.id===pin)?.binding.point||'Unknown';
      if(e.model.kind==='conductor_paths') {
        description=e.id.key==='L1'?'Four explicit conductors connect the source bus S to feeder bus F.':'Only A and N continue to the single-phase lateral.';
        const paths=s.kind==='part'?e.model.paths.filter(p=>p.id===s.part):e.model.paths;
        rows=paths.map(p=>[p.id,at(p.from_pin),at(p.to_pin)]);
        meta='<strong>Explicit conductors</strong><br>No earth-return wire is inferred.<br>Operating values are not part of this fixture.';
      } else if(e.model.kind==='transformer') {
        if(s.kind==='part'&&s.part_kind==='grounding') {
          description='The secondary neutral has one declared ideal connection to the model earth reference.';rows=[['GS','S.N','earth']];meta='<strong>Grounding reference</strong><br>This does not add a protective-earth conductor to the feeder.';
        } else {
          description='Delta primary, wye secondary. Each winding retains its own terminal connections.';
          const coils=e.model.windings.flatMap(w=>w.coils).filter(c=>s.kind!=='part'||s.part_kind!=='coil'||c.id===s.part);
          rows=coils.map(c=>[c.id,at(c.a.pin),at(c.b.pin)]);
          meta='<strong>No interwinding wire</strong><br>Magnetic leg associations and coil polarity are unspecified. No polarity dots are drawn.';
        }
      } else if(e.model.kind==='terminal_device') {
        description=e.id.key==='Q3'?'Three load elements share the declared F.N terminal.':'One load element connects L.A to L.N.';
        rows=e.model.elements.filter(c=>s.kind!=='part'||c.id===s.part).map(c=>[c.id,at(c.a.pin),at(c.b.pin)]);
        meta='<strong>Terminal connections</strong><br>No power or impedance value is implied by these generic load symbols.';
      } else {
        description='Upstream equivalent with three known external terminals.';rows=e.pins.map(p=>[p.id,p.binding.point,'—']);meta='<strong>Internal detail unavailable</strong><br>The fixture does not declare connections inside this source.';
      }
    }
    el.innerHTML=`<div class="inspector-grid"><div><h2>${esc(title)}</h2><p class="description">${esc(description)}</p></div>${table(rows,heads)}<div class="meta">${meta}</div></div>`;
  }
  for(const type of ['view','theme','colour'])for(const button of document.querySelectorAll(`button[data-${type}]`)) {
    button.addEventListener('click',()=>{state[type]=button.dataset[type];render();});
  }
  document.getElementById('clear-selection').addEventListener('click',()=>{state.selected=null;updateSelection();});
  document.getElementById('export-svg').addEventListener('click',()=>{
    const svg=host.querySelector('svg'),copy=svg.cloneNode(true),original=[svg,...svg.querySelectorAll('*')],clones=[copy,...copy.querySelectorAll('*')];
    const properties=['fill','fill-opacity','stroke','stroke-width','stroke-opacity','stroke-dasharray','stroke-linecap','stroke-linejoin','font-family','font-size','font-weight','text-anchor','opacity','color'];
    for(let i=0;i<original.length;i++){
      const cs=getComputedStyle(original[i]);clones[i].setAttribute('style',properties.map(k=>`${k}:${cs.getPropertyValue(k)}`).join(';'));
    }
    copy.removeAttribute('class');copy.setAttribute('width','1440');copy.setAttribute('height',state.view==='single_line'?'410':'610');
    const bg=document.createElementNS('http://www.w3.org/2000/svg','rect');bg.setAttribute('x','0');bg.setAttribute('y',state.view==='single_line'?'50':'140');bg.setAttribute('width','1440');bg.setAttribute('height',state.view==='single_line'?'410':'610');bg.setAttribute('fill',getComputedStyle(document.documentElement).getPropertyValue('--paper').trim());copy.insertBefore(bg,copy.firstChild);
    for(const el of copy.querySelectorAll('.hit'))el.remove();
    const blob=new Blob([new XMLSerializer().serializeToString(copy)],{type:'image/svg+xml'}),url=URL.createObjectURL(blob),a=document.createElement('a');
    a.href=url;a.download=`tellegen-feeder-${state.view}-${state.theme}.svg`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
  });
  render();
})();
