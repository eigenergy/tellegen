/** Offline O((segments + glyphs) log(segments + glyphs)) clearance audit. */
const lower=(a,x)=>{let l=0,r=a.length;while(l<r){const m=(l+r)>>1;if(a[m]<x)l=m+1;else r=m;}return l;};
const upper=(a,x)=>{let l=0,r=a.length;while(l<r){const m=(l+r)>>1;if(a[m]<=x)l=m+1;else r=m;}return l;};
export function equipmentClearanceMetrics(f){
 const boxes=Array.from(f.equipmentGlyphOwners??[],(owner,i)=>({owner,x:f.equipmentCenters[i*3],y:f.equipmentCenters[i*3+1]}));
 const own=new Map(boxes.map(b=>[b.owner,b]));let collisions=0;
 for(const axis of [0,1]){
  const queries=[];
  for(let s=0;s<f.manifest.segments;s++){
   const a=f.segmentSources,b=f.segmentTargets,k=s*3;if(a[k+1-axis]!==b[k+1-axis])continue;
   const start=Math.min(a[k+axis],b[k+axis]),end=Math.max(a[k+axis],b[k+axis]);if(start===end)continue;
   queries.push({start,end,fixed:a[k+1-axis],owner:f.segmentBranches[s]});
  }
  const ys=[...new Set(queries.map(q=>q.fixed))].sort((a,b)=>a-b);
  for(const [endpoint,bound,sign] of [['end',-16,1],['start',16,-1]]){
   const sorted=[...boxes].sort((a,b)=>(axis?a.y:a.x)-(axis?b.y:b.x)),ordered=[...queries].sort((a,b)=>a[endpoint]-b[endpoint]);
   const tree=new Float64Array(ys.length+2),add=(index,v)=>{for(let i=index+1;i<tree.length;i+=i&-i)tree[i]+=v;},sum=index=>{let v=0;for(let i=index+1;i>0;i-=i&-i)v+=tree[i];return v;};
   let cursor=0;
   for(const q of ordered){
    while(cursor<sorted.length&&((axis?sorted[cursor].y:sorted[cursor].x)+bound<q[endpoint]||(sign===-1&&(axis?sorted[cursor].y:sorted[cursor].x)+bound===q[endpoint]))){const b=sorted[cursor++],y=axis?b.x:b.y;add(upper(ys,y-16),1);add(lower(ys,y+16),-1);}
    collisions+=sign*sum(lower(ys,q.fixed));
   }
  }
  // A route's own lead is allowed to enter its reserved padding to meet its symbol.
  for(const q of queries){const b=own.get(q.owner);if(!b)continue;const x=axis?b.y:b.x,y=axis?b.x:b.y;if(q.start<x+16&&q.end>x-16&&q.fixed>y-16&&q.fixed<y+16)collisions--;}
 }
 return {reservedBoxHalfSize:16,foreignRouteBoxIntersections:collisions,glyphs:boxes.length};
}
