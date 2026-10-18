import test from 'node:test';import assert from 'node:assert/strict';
import {viewportSvg,titleBlock,TITLE_BLOCK_HEIGHT} from './export-view.mjs';
const set=(segs,colors,widths=null)=>({sources:Float32Array.from(segs.flatMap(s=>[s[0],s[1],0])),targets:Float32Array.from(segs.flatMap(s=>[s[2],s[3],0])),colors:Uint8Array.from(colors.flat()),widths,count:segs.length});
const title={left:['Feeder <A>','Tidy tree'],right:['Saved']};
const base={bounds:[0,0,100,50],scale:2,width:200,height:100,background:'#fff',ink:'#000',muted:'#666',title};
test('SVG keeps only segments that touch the view and groups paths by colour and width',()=>{
 const svg=viewportSvg({...base,lineSets:[set([[0,0,10,0],[20,0,30,0],[200,200,300,300],[90,40,150,40]],[[255,0,0,255],[255,0,0,255],[255,0,0,255],[0,0,255,255]])]});
 assert.equal((svg.match(/<path /g)||[]).length,2);assert.match(svg,/M0 0L20 0M40 0L60 0/);assert.ok(!svg.includes('M400'));assert.match(svg,/stroke="rgb\(0 0 255\)"/);});
test('SVG carries flow widths, skips transparent segments and escapes labels',()=>{
 const svg=viewportSvg({...base,lineSets:[set([[0,0,10,0],[0,10,10,10]],[[1,2,3,255],[1,2,3,0]],Float32Array.of(4.25,1))],texts:[{position:[5,5],text:'<b>&'},{position:[500,5],text:'off'}]});
 assert.match(svg,/stroke-width="4.3"/);assert.equal((svg.match(/<path /g)||[]).length,1);assert.match(svg,/&lt;b&gt;&amp;/);assert.ok(!svg.includes('>off<'));assert.match(svg,/Feeder &lt;A&gt;/);});
test('SVG adds a title block below the drawing and refuses oversized views',()=>{
 const svg=viewportSvg({...base});assert.match(svg,new RegExp(`height="${100+TITLE_BLOCK_HEIGHT}"`));
 const n=200001,many={sources:new Float32Array(n*3),targets:new Float32Array(n*3).fill(1),colors:new Uint8Array(n*4).fill(255),count:n};
 assert.throws(()=>viewportSvg({...base,lineSets:[many]}),/zoom in/);});
test('title block names the view and the result provenance, and flags stress data',()=>{
 const camera={target:[1,2,0],zoom:1};
 const solved=titleBlock({dataset:'French',layout:'Tidy tree',view:'Loading',result:{kind:'solved',label:'Saved PF',iterations:16,sha256:'abcdef0123456789abcdef'},camera,exported:'2026-10-18T00:00:00Z'});
 assert.deepEqual(solved.right,['Saved PF · 16 iterations','Result SHA-256 abcdef0123456789…','P/Q positive into equipment at each end','Exported 2026-10-18T00:00:00Z']);assert.match(solved.left[3],/2\.00×/);
 assert.match(titleBlock({dataset:'S',layout:'L',view:'V',result:{kind:'stress',sha256:'x'.repeat(64)},camera,exported:'t'}).right[0],/NOT A SOLUTION/);});
test('arrows rotate clockwise in SVG from counter-clockwise text angles; long title lines are truncated',()=>{
 const svg=viewportSvg({...base,glyphs:[{position:[10,10],text:'▸',angle:90,color:'teal'}],markers:[{position:[20,20],radius:6,fill:'orange'}],title:{left:['x'.repeat(300)],right:[]}});
 assert.match(svg,/rotate\(-90 20 20\)/);assert.match(svg,/<circle cx="40" cy="40" r="6" fill="orange"\/>/);assert.match(svg,/x{10,}…<\/text>/);});
