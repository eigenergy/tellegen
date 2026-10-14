/** Display-tree inspection only: never an electrical continuity or power-flow claim. */
export function nonTreeBranches(f) {
  if(!f.parent||!f.branchRoles)return new Uint32Array();
  const indices=[];
  for(let b=0;b<f.branchRoles.length;b++)if(f.branchRoles[b]===1&&!f.states[b])indices.push(b);
  return Uint32Array.from(indices);
}

/** Ordered path from a to b, visiting only ancestors up to their common ancestor. */
export function treePath(f,a,b) {
  if(!f.parent||!f.depth||!f.component)throw new Error('Display spanning forest unavailable');
  if(!Number.isInteger(a)||!Number.isInteger(b)||a<0||b<0||a>=f.parent.length||b>=f.parent.length)throw new Error('Invalid endpoint');
  if(f.component[a]!==f.component[b])return {connected:false,nodes:[],edges:[],ancestor:null};
  const left=[a],right=[b],leftEdges=[],rightEdges=[];let u=a,v=b;
  function ascend(n,nodes,edges) {
    const p=f.parent[n],edge=f.parentBranch[n];
    if(p===n||p>=f.parent.length||f.depth[p]+1!==f.depth[n]||edge>=f.manifest.branches)throw new Error('Invalid display parent');
    const x=f.endpoints[edge*2],y=f.endpoints[edge*2+1];
    if(f.states[edge]||f.branchRoles[edge]!==0||!((x===n&&y===p)||(y===n&&x===p)))throw new Error('Invalid tree branch');
    nodes.push(p);edges.push(edge);return p;
  }
  while(u!==v){
    if(f.depth[u]>=f.depth[v])u=ascend(u,left,leftEdges);
    else v=ascend(v,right,rightEdges);
  }
  return {connected:true,nodes:left.concat(right.slice(0,-1).reverse()),edges:leftEdges.concat(rightEdges.reverse()),ancestor:u};
}

export function inspectCircuit(f,branch) {
  if(!Number.isInteger(branch)||branch<0||branch>=f.manifest.branches||f.branchRoles?.[branch]!==1||f.states[branch])throw new Error('Select an enabled non-tree connection');
  const a=f.endpoints[branch*2],b=f.endpoints[branch*2+1];
  return {branch,endpoints:[a,b],...treePath(f,a,b)};
}

/** References resident coordinates; creates only route-local overlay objects. */
export function branchSegments(f,branches) {
  const result=[];
  for(const b of branches)for(let s=f.firstSegment[b];s<f.firstSegment[b+1];s++)result.push({source:f.segmentSources.subarray(s*3,s*3+3),target:f.segmentTargets.subarray(s*3,s*3+3)});
  return result;
}
export function overlayBounds(f,nodes,segments) {
  let x0=Infinity,y0=Infinity,x1=-Infinity,y1=-Infinity;
  function include(p){x0=Math.min(x0,p[0]);y0=Math.min(y0,p[1]);x1=Math.max(x1,p[0]);y1=Math.max(y1,p[1]);}
  for(const n of nodes){include(f.positions.subarray(n*3,n*3+3));if(f.busbarSources){include(f.busbarSources.subarray(n*3,n*3+3));include(f.busbarTargets.subarray(n*3,n*3+3));}}
  for(const s of segments){include(s.source);include(s.target);}
  return [x0,y0,x1,y1];
}
