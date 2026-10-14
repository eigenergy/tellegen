/** Independent Buchheim-style tidy placement of an ordered spanning forest.
 * Dense indices are layout addresses only. No source identities or edges change.
 * The first walk emulates recursive call/return order: apportion a completed
 * child BEFORE entering its next sibling. A reversed postorder alone is wrong.
 */
export function tidyTreeLanes({parent, firstChild, nextSibling, lastChild, roots}) {
  const n=parent.length;
  const prelim=new Float64Array(n),mod=new Float64Array(n),shift=new Float64Array(n),change=new Float64Array(n);
  const previous=new Int32Array(n).fill(-1),thread=new Int32Array(n).fill(-1);
  const ancestor=new Int32Array(n),number=new Uint32Array(n),cursor=firstChild.slice(),fallback=new Int32Array(n);
  for(let u=0;u<n;u++){
    ancestor[u]=u;fallback[u]=firstChild[u];let prev=-1,ordinal=0;
    for(let v=firstChild[u];v!==-1;v=nextSibling[v]){previous[v]=prev;number[v]=ordinal++;prev=v;}
  }
  const left=u=>firstChild[u]===-1?thread[u]:firstChild[u];
  const right=u=>lastChild[u]===-1?thread[u]:lastChild[u];
  function apportion(u,defaultAncestor){
    const brother=previous[u];if(brother===-1)return defaultAncestor;
    let innerLeft=brother,innerRight=u,outerLeft=firstChild[parent[u]],outerRight=u;
    let il=mod[innerLeft],ir=mod[innerRight],ol=mod[outerLeft],or=mod[outerRight];
    while(right(innerLeft)!==-1&&left(innerRight)!==-1){
      innerLeft=right(innerLeft);innerRight=left(innerRight);outerLeft=left(outerLeft);outerRight=right(outerRight);
      ancestor[outerRight]=u;
      const delta=prelim[innerLeft]+il-prelim[innerRight]-ir+1;
      if(delta>0){
        // Constant-time ancestor membership; do not scan a high-degree child list.
        const a=parent[ancestor[innerLeft]]===parent[u]?ancestor[innerLeft]:defaultAncestor;
        const share=delta/(number[u]-number[a]);
        change[u]-=share;shift[u]+=delta;change[a]+=share;
        prelim[u]+=delta;mod[u]+=delta;ir+=delta;or+=delta;
      }
      il+=mod[innerLeft];ir+=mod[innerRight];ol+=mod[outerLeft];or+=mod[outerRight];
    }
    if(right(innerLeft)!==-1&&right(outerRight)===-1){thread[outerRight]=right(innerLeft);mod[outerRight]+=il-or;}
    else{
      if(left(innerRight)!==-1&&left(outerLeft)===-1){thread[outerLeft]=left(innerRight);mod[outerLeft]+=ir-ol;}
      defaultAncestor=u;
    }
    return defaultAncestor;
  }
  const lanes=new Float64Array(n),spans=[];
  for(const root of roots){
    let u=root;
    while(true){
      if(cursor[u]!==-1){u=cursor[u];continue;}
      const brother=previous[u];
      if(firstChild[u]===-1)prelim[u]=brother===-1?0:prelim[brother]+1;
      else{
        let totalShift=0,totalChange=0;
        for(let v=lastChild[u];v!==-1;v=previous[v]){prelim[v]+=totalShift;mod[v]+=totalShift;totalChange+=change[v];totalShift+=shift[v]+totalChange;}
        const midpoint=(prelim[firstChild[u]]+prelim[lastChild[u]])/2;
        prelim[u]=brother===-1?midpoint:prelim[brother]+1;mod[u]=prelim[u]-midpoint;
      }
      if(u===root)break;
      const p=parent[u];fallback[p]=apportion(u,fallback[p]);cursor[p]=nextSibling[u];u=p;
    }
    // Resolve accumulated modifiers in preorder, then normalise each component.
    const stack=[root];let min=Infinity,max=-Infinity;
    while(stack.length){const v=stack.pop();lanes[v]=prelim[v]+(v===root?0:mod[parent[v]]);mod[v]+=v===root?0:mod[parent[v]];min=Math.min(min,lanes[v]);max=Math.max(max,lanes[v]);for(let c=firstChild[v];c!==-1;c=nextSibling[c])stack.push(c);}
    stack.push(root);while(stack.length){const v=stack.pop();lanes[v]-=min;for(let c=firstChild[v];c!==-1;c=nextSibling[c])stack.push(c);}
    spans.push(max-min);
  }
  return {lanes,spans};
}
