/** Animated transition between two layouts of the same source buses, so the eye can follow a feeder between views. */
/** Above this size the per-frame interpolation and re-upload is unmeasured on real GPUs, so layouts switch without animation. */
export const MORPH_MS=900,MORPH_MAX_BUSES=25000;
export const ease=t=>t<.5?4*t*t*t:1-Math.pow(-2*t+2,3)/2;
/** Re-express layout A positions in layout B's world so each bus starts where it is on screen now. */
export function screenAnchored(positions,from,to){
 const k=2**from.zoom/2**to.zoom,out=new Float32Array(positions.length);
 for(let i=0;i<positions.length;i+=3){out[i]=to.target[0]+(positions[i]-from.target[0])*k;out[i+1]=to.target[1]+(positions[i+1]-from.target[1])*k;}
 return out;
}
export function lerpInto(out,a,b,t){for(let i=0;i<out.length;i++)out[i]=a[i]+(b[i]-a[i])*t;return out;}
/** Straight bus-to-bus chords while moving; routed geometry returns when the transition ends. */
export function chordsInto(sources,targets,endpoints,positions){
 for(let b=0;b<endpoints.length/2;b++){const u=endpoints[b*2]*3,v=endpoints[b*2+1]*3;for(let k=0;k<3;k++){sources[b*3+k]=positions[u+k];targets[b*3+k]=positions[v+k];}}
}
