/** Bounded local lane permutation. Tree lanes retain their relative order. */
export const MAX_ORDERED_LANES=32;
export const ORDERING_PASSES=4;
const lower=(a,x)=>{let l=0,r=a.length;while(l<r){const m=(l+r)>>>1;if(a[m]<x)l=m+1;else r=m;}return l;};
const upper=(a,x)=>{let l=0,r=a.length;while(l<r){const m=(l+r)>>>1;if(a[m]<=x)l=m+1;else r=m;}return l;};
const inside=(a,lo,hi)=>hi>lo?lower(a,hi)-upper(a,lo):0;
export function orderCost(order,cost){let total=0;for(let i=0;i<order.length;i++)for(let j=i+1;j<order.length;j++)total+=cost[order[i]][order[j]];return total;}
export function improveOrder(cost,treeLanes){
  const n=cost.length,order=Array.from({length:n},(_,i)=>i);let moves=0;
  // Stable IDs and strict improvements make equal-cost cases deterministic.
  for(let pass=0;pass<ORDERING_PASSES;pass++){
    let changed=false;
    for(let k=treeLanes;k<n;k++){
      const id=pass%2?n-1-(k-treeLanes):k,at=order.indexOf(id);let best=at,bestDelta=0,delta=0;
      for(let p=at-1;p>=0;p--){const other=order[p];delta+=cost[id][other]-cost[other][id];if(delta<bestDelta){best=p;bestDelta=delta;}}
      delta=0;
      for(let p=at+1;p<n;p++){const other=order[p];delta+=cost[other][id]-cost[id][other];if(delta<bestDelta){best=p;bestDelta=delta;}}
      if(best!==at){order.splice(at,1);order.splice(best,0,id);changed=true;moves++;}
    }
    if(!changed)break;
  }
  return {order,moves};
}
export function crossingAwareOrder(corridor,{model,ports,depth,external}){
  const intervals=corridor.intervals,n=corridor.count;
  if(!intervals.some(i=>i.bank===2))return {skipped:'radial'};
  if(n>MAX_ORDERED_LANES)return {skipped:'lane-cap'};
  // Gap placement and exterior escape bends need a separate cost model.
  if(intervals.some(i=>external[Math.floor(i.id/2)]||model.branches[Math.floor(i.id/2)].disabled))return {skipped:'external-or-disabled'};
  const lanes=Array.from({length:n},()=>({left:[],right:[],vertical:[]}));let treeLanes=0;
  for(const i of intervals){
    const b=Math.floor(i.id/2),edge=model.branches[b],a=ports[b*6+1],z=ports[b*6+4],lane=lanes[i.lane];
    lane.vertical.push([Math.min(a,z),Math.max(a,z)]);
    lane[depth[edge.from]===corridor.rank?'left':'right'].push(a);
    lane[depth[edge.to]===corridor.rank?'left':'right'].push(z);
    if(i.bank!==2)treeLanes=Math.max(treeLanes,i.lane+1);
  }
  for(const lane of lanes){lane.left.sort((a,b)=>a-b);lane.right.sort((a,b)=>a-b);}
  // cost[a][b]: crossings between lanes a and b when a lies to b's left.
  // Count strict interior intersections via sorted endpoint rows, never edge pairs.
  const cost=Array.from({length:n},()=>new Float64Array(n));
  for(let a=0;a<n;a++)for(let b=0;b<n;b++)if(a!==b){
    for(const [lo,hi]of lanes[a].vertical)cost[a][b]+=inside(lanes[b].left,lo,hi);
    for(const [lo,hi]of lanes[b].vertical)cost[a][b]+=inside(lanes[a].right,lo,hi);
  }
  const before=orderCost(Array.from({length:n},(_,i)=>i),cost),{order,moves}=improveOrder(cost,treeLanes),after=orderCost(order,cost);
  const rank=new Uint32Array(n);order.forEach((id,i)=>rank[id]=i);
  for(const interval of intervals)interval.lane=rank[interval.lane];
  return {before,after,moves,changed:moves>0};
}
