/** Hand-authored visual specimens, not inferred models of imported equipment. */
export const WIDTH=380,HEIGHT=200;
const p=(d,role='wire')=>({tag:'path',d,role});
const c=(cx,cy,r,role='symbol')=>({tag:'circle',cx,cy,r,role});
const rect=(x,y,width,height,role='block')=>({tag:'rect',x,y,width,height,role});
const t=(x,y,text,role='label',anchor='middle')=>({tag:'text',x,y,text,role,anchor});
const pin=(id,x,y,label,kind='terminal')=>({id,x,y,label,kind});
const ends=[pin('from',24,80,'from'),pin('to',356,80,'to')];
// Reference identifiers are local to one specimen, never implicit network grounds.
const ref=(x,y,id='0',label='0 — model reference')=>[
 {...p(`M${x-8} ${y}H${x+8}L${x} ${y+9}Z`,'model-reference'),'data-model-reference':id},
 t(x,y+29,label,'reference-label')
];
const z=(x,y,w,label)=>[rect(x,y-13,w,26),t(x+w/2,y+5,label,'value')];
const view=(caption,primitives)=>({caption,primitives});
const sourceEquipment=letter=>[c(110,80,27),letter==='∿'?p('M94 80C98 62 106 62 110 80S122 98 126 80','symbol'):t(110,87,letter,'large'),p('M137 80H356')];
const sourceModel=(e,impedance)=>[c(76,94,23),t(76,100,e,'value'),t(60,64,'+','small'),t(60,132,'−','small'),p('M76 71V44H148V80H164 M240 80H356 M76 117V154'),...z(164,80,76,impedance),...ref(76,154)];
const contacts=()=>[c(152,80,4,'contact'),c(228,80,4,'contact')];
const switchLeads=()=>[p('M24 80H148 M232 80H356'),...contacts()];
export const specimens=[
 {id:'L1',kind:'pi_line',title:'π-model line',tag:'SERIES + SHUNTS',ports:ends,
  assumption:'Series impedance between line ends, with a shunt admittance from each end to the model voltage reference. End values follow PowerIO, including equal halves where specified. This specimen shows the scalar case.',
  equipment:view('One equipment connection; the π badge identifies its model.',[p('M24 80H356'),rect(170,28,40,29,'badge'),t(190,49,'π','large')]),
  equivalent:view('Zs joins the line ends; Yf and Yt each connect to reference 0.',[p('M24 80H140 M240 80H356 M84 80V111 M296 80V111 M84 137V154 M296 137V154'),...z(140,80,100,'Zs'),...z(55,124,58,'Yf'),...z(267,124,58,'Yt'),c(84,80,3,'junction'),c(296,80,3,'junction'),...ref(84,154),...ref(296,154)])},
 {id:'G1',kind:'generator',title:'Generator',tag:'MACHINE',ports:[pin('terminal',356,80,'terminal')],
  assumption:'Illustrative generator represented by internal voltage E behind Zg. This is a chosen study model, not a claim that every generator or inverter uses it. The source voltage is measured to reference 0. No operating values or power-flow direction are assigned.',
  equipment:view('Machine identity is carried by the G symbol.',sourceEquipment('G')),
  equivalent:view('Chosen internal-voltage model: E behind Zg.',sourceModel('E','Zg'))},
 {id:'S1',kind:'supply',title:'External supply',tag:'GRID BOUNDARY',ports:[pin('terminal',356,80,'terminal')],
  assumption:'The supply is a Thévenin boundary with Vth and Zth. The sinusoid identifies an AC source; it does not assert generator technology or a slack-bus assignment. Vth is measured to reference 0; no physical earthing arrangement is specified.',
  equipment:view('An AC supply boundary, distinct from a machine.',sourceEquipment('∿')),
  equivalent:view('Chosen Thévenin model: Vth behind Zth.',sourceModel('Vth','Zth'))},
 {id:'D1',kind:'load',title:'Load',tag:'CONSTANT POWER',ports:[pin('terminal',190,24,'terminal')],
  assumption:'This specimen explicitly chooses a constant-power load, S = P + jQ. Voltage is measured from the terminal to reference 0. The model block does not imply resistance. A ZIP load, motor, or phase-to-phase device needs a different declared model and terminal mapping.',
  equipment:view('A demand arrow identifies the load attachment.',[p('M190 24V81'),p('M172 81H208L190 111Z','symbol'),t(190,150,'load','reference-label')]),
  equivalent:view('A constitutive model block, not a resistor.',[p('M190 24V64 M190 120V154'),rect(122,64,136,56),t(190,97,'S = P + jQ','value'),...ref(190,154)])},
 {id:'T1',kind:'transformer',title:'Transformer',tag:'TWO WINDINGS',ports:[pin('primary',24,80,'primary'),pin('secondary',356,80,'secondary')],
  assumption:'Illustrative scalar model: primary-referred leakage Zℓ and an ideal a:1 transformer. The magnetizing branch is omitted by this declared model. The voltage references 0p and 0s are local to their respective windings and are not electrically joined; this is not a winding or multi-wire connection diagram.',
  equipment:view('Two winding symbols with distinct equipment ends.',[p('M24 80H140 M240 80H356'),c(168,80,28),c(212,80,28),t(190,142,'two-winding transformer','reference-label')]),
  equivalent:view('Leakage plus ideal ratio; no interwinding wire.',[p('M24 80H60 M138 80H151V50H178 M230 50H254V80H356 M178 114V154H143 M230 114V154H267'),...z(60,80,78,'Zℓ'),p('M178 50c-18 0 -18 16 0 16c-18 0 -18 16 0 16c-18 0 -18 16 0 16c-18 0 -18 16 0 16','symbol'),p('M230 50c18 0 18 16 0 16c18 0 18 16 0 16c18 0 18 16 0 16c18 0 18 16 0 16','symbol'),p('M197 50V114 M211 50V114','magnetic'),t(204,30,'ideal a:1','small'),...ref(143,154,'0p','0p · reference'),...ref(267,154,'0s','0s · reference')])},
 {id:'C1',kind:'shunt',title:'Capacitive shunt',tag:'TWO TERMINALS',ports:[pin('bus',190,24,'bus'),pin('reference',190,158,'return (external)','reference')],
  assumption:'A capacitive shunt is explicitly declared: Yc = jωC. The hollow return port is an external terminal with its connection unspecified, not a fixed voltage-reference marker. An inductive or general shunt must not inherit this capacitor symbol.',
  equipment:view('Capacitor plates identify the declared shunt type.',[p('M190 24V72 M164 72H216 M164 86H216 M190 86V158'),t(238,86,'C','value')]),
  equivalent:view('Admittance between bus and external return; no reference is assigned.',[p('M190 24V65 M190 117V158'),rect(132,65,116,52),t(190,96,'Yc = jωC','value')])},
 ...['closed','open','unknown'].map((state,i)=>({id:`Q${i+1}`,kind:'switch',state,title:`Switch · ${state}`,tag:state.toUpperCase(),ports:ends,
  assumption:state==='unknown'?'Position is unavailable. The question mark is not a partly open blade; no conductive relation or ideal-switch equation is inferred. Service availability is a separate attribute.':`A generic switch with a declared ${state} pole. This is not a circuit-breaker or disconnector classification. The ideal model describes position, not service availability, energization or power flow.`,
  equipment:view(state==='unknown'?'Unknown position has its own geometry.':`The blade makes the ${state} position explicit.`,[...switchLeads(),...(state==='unknown'?[rect(170,58,40,44,'unknown'),t(190,90,'?','large')]:[p(state==='open'?'M152 80L218 46':'M152 80H228','symbol')]),t(190,140,state.toUpperCase(),'reference-label')]),
  equivalent:view(state==='unknown'?'Unresolved state: no inferred continuity.':`Ideal ${state} pole with its governing constraint.`,[...switchLeads(),...(state==='closed'?[p('M152 80H228'),t(190,135,'ΔV = 0','value')]:state==='open'?[t(190,135,'I = 0','value')]:[rect(170,58,40,44,'unknown'),t(190,90,'?','large'),t(190,143,'state unresolved','reference-label')])])}))
];
export const escape=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
export function glyph(specimen,mode){
 const primitives=specimen[mode].primitives.map(({tag,role,text,anchor,...attrs})=>`<${tag} class="${role}" ${Object.entries(attrs).map(([k,v])=>`${k}="${escape(v)}"`).join(' ')}${tag==='text'?` text-anchor="${anchor}"`:''}>${text===undefined?'':escape(text)}</${tag}>`).join('');
 const ports=specimen.ports.map(port=>`<g data-port="${escape(port.id)}"><circle class="${port.kind==='reference'?'reference':'port'}" cx="${port.x}" cy="${port.y}" r="3"/><text class="port-label" x="${port.x}" y="${port.y===24?port.y-9:port.y+27}" text-anchor="${port.x<60?'start':port.x>320?'end':'middle'}">${escape(port.label)}</text></g>`).join('');
 return primitives+ports;
}
export function svg(specimen,mode){return `<svg class="specimen-svg" viewBox="0 0 ${WIDTH} ${HEIGHT}" role="img" aria-labelledby="${specimen.id}-${mode}-title ${specimen.id}-${mode}-desc" xmlns="http://www.w3.org/2000/svg"><title id="${specimen.id}-${mode}-title">${escape(specimen.title)} · ${mode==='equipment'?'equipment':'equivalent circuit'}</title><desc id="${specimen.id}-${mode}-desc">${escape(specimen[mode].caption+' '+specimen.assumption)}</desc><g data-source="${specimen.id}">${glyph(specimen,mode)}</g></svg>`;}
export function svgStyle(palette){return `text{font-family:ui-monospace,monospace;fill:${palette.ink};font-size:14px}.wire,.symbol,.model-reference{fill:none;stroke:${palette.wire};stroke-width:1.8;stroke-linecap:round;stroke-linejoin:round}.block,.badge{fill:${palette.paper};stroke:${palette.wire};stroke-width:1.8}.badge{stroke:${palette.accent}}.contact,.port,.reference{fill:${palette.paper};stroke:${palette.wire};stroke-width:1.8}.junction{fill:${palette.wire}}.magnetic{fill:none;stroke:${palette.muted};stroke-width:1.4;stroke-dasharray:3 4}.unknown{fill:${palette.paper};stroke:${palette.wire};stroke-width:1.8;stroke-dasharray:3 3}.large{font-size:25px}.small{font-size:12px}.port-label,.reference-label{fill:${palette.muted};font-size:12px}.value{font-size:15px}`;}
/** Portable review export, with theme colors resolved and stable source/port IDs. */
export function sheetSVG(mode,palette){
 const modes=mode==='compare'?['equipment','equivalent']:[mode],width=modes.length===2?900:470,rowHeight=276,height=146+specimens.length*rowHeight;
 const rows=specimens.map((s,i)=>{const y=140+i*rowHeight;return `<g data-specimen="${s.id}" transform="translate(28 ${y})"><text x="0" y="0" style="font-size:18px">${s.id} · ${escape(s.title)}</text>${modes.map((m,j)=>`<g transform="translate(${j*432} 22)"><text class="reference-label" x="0" y="14">${m==='equipment'?'EQUIPMENT':'EQUIVALENT CIRCUIT'}</text><g transform="translate(0 20)" data-view="${m}" data-source="${s.id}">${glyph(s,m)}</g></g>`).join('')}<desc>${escape(s.assumption)}</desc></g>`;}).join('');
 return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" role="img"><title>Tellegen electrical symbol specimens</title><desc>Illustrative scalar study models. Triangles label model voltage references local to each specimen; small circles mark external ports or switch contacts. No physical earthing arrangement is specified. Not a multi-wire connection diagram or a standards compliance claim. Each specimen description records its assumptions.</desc><style>${svgStyle(palette)}</style><rect width="100%" height="100%" fill="${palette.paper}"/><text x="28" y="35" style="font-size:22px">tellegen. / Symbol specimens</text><text x="28" y="60" class="reference-label">Illustrative scalar models · no solved values</text><text x="28" y="80" class="reference-label">▽ labelled voltage reference · ○ external port/contact</text><text x="28" y="100" class="reference-label">References are local to each specimen; no earth bond implied.</text>${rows}</svg>`;
}
