import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { singleLine, multiWire } from '../feeder.ts';

const connectivity = JSON.parse(await readFile(new URL('../feeder.connectivity.json', import.meta.url), 'utf8'));
const output = await readFile(new URL('gallery-data.js', import.meta.url), 'utf8');
const data = JSON.parse(output.slice(output.indexOf('window.GALLERY_DATA = ') + 22).trim().replace(/;$/, ''));
// JSON round-tripping removes optional undefined properties in the TS builders.
assert.deepEqual(data, JSON.parse(JSON.stringify({ connectivity, singleLine, multiWire })));
const equipment = ref => connectivity.equipment.find(e => e.id.family===ref.family && e.id.key===ref.key);
const at = (eq, pin) => eq.pins.find(p => p.id===pin)?.binding.point;
const pointIds = new Set(connectivity.points.map(p=>p.id));
const partIds = (eq, kind) => {
  if(kind==='pin') return eq.pins.map(p=>p.id);
  if(kind==='end') return eq.ends.map(p=>p.id);
  if(kind==='conductor') return eq.model.paths.map(p=>p.id);
  if(kind==='coil') return eq.model.windings.flatMap(w=>w.coils.map(c=>c.id));
  if(kind==='winding') return eq.model.windings.map(w=>w.id);
  if(kind==='element') return eq.model.elements.map(e=>e.id);
  if(kind==='grounding') return connectivity.grounding.filter(g=>g.sources.some(s=>s.equipment?.key===eq.id.key)).map(g=>g.id);
  throw new Error(`Unsupported fixture part ${kind}`);
};
function validateSource(s) {
  if(s.kind==='bus') assert(connectivity.buses.some(b=>b.id===s.bus));
  else if(s.kind==='terminal') assert(connectivity.points.some(p=>p.sources.some(r=>r.kind==='terminal'&&r.bus===s.bus&&r.terminal===s.terminal)));
  else {const eq=equipment(s.equipment);assert(eq);if(s.kind==='part') assert(partIds(eq,s.part_kind).includes(s.part));}
}

for(const p of [singleLine,multiWire]) {
  assert.deepEqual(p.model_ref,connectivity.model_ref);
  const all=[...p.symbols,...p.ports,...p.connections,...p.groups,...p.decorations];
  assert.equal(new Set(all.map(x=>x.id)).size,all.length);
  const symbols=new Set(p.symbols.map(x=>x.id)),groups=new Set(p.groups.map(x=>x.id));
  const ports=new Map(p.ports.map(x=>[x.id,x]));
  for(const s of p.symbols)if(s.group)assert(groups.has(s.group));
  for(const g of p.groups)if(g.parent)assert(groups.has(g.parent));
  for(const port of p.ports) {
    assert(symbols.has(port.symbol));assert(port.points.every(id=>pointIds.has(id)));
    for(const pin of port.pins)assert(port.points.includes(at(equipment(pin.equipment),pin.pin)));
    if(p.mode==='multi_wire'){assert.equal(port.grouping,'individual');assert.equal(port.points.length,1);}
  }
  const conductors=[];
  for(const c of p.connections) {
    const from=ports.get(c.from),to=ports.get(c.to);assert(from&&to);
    if(c.meaning.kind==='attachment')assert.deepEqual([...from.points].sort(),[...to.points].sort());
    else if(c.meaning.kind==='conductor') {
      const eq=equipment(c.meaning.equipment);
      for(const id of c.meaning.paths) {
        const path=eq.model.paths.find(x=>x.id===id);assert(path);
        assert(from.points.includes(at(eq,path.from_pin)));assert(to.points.includes(at(eq,path.to_pin)));
        conductors.push(`${eq.id.key}/${id}`);
      }
    } else if(c.meaning.kind==='grounding') {
      const ground=connectivity.grounding.find(g=>g.id===c.meaning.grounding);assert(ground);
      assert.deepEqual(from.points,[ground.point]);assert.deepEqual(to.points,[ground.reference]);
    } else throw new Error('Unexpected fixture relationship');
  }
  assert.deepEqual(conductors.sort(),['L1/A','L1/B','L1/C','L1/N','L2/A','L2/N']);
  for(const item of [...p.symbols,...p.connections,...p.groups,...p.decorations,...p.notices])item.sources.forEach(validateSource);
  for(const d of p.decorations)assert(d.symbols.every(id=>symbols.has(id)));
  console.log(`${p.mode}: ${p.symbols.length} symbols, ${p.connections.length} connections; source mappings and grounding verified`);
}
assert.equal(connectivity.grounding.length,1);
assert.equal(connectivity.grounding[0].point,'S.N');
console.log('Generated gallery data matches both design projections.');
