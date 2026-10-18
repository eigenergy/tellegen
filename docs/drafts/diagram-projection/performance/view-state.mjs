/** Shareable view state in the query string. Selections use source identities, never array offsets. */
export const VIEW_PARAMS=['dataset','layout','routing','open','results','flows','sel','view','theme','mode','panel'];
export function encodeCamera(c){return [c.target[0].toFixed(1),c.target[1].toFixed(1),c.zoom.toFixed(2)].join(',');}
export function decodeCamera(text){
 const parts=String(text??'').split(',').map(Number);
 return parts.length===3&&parts.every(Number.isFinite)&&parts[2]>=-24&&parts[2]<=5?{target:[parts[0],parts[1],0],zoom:parts[2]}:null;
}
/** Merge state into a URL without dropping unrelated parameters; null/'' removes a key. */
export function withView(href,state){
 const url=new URL(href);
 for(const [k,v] of Object.entries(state))if(v===null||v===undefined||v==='')url.searchParams.delete(k);else url.searchParams.set(k,String(v));
 return url;
}
