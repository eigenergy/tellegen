/** Report-ready exports of the current view: vector SVG and a title block shared with PNG. Pure; no DOM. */
export const TITLE_BLOCK_HEIGHT=76,MAX_SVG_SEGMENTS=200000,TITLE_CHAR_PX=6.7;
/** Fit a title-block line into its column; monospace makes character count a safe width proxy. */
export const fitText=(text,px)=>{const n=Math.max(4,Math.floor((px-24)/TITLE_CHAR_PX));return text.length>n?text.slice(0,n-1)+'…':text;};
const esc=v=>String(v).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const rgb=(c,i=0)=>`rgb(${c[i]} ${c[i+1]} ${c[i+2]})`;
const r1=v=>Math.round(v*10)/10;
/** Visible segments only, grouped into one path per colour and width. Throws rather than writing a partial drawing. */
export function viewportSvg({bounds,scale,width,height,background,ink,muted,lineSets=[],points=null,markers=[],glyphs=[],texts=[],title}){
 const [x0,y0,x1,y1]=bounds,X=x=>r1((x-x0)*scale),Y=y=>r1((y-y0)*scale),groups=new Map();let segments=0;
 for(const set of lineSets){
  const {sources:s,targets:t,colors,widths,count,width:fixed=1}=set;
  for(let i=0;i<count;i++){
   const ax=s[i*3],ay=s[i*3+1],bx=t[i*3],by=t[i*3+1];
   if(Math.max(ax,bx)<x0||Math.min(ax,bx)>x1||Math.max(ay,by)<y0||Math.min(ay,by)>y1)continue;
   if(++segments>MAX_SVG_SEGMENTS)throw new Error(`More than ${MAX_SVG_SEGMENTS.toLocaleString()} segments in view; zoom in to export a vector drawing`);
   if(colors[i*4+3]===0)continue;
   const key=`${rgb(colors,i*4)}|${widths?r1(widths[i]):fixed}|${r1(colors[i*4+3]/255)}`;
   if(!groups.has(key))groups.set(key,[]);groups.get(key).push(`M${X(ax)} ${Y(ay)}L${X(bx)} ${Y(by)}`);
  }
 }
 const paths=[...groups].map(([key,d])=>{const [stroke,w,a]=key.split('|');return `<path d="${d.join('')}" stroke="${stroke}" stroke-width="${w}"${a<1?` stroke-opacity="${a}"`:''}/>`;});
 const dots=[];
 if(points)for(let i=0;i<points.count;i++){const px=points.positions[i*3],py=points.positions[i*3+1];if(px<x0||px>x1||py<y0||py>y1)continue;dots.push(`<circle cx="${X(px)}" cy="${Y(py)}" r="${points.radius}" fill="${rgb(points.colors,i*4)}"/>`);}
 const inside=p=>p[0]>=x0&&p[0]<=x1&&p[1]>=y0&&p[1]<=y1;
 for(const m of markers)if(inside(m.position))dots.push(`<circle cx="${X(m.position[0])}" cy="${Y(m.position[1])}" r="${m.radius}" fill="${m.fill??'none'}"${m.stroke?` stroke="${m.stroke}" stroke-width="1.5"`:''}/>`);
 // Text-layer angles are counter-clockwise; SVG rotation is clockwise.
 const arrows=glyphs.filter(g=>inside(g.position)).map(g=>`<text x="${X(g.position[0])}" y="${Y(g.position[1])}" fill="${g.color}" font-size="${g.size??22}" text-anchor="middle" dominant-baseline="central" transform="rotate(${r1(-g.angle)} ${X(g.position[0])} ${Y(g.position[1])})">${esc(g.text)}</text>`);
 const labels=texts.filter(t=>t.position[0]>=x0&&t.position[0]<=x1&&t.position[1]>=y0&&t.position[1]<=y1).map(t=>`<text x="${X(t.position[0])+(t.dx??0)}" y="${Y(t.position[1])+(t.dy??0)}" fill="${t.color??ink}">${esc(t.text)}</text>`);
 const H=height+TITLE_BLOCK_HEIGHT,col=width/2,line=(x,y,text,cls='')=>`<text x="${x}" y="${y}"${cls?` class="${cls}"`:''}>${esc(fitText(text,col))}</text>`;
 const block=[`<rect x="0" y="${height}" width="${width}" height="${TITLE_BLOCK_HEIGHT}" fill="${background}"/>`,`<line x1="0" x2="${width}" y1="${height+.5}" y2="${height+.5}" stroke="${muted}"/>`,
  ...title.left.map((t,i)=>line(12,height+18+i*15,t,i?'muted':'strong')),...title.right.map((t,i)=>line(col+12,height+18+i*15,t,'muted'))];
 return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${H}" viewBox="0 0 ${width} ${H}" font-family="IBM Plex Mono, ui-monospace, monospace" font-size="11">`+
  `<style>path{fill:none;stroke-linecap:round}.muted{fill:${muted}}.strong{fill:${ink};font-size:12px;font-weight:600}</style>`+
  `<rect width="${width}" height="${height}" fill="${background}"/><g>${paths.join('')}</g><g>${dots.join('')}</g><g>${arrows.join('')}</g><g>${labels.join('')}</g>${block.join('')}</svg>`;
}
/** Title block text: what, which view, and the provenance needed to reproduce it. */
export function titleBlock({dataset,layout,view,result,camera,exported}){
 return {left:[dataset,layout,view,`Centre ${camera.target[0].toFixed(1)}, ${camera.target[1].toFixed(1)} · ${(2**camera.zoom).toPrecision(3)}×`],
  right:[result?.kind==='stress'?'ARTIFICIAL STRESS: NOT A SOLUTION':result?`${result.label} · ${result.iterations} iterations`:'No result snapshot',result?`Result SHA-256 ${result.sha256.slice(0,16)}…`:'','P/Q positive into equipment at each end',`Exported ${exported}`].filter(Boolean)};
}
