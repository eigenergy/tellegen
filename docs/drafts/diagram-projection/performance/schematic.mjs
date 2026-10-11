/** Deterministic topology layout. Electrical identities and endpoints are untouched. */
import {packArrays} from './fixture.mjs';
export const SCHEMATIC_VERSION=1;
export const NO_BRANCH=0xffffffff;
export const ROLE={TREE:0,CROSS_LINK:1,DISABLED:2};
export const SPACING={depth:120,leaf:24,component:160,panel:320};

export function makeSchematic(model){
  const started=performance.now(),n=model.buses.length,edges=model.branches,e=edges.length;
  const offsets=new Uint32Array(n+1);
  for(const b of edges){
    if(!model.buses[b.from]||!model.buses[b.to]||model.buses[b.from].source!==b.source||model.buses[b.to].source!==b.source)throw new Error('Layout requires valid source-local endpoints');
    if(!b.disabled){offsets[b.from+1]++;offsets[b.to+1]++;}
  }
  for(let i=1;i<=n;i++)offsets[i]+=offsets[i-1];
  const adjacency=new Uint32Array(offsets[n]),cursor=offsets.slice();
  // Adapter order is canonical by source/family/kind/original ID. No coordinate sorting.
  for(let b=0;b<e;b++)if(!edges[b].disabled){adjacency[cursor[edges[b].from]++]=b;adjacency[cursor[edges[b].to]++]=b;}
  const parent=new Uint32Array(n).fill(NO_BRANCH),parentBranch=new Uint32Array(n).fill(NO_BRANCH),depth=new Uint32Array(n),component=new Uint32Array(n);
  const roles=new Uint8Array(e);for(let b=0;b<e;b++)roles[b]=edges[b].disabled?ROLE.DISABLED:ROLE.CROSS_LINK;
  const queue=new Uint32Array(n),order=[],components=[];
  const declaredSources=new Set(model.attachments.filter(a=>a.family==='voltage_source'&&!a.disabled).map(a=>a.bus));
  const candidates=[...declaredSources].sort((a,b)=>a-b);
  for(let i=0;i<n;i++)if(!declaredSources.has(i))candidates.push(i);
  for(const root of candidates){
    if(parent[root]!==NO_BRANCH)continue;
    const c={root,source:model.buses[root].source,rootReason:declaredSources.has(root)?'voltage_source':'stable-ID fallback',buses:0,maxDepth:0};
    const ci=components.length;components.push(c);parent[root]=root;let head=0,tail=1;queue[0]=root;
    while(head<tail){
      const u=queue[head++];order.push(u);component[u]=ci;c.buses++;c.maxDepth=Math.max(c.maxDepth,depth[u]);
      for(let j=offsets[u];j<offsets[u+1];j++){
        const b=adjacency[j],edge=edges[b],v=edge.from===u?edge.to:edge.from;
        if(parent[v]!==NO_BRANCH)continue;
        parent[v]=u;parentBranch[v]=b;depth[v]=depth[u]+1;roles[b]=ROLE.TREE;queue[tail++]=v;
      }
    }
  }
  // Intrusive child lists avoid per-node arrays. Traversals are iterative, including long chains.
  const firstChild=new Int32Array(n).fill(-1),nextSibling=new Int32Array(n).fill(-1),lastChild=new Int32Array(n).fill(-1);
  for(const u of order)if(parent[u]!==u){const p=parent[u];if(firstChild[p]===-1)firstChild[p]=u;else nextSibling[lastChild[p]]=u;lastChild[p]=u;}
  const positions=new Float32Array(n*3),sourceHeights=new Float64Array(model.sources.length),sourceWidths=new Float64Array(model.sources.length);
  for(const c of components){
    const stack=[c.root];let leaf=0;
    while(stack.length){const u=stack.pop();positions[u*3]=depth[u]*SPACING.depth;
      if(firstChild[u]===-1)positions[u*3+1]=leaf++*SPACING.leaf;
      else{const children=[];for(let v=firstChild[u];v!==-1;v=nextSibling[v])children.push(v);for(let j=children.length-1;j>=0;j--)stack.push(children[j]);}
    }
    c.leaves=leaf;
  }
  for(let i=order.length-1;i>=0;i--){const u=order[i];if(firstChild[u]!==-1)positions[u*3+1]=(positions[firstChild[u]*3+1]+positions[lastChild[u]*3+1])/2;}
  // Stack disconnected components within their original source, then pack source panels.
  for(const c of components){const y=sourceHeights[c.source],width=c.maxDepth*SPACING.depth,height=(c.leaves-1)*SPACING.leaf;
    c.bounds=[0,y,width,y+height];sourceHeights[c.source]+=height+SPACING.component;sourceWidths[c.source]=Math.max(sourceWidths[c.source],width);
  }
  const columns=Math.ceil(Math.sqrt(model.sources.length)),panels=[],translations=[];
  const cellWidth=Math.max(...sourceWidths)+SPACING.panel,cellHeight=Math.max(...sourceHeights)+SPACING.panel;
  for(let s=0;s<model.sources.length;s++){const x=(s%columns)*cellWidth,y=Math.floor(s/columns)*cellHeight;translations.push([x,y]);panels.push({source:s,label:model.sources[s].label,bounds:[x-40,y-60,x+sourceWidths[s]+60,y+Math.max(0,sourceHeights[s]-SPACING.component)+40]});}
  for(let u=0;u<n;u++){const c=components[component[u]],t=translations[c.source];positions[u*3]+=t[0];positions[u*3+1]+=c.bounds[1]+t[1];}
  for(const c of components){const t=translations[c.source];c.bounds=c.bounds.map((v,i)=>v+t[i%2]);}
  const placed=performance.now(),ss=[],tt=[],owners=[],endpoints=new Uint32Array(e*2),states=new Uint8Array(e);
  function segment(a,b,owner){if(a[0]===b[0]&&a[1]===b[1])return;ss.push(a[0],a[1],0);tt.push(b[0],b[1],0);owners.push(owner);}
  for(let i=0;i<e;i++){
    const b=edges[i],a=[positions[b.from*3],positions[b.from*3+1]],z=[positions[b.to*3],positions[b.to*3+1]];
    endpoints.set([b.from,b.to],i*2);states[i]=Number(b.disabled);let points;
    if(b.from===b.to)points=[a,[a[0]+24,a[1]],[a[0]+24,a[1]-18],[a[0],a[1]-18],z];
    else if(roles[i]===ROLE.TREE){const mid=(a[0]+z[0])/2;points=a[1]===z[1]?[a,z]:[a,[mid,a[1]],[mid,z[1]],z];}
    else{
      // A separate routing channel makes non-tree links visible; crossings are not junctions.
      const stub=14,lane=Math.max(a[0],z[0])+36+(i%7)*4,dy=8+(i%3)*2;
      points=[a,[a[0]+stub,a[1]],[a[0]+stub,a[1]-dy],[lane,a[1]-dy],[lane,z[1]-dy],[z[0]+stub,z[1]-dy],[z[0]+stub,z[1]],z];
    }
    let gapSegment=-1,longest=0;
    if(b.disabled)for(let j=1;j<points.length;j++){const distance=Math.abs(points[j][0]-points[j-1][0])+Math.abs(points[j][1]-points[j-1][1]);if(distance>longest){longest=distance;gapSegment=j;}}
    for(let j=1;j<points.length;j++){
      const p=points[j-1],q=points[j];
      if(j===gapSegment){const gap=Math.min(8,longest/4),before=.5-gap/longest/2,after=.5+gap/longest/2;segment(p,p.map((v,k)=>v+(q[k]-v)*before),i);segment(p.map((v,k)=>v+(q[k]-v)*after),q,i);}
      else segment(p,q,i);
    }
  }
  const {buffer,sections}=packArrays({positions,endpoints,states,segmentSources:new Float32Array(ss),segmentTargets:new Float32Array(tt),segmentBranches:new Uint32Array(owners),parent,parentBranch,depth,component,branchRoles:roles});
  let x0=Infinity,y0=Infinity,x1=-Infinity,y1=-Infinity;
  for(const a of [positions,ss,tt])for(let i=0;i<a.length;i+=3){x0=Math.min(x0,a[i]);x1=Math.max(x1,a[i]);y0=Math.min(y0,a[i+1]);y1=Math.max(y1,a[i+1]);}
  const treeEdges=roles.reduce((sum,r)=>sum+(r===ROLE.TREE),0),disabledEdges=states.reduce((sum,r)=>sum+r,0);
  return {buffer,manifest:{version:1,count:n,branches:e,segments:owners.length,bytes:buffer.byteLength,sections,bounds:[x0-80,y0-140,x1+80,y1+80],coordinateSpace:'schematic',placement:'source-rooted spanning forest; orthogonal routing',coincidentBuses:0,coincidentBranches:edges.filter(b=>b.from===b.to).length,
    layoutMode:'schematic',panels,layout:{algorithm:'source-rooted-bfs-leaf-order',version:SCHEMATIC_VERSION,spacing:SPACING,rootRule:'First enabled voltage-source bus in canonical order per enabled component, then first canonical bus',orientation:'left to right; topological depth, not power flow',components,sourceBuses:[...declaredSources],treeEdges,crossLinks:e-treeEdges-disabledEdges,disabledEdges,maxDepth:components.reduce((max,c)=>Math.max(max,c.maxDepth),0),placementMs:placed-started,routingAndPackingMs:performance.now()-placed,totalMs:performance.now()-started}}};
}

