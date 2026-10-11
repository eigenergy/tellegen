/** Bus-owned fan-out and interval-coloured corridors. No new electrical nodes.
 * Pure preprocessing: O((V+E) log(V+E)) time, O(V+E) storage.
 */
export const ROUTING_VERSION=1;
const PORT_GAP=4,LANE_GAP=8,MARGIN=16,BUS_CLEARANCE=12,COMPONENT_GAP=160,PANEL_GAP=320;
class Heap {
  constructor(compare){this.items=[];this.compare=compare;}
  get first(){return this.items[0];}
  push(value){const a=this.items;let i=a.length;a.push(value);while(i){const p=(i-1)>>1;if(this.compare(a[p],value)<=0)break;a[i]=a[p];i=p;}a[i]=value;}
  pop(){const a=this.items,out=a[0],last=a.pop();if(a.length){let i=0;while(i*2+1<a.length){let c=i*2+1;if(c+1<a.length&&this.compare(a[c+1],a[c])<0)c++;if(this.compare(last,a[c])<=0)break;a[i]=a[c];i=c;}a[i]=last;}return out;}
}
export function allocateLanes(intervals){
  intervals.sort((a,b)=>a.start-b.start||a.end-b.end||a.id-b.id);
  const active=new Heap((a,b)=>a.end-b.end||a.lane-b.lane),free=new Heap((a,b)=>a-b);let count=0;
  for(const interval of intervals){
    while(active.first&&active.first.end<interval.start)free.push(active.pop().lane);
    interval.lane=free.first===undefined?count++:free.pop();active.push({end:interval.end,lane:interval.lane});
  }
  return count;
}
export function corridorRouting({model,positions,depth,component,components,roles}){
  const n=model.buses.length,e=model.branches.length,ports=new Float32Array(e*6);
  const offsets=new Float64Array(e*2),halves=new Float64Array(n);
  const incident=Array.from({length:n},()=>[[],[]]);
  const external=new Uint8Array(e);
  for(let b=0;b<e;b++){
    const edge=model.branches[b],u=edge.from,v=edge.to;
    external[b]=Number(component[u]!==component[v]||Math.abs(depth[u]-depth[v])>1);
    for(let end=0;end<2;end++){
      const a=end?v:u,z=end?u:v,side=external[b]||depth[a]<=depth[z]?1:0,k=b*2+end;
      incident[a][side].push({k,other:z});
    }
  }
  for(let u=0;u<n;u++)for(const [side,list] of incident[u].entries()){
    // Quarter-unit stagger between left/right ports prevents opposite-side
    // horizontal leads from occupying the same row. Bus centres use whole units.
    list.sort((a,b)=>positions[a.other*3+1]-positions[b.other*3+1]||a.k-b.k);
    list.forEach((p,i)=>offsets[p.k]=(i-(list.length-1)/2)*PORT_GAP+(side?.25:.75));
    halves[u]=Math.max(halves[u],(list.length-1)*PORT_GAP/2+1,1);
  }
  // Uniform scaling per component preserves subtree shape and centering while
  // reserving enough vertical space for neighbouring busbars in each rank.
  const ranks=new Map(),scales=new Float64Array(components.length).fill(1);
  for(let u=0;u<n;u++){const key=`${component[u]}/${depth[u]}`;if(!ranks.has(key))ranks.set(key,[]);ranks.get(key).push(u);}
  for(const nodes of ranks.values()){
    nodes.sort((a,b)=>positions[a*3+1]-positions[b*3+1]||a-b);
    for(let i=1;i<nodes.length;i++){const u=nodes[i-1],v=nodes[i];scales[component[u]]=Math.max(scales[component[u]],(halves[u]+halves[v]+BUS_CLEARANCE)/(positions[v*3+1]-positions[u*3+1]));}
  }
  const low=new Float64Array(components.length).fill(Infinity),high=new Float64Array(components.length).fill(-Infinity);
  for(let u=0;u<n;u++){
    const c=component[u],y=Math.round((positions[u*3+1]-components[c].bounds[1])*scales[c]);
    positions[u*3+1]=y;low[c]=Math.min(low[c],y-halves[u]);high[c]=Math.max(high[c],y+halves[u]);
  }
  const heights=new Float64Array(model.sources.length),componentY=new Float64Array(components.length);
  components.forEach((c,i)=>{componentY[i]=heights[c.source]-low[i];heights[c.source]+=high[i]-low[i]+COMPONENT_GAP;});
  for(let u=0;u<n;u++)positions[u*3+1]+=componentY[component[u]];
  for(let b=0;b<e;b++)for(let end=0;end<2;end++){const u=end?model.branches[b].to:model.branches[b].from;ports[b*6+end*3+1]=positions[u*3+1]+offsets[b*2+end];}
  const corridors=new Map(),routes=new Array(e),outer=Array.from({length:model.sources.length},()=>[]);
  function request(source,rank,start,end,id){
    const key=`${source}/${rank}`;if(!corridors.has(key))corridors.set(key,{source,rank,intervals:[]});
    const b=Math.floor(id/2),edge=model.branches[b];
    const leftY=ports[b*6+(depth[edge.from]<=depth[edge.to]?1:4)],rightY=ports[b*6+(depth[edge.from]<=depth[edge.to]?4:1)];
    const bank=roles[b]===0?(leftY<=rightY?1:0):2;
    const interval={start:Math.min(start,end)-2,end:Math.max(start,end)+2,id,bank,fan:depth[edge.from]<=depth[edge.to]?edge.from:edge.to};corridors.get(key).intervals.push(interval);return interval;
  }
  // Long/disabled inter-component links use an exterior horizontal track and
  // two local escape corridors. Source panels remain electrically independent.
  for(let b=0;b<e;b++)if(external[b]){
    const edge=model.branches[b];outer[edge.source].push({start:Math.min(depth[edge.from],depth[edge.to]),end:Math.max(depth[edge.from],depth[edge.to])+1,id:b});
  }
  const outerCounts=outer.map(list=>allocateLanes(list)),outerY=new Float64Array(e);
  for(const list of outer)for(const item of list)outerY[item.id]=-MARGIN-(item.lane+1)*LANE_GAP;
  for(let b=0;b<e;b++){
    const edge=model.branches[b],u=edge.from,v=edge.to,a=ports[b*6+1],z=ports[b*6+4];
    routes[b]=external[b]?
      [request(edge.source,depth[u],a,outerY[b],b*2),request(edge.source,depth[v],z,outerY[b],b*2+1)]:
      [request(edge.source,Math.min(depth[u],depth[v]),a,z,b*2)];
  }
  let maxLanes=0,totalLanes=0;
  for(const corridor of corridors.values()){
    // Upward and downward tree fans need opposite track orders. Separate banks
    // preserve that order; mesh/disabled routes use a third bank. This avoids
    // creating sibling crossings just to remove shared tree trunks.
    let offset=0;
    for(let bank=0;bank<3;bank++){
      const intervals=corridor.intervals.filter(i=>i.bank===bank);
      let count=0;
      if(bank===2)count=allocateLanes(intervals);
      else{
        // Allocate the whole fan atomically: per-segment reuse could give a
        // nearer sibling a track outside its farther sibling and create a crossing.
        const fans=new Map();
        for(const interval of intervals){if(!fans.has(interval.fan))fans.set(interval.fan,{id:interval.fan,start:Infinity,end:-Infinity,items:[]});const f=fans.get(interval.fan);f.start=Math.min(f.start,interval.start);f.end=Math.max(f.end,interval.end);f.items.push(interval);}
        const active=new Heap((a,b)=>a.end-b.end||a.lane-b.lane),free=new Heap((a,b)=>a-b);
        for(const fan of [...fans.values()].sort((a,b)=>a.start-b.start||a.id-b.id)){
          while(active.first&&active.first.end<fan.start)free.push(active.pop().lane);
          fan.items.sort(bank===0?(a,b)=>a.start-b.start||a.id-b.id:(a,b)=>b.end-a.end||a.id-b.id);
          for(const item of fan.items){item.lane=free.first===undefined?count++:free.pop();active.push({end:fan.end,lane:item.lane});}
        }
      }
      for(const interval of intervals)interval.lane+=offset;
      offset+=count;
    }
    corridor.count=offset;maxLanes=Math.max(maxLanes,offset);totalLanes+=offset;
  }
  const maxRanks=new Uint32Array(model.sources.length);
  for(let u=0;u<n;u++){const s=model.buses[u].source;maxRanks[s]=Math.max(maxRanks[s],depth[u]);}
  const columns=Array.from(maxRanks,max=>new Float64Array(max+2)),widths=new Float64Array(model.sources.length);
  for(let s=0;s<columns.length;s++){
    for(let rank=0;rank<=maxRanks[s];rank++){const c=corridors.get(`${s}/${rank}`);columns[s][rank+1]=columns[s][rank]+Math.max(120,2*MARGIN+(c?.count??0)*LANE_GAP);if(c)for(const interval of c.intervals)interval.x=columns[s][rank]+MARGIN+interval.lane*LANE_GAP;}
    widths[s]=columns[s][maxRanks[s]+1];
  }
  const panelColumns=Math.ceil(Math.sqrt(model.sources.length));let cellWidth=0,cellHeight=0;
  for(let s=0;s<columns.length;s++){cellWidth=Math.max(cellWidth,widths[s]+PANEL_GAP);cellHeight=Math.max(cellHeight,heights[s]+(outerCounts[s]+1)*LANE_GAP+MARGIN+PANEL_GAP);}
  const translations=model.sources.map((_,s)=>[(s%panelColumns)*cellWidth,Math.floor(s/panelColumns)*cellHeight+(outerCounts[s]+1)*LANE_GAP+MARGIN]);
  const barSources=new Float32Array(n*3),barTargets=new Float32Array(n*3);
  for(let u=0;u<n;u++){
    const s=model.buses[u].source,[x,y]=translations[s];positions[u*3]=columns[s][depth[u]]+x;positions[u*3+1]+=y;
    barSources.set([positions[u*3],positions[u*3+1]-halves[u],0],u*3);barTargets.set([positions[u*3],positions[u*3+1]+halves[u],0],u*3);
  }
  for(let b=0;b<e;b++)for(let end=0;end<2;end++){const u=end?model.branches[b].to:model.branches[b].from;ports[b*6+end*3]=positions[u*3];ports[b*6+end*3+1]=positions[u*3+1]+offsets[b*2+end];}
  components.forEach((c,i)=>{const [x,y]=translations[c.source];c.bounds=[x,componentY[i]+low[i]+y,x+columns[c.source][c.maxDepth],componentY[i]+high[i]+y];});
  const panels=model.sources.map((s,i)=>{const [x,y]=translations[i];return {source:i,label:s.label,bounds:[x-40,y-MARGIN-(outerCounts[i]+1)*LANE_GAP-40,x+widths[i]+40,y+heights[i]-COMPONENT_GAP+40]};});
  function points(b){
    const [tx,ty]=translations[model.branches[b].source],p=[ports[b*6],ports[b*6+1]],q=[ports[b*6+3],ports[b*6+4]],lane=routes[b][0].x+tx;
    if(external[b]){const end=routes[b][1].x+tx,y=outerY[b]+ty;return [p,[lane,p[1]],[lane,y],[end,y],[end,q[1]],q];}
    if(p[1]===q[1]&&p[0]!==q[0])return [p,q];
    return [p,[lane,p[1]],[lane,q[1]],q];
  }
  return {points,panels,arrays:{branchPorts:ports,busbarSources:barSources,busbarTargets:barTargets},metadata:{algorithm:'bus-fan-out-interval-corridors',version:ROUTING_VERSION,portGap:PORT_GAP,laneGap:LANE_GAP,maxLanes,totalLanes,corridors:corridors.size,externalLinks:external.reduce((a,b)=>a+b,0),componentYScales:[...scales]}};
}
