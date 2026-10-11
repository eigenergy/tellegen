/** Geometry diagnostics, outside the render loop and layout timing. */
const lower=(a,x)=>{let lo=0,hi=a.length;while(lo<hi){const m=(lo+hi)>>1;if(a[m]<x)lo=m+1;else hi=m;}return lo;};
const upper=(a,x)=>{let lo=0,hi=a.length;while(lo<hi){const m=(lo+hi)>>1;if(a[m]<=x)lo=m+1;else hi=m;}return lo;};
// Strict interior intersections: touching endpoints and collinear sharing excluded.
function crossings(horizontal,vertical){
  const xs=[...new Set(vertical.map(v=>v.fixed))].sort((a,b)=>a-b),tree=new Float64Array(xs.length+1);
  const add=(x,value)=>{for(let i=lower(xs,x)+1;i<tree.length;i+=i&-i)tree[i]+=value;};
  const sum=i=>{let n=0;for(;i>0;i-=i&-i)n+=tree[i];return n;};
  const events=[];
  for(const v of vertical)if(v.start<v.end){events.push({y:v.start,type:2,v},{y:v.end,type:0,v});}
  for(const h of horizontal)events.push({y:h.fixed,type:1,h});
  events.sort((a,b)=>a.y-b.y||a.type-b.type);let total=0;
  for(const event of events){if(event.type===1){const h=event.h;total+=sum(lower(xs,h.end))-sum(upper(xs,h.start));}else add(event.v.fixed,event.type===2?1:-1);}
  return total;
}
export function routingMetrics(f){
 const horizontal=[],vertical=[],groups=new Map();let length=0;
 for(let s=0;s<f.manifest.segments;s++){
  const i=s*3,a=f.segmentSources,b=f.segmentTargets,isVertical=a[i]===b[i],axis=isVertical?1:0;
  const interval={start:Math.min(a[i+axis],b[i+axis]),end:Math.max(a[i+axis],b[i+axis]),fixed:a[i+(isVertical?0:1)],owner:f.segmentBranches[s]};
  (isVertical?vertical:horizontal).push(interval);length+=interval.end-interval.start;
  const key=`${axis}/${interval.fixed}`;if(!groups.has(key))groups.set(key,[]);groups.get(key).push(interval);
 }
 let pairs=0,overlapLength=0;
 for(const intervals of groups.values()){
  intervals.sort((a,b)=>a.start-b.start);const ends=[...intervals].sort((a,b)=>a.end-b.end),active=new Map();let cursor=0,count=0;
  const events=[];
  for(const interval of intervals){
   while(cursor<ends.length&&ends[cursor].end<=interval.start){const e=ends[cursor++];active.set(e.owner,active.get(e.owner)-1);count--;}
   pairs+=count-(active.get(interval.owner)??0);count++;active.set(interval.owner,(active.get(interval.owner)??0)+1);
   events.push([interval.start,1],[interval.end,-1]);
  }
  events.sort((a,b)=>a[0]-b[0]||a[1]-b[1]);let occupancy=0,previous=0;
  for(const [x,delta]of events){if(occupancy>1)overlapLength+=x-previous;occupancy+=delta;previous=x;}
 }
 const bars=[];
 for(let u=0;u<f.manifest.count;u++)bars.push({fixed:f.positions[u*3],start:f.busbarSources?.[u*3+1]??f.positions[u*3+1]-1,end:f.busbarTargets?.[u*3+1]??f.positions[u*3+1]+1});
 const b=f.manifest.bounds;
 return {segments:f.manifest.segments,geometryBytes:f.manifest.bytes,width:b[2]-b[0],height:b[3]-b[1],collinearOverlapPairs:pairs,collinearOverlapLength:overlapLength,properRouteCrossings:crossings(horizontal,vertical),horizontalWireBusbarInteriorCrossings:crossings(horizontal,bars),routeLength:length,segmentsPerBranch:f.manifest.segments/f.manifest.branches};
}