export function validateSchematic(f){
  const m=f.manifest,n=m.count,roles=f.branchRoles;
  if(f.parent?.length!==n||f.parentBranch?.length!==n||f.depth?.length!==n||f.component?.length!==n||roles?.length!==m.branches)throw new Error('Invalid schematic arrays');
  const positions=new Set();let roots=0,trees=0,cross=0,disabled=0;
  for(let u=0;u<n;u++){
    const key=`${f.positions[u*3]},${f.positions[u*3+1]}`;if(positions.has(key))throw new Error('Overlapping schematic buses');positions.add(key);
    const p=f.parent[u],b=f.parentBranch[u];if(p>=n)throw new Error('Unknown parent');
    if(p===u){roots++;if(b!==NO_BRANCH||f.depth[u]!==0)throw new Error('Invalid root');}
    else if(b>=m.branches||roles[b]!==ROLE.TREE||f.states[b]||f.depth[p]+1!==f.depth[u]||f.component[p]!==f.component[u]||!((f.endpoints[b*2]===p&&f.endpoints[b*2+1]===u)||(f.endpoints[b*2]===u&&f.endpoints[b*2+1]===p)))throw new Error('Invalid tree parent');
  }
  for(let b=0;b<m.branches;b++){if(roles[b]===ROLE.TREE)trees++;else if(roles[b]===ROLE.CROSS_LINK)cross++;else if(roles[b]===ROLE.DISABLED)disabled++;else throw new Error('Unknown route role');if(Boolean(f.states[b])!==(roles[b]===ROLE.DISABLED))throw new Error('Incorrect disabled role');}
  if(trees!==n-roots||roots!==m.layout.components.length||trees!==m.layout.treeEdges||cross!==m.layout.crossLinks||disabled!==m.layout.disabledEdges)throw new Error('Forest counts mismatch');
  for(let s=0;s<m.segments;s++){const j=s*3;if(f.segmentSources[j]!==f.segmentTargets[j]&&f.segmentSources[j+1]!==f.segmentTargets[j+1])throw new Error('Non-orthogonal route');}
}
