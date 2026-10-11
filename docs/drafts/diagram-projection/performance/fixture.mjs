/** Deterministic schematic stress data, not a PowerIO or solvable electrical case. */
export const SIZES = [1000, 10000, 100000];
export const VERSION = 1;
export const REGION_SIZE = 1000;
export const REGION_WIDTH = 920;
export const REGION_HEIGHT = 1120;
export function makeFixture(count) {
  if (!SIZES.includes(count)) throw new Error('Unsupported fixture size');
  const regions = count / REGION_SIZE, columns = Math.ceil(Math.sqrt(regions));
  const positions = new Float32Array(count * 3);
  const localY = new Float32Array(REGION_SIZE);
  let leaf = 0;
  // A complete binary feeder with deterministic, non-overlapping leaf order.
  function place(i) {
    const left = 2*i+1, right = left+1;
    if (left >= REGION_SIZE) return localY[i] = 40 + leaf++ * 2;
    const a = place(left), b = right < REGION_SIZE ? place(right) : a;
    return localY[i] = (a+b)/2;
  }
  place(0);
  for (let r=0;r<regions;r++) for(let i=0;i<REGION_SIZE;i++) {
    const n = r*REGION_SIZE+i;
    positions[n*3] = (r%columns)*REGION_WIDTH + 60 + Math.floor(Math.log2(i+1))*80;
    positions[n*3+1] = Math.floor(r/columns)*REGION_HEIGHT + localY[i];
  }
  const ends=[], status=[], sources=[], targets=[], owners=[];
  function segment(a,b,owner) {
    if (a[0]===b[0] && a[1]===b[1]) return;
    sources.push(a[0],a[1],0); targets.push(b[0],b[1],0); owners.push(owner);
  }
  function branch(a,b,open=false,backbone=false) {
    const id=status.length; ends.push(a,b); status.push(open?1:0);
    const p=[positions[a*3],positions[a*3+1]],q=[positions[b*3],positions[b*3+1]];
    // Open ties have a real visible gap, not colour-only state.
    if(open) { const mid=[(p[0]+q[0])/2,(p[1]+q[1])/2]; segment(p,[mid[0],mid[1]-0.7],id);segment([mid[0],mid[1]+0.7],q,id); }
    else if(backbone) {
      const y=Math.min(p[1],q[1])-localY[0]+15;
      const lane=p[0]-35;
      const points=[p,[lane,p[1]],[lane,y],[q[0],y],q];
      for(let i=1;i<points.length;i++)segment(points[i-1],points[i],id);
    } else { segment(p,[p[0],q[1]],id); segment([p[0],q[1]],q,id); }
  }
  for(let r=0;r<regions;r++) {
    const base=r*REGION_SIZE;
    for(let i=1;i<REGION_SIZE;i++)branch(base+Math.floor((i-1)/2),base+i);
    // Ten normally-open cross ties per region, between adjacent sibling leaves.
    for(let i=0;i<10;i++)branch(base+501+i*40,base+502+i*40,true);
    if(r)branch((r-1)*REGION_SIZE,base,false,true);
  }
  const arrays = {positions, endpoints:new Uint32Array(ends), states:new Uint8Array(status),
    segmentSources:new Float32Array(sources), segmentTargets:new Float32Array(targets), segmentBranches:new Uint32Array(owners)};
  const {buffer,sections}=packArrays(arrays);
  const manifest={version:VERSION,count,regions,columns,regionSize:REGION_SIZE,regionWidth:REGION_WIDTH,regionHeight:REGION_HEIGHT,
    branches:status.length,openTies:regions*10,segments:owners.length,bytes:buffer.byteLength,sections,
    bounds:[0,0,columns*REGION_WIDTH,Math.ceil(regions/columns)*REGION_HEIGHT],file:`feeder-${count}.bin`};
  return {manifest,buffer};
}
export function packArrays(arrays) {
  let bytes=0; const sections={};
  for(const [name,array] of Object.entries(arrays)) { bytes=Math.ceil(bytes/4)*4;sections[name]={offset:bytes,length:array.length,type:array.constructor.name};bytes+=array.byteLength; }
  const buffer=new ArrayBuffer(Math.ceil(bytes/4)*4);
  for(const [name,array] of Object.entries(arrays))new Uint8Array(buffer,sections[name].offset,array.byteLength).set(new Uint8Array(array.buffer));
  return {buffer,sections};
}
export function decodeFixture(manifest,buffer) {
  if(manifest.version!==VERSION || (!Number.isSafeInteger(manifest.count)||manifest.count<1) || buffer.byteLength!==manifest.bytes)throw new Error('Invalid fixture header or length');
  const types={Float32Array,Uint32Array,Uint8Array}, result={manifest,buffer};
  for(const [name,s] of Object.entries(manifest.sections)) {
    const Type=types[s.type]; if(!Type || s.offset<0 || s.length<0 || s.offset+s.length*Type.BYTES_PER_ELEMENT>buffer.byteLength)throw new Error('Invalid fixture section');
    result[name]=new Type(buffer,s.offset,s.length);
  }
  const expected={positions:manifest.count*3,endpoints:manifest.branches*2,states:manifest.branches,segmentSources:manifest.segments*3,segmentTargets:manifest.segments*3,segmentBranches:manifest.segments};
  for(const [key,length] of Object.entries(expected))if(result[key]?.length!==length)throw new Error(`Invalid fixture ${key} length`);
  if(!result.endpoints.every(i=>i<manifest.count)||!result.segmentBranches.every(i=>i<manifest.branches))throw new Error('Invalid fixture reference');
  for(const key of ['positions','segmentSources','segmentTargets'])if(!result[key].every(Number.isFinite))throw new Error('Non-finite geometry');
  return result;
}
