/** Hand-authored projections of feeder.connectivity.json; not an auto-projector. */
import type {
  DiagramProjection, DiagramSymbol, DiagramPort, EquipmentRef, SourceRef,
  PinRef, WireMeaning,
} from "./tellegen.js";

const model = { network: "four-wire-feeder", revision: "fixture-v1" };
const terminals: Record<string, string[]> = {
  H: ["A", "B", "C"], S: ["A", "B", "C", "N"],
  F: ["A", "B", "C", "N"], L: ["A", "N"],
};
const families: Record<string, string> = {
  U: "source", T1: "transformer", L1: "line", L2: "line", Q3: "load", Q1: "load",
};
const eq = (key: string): EquipmentRef => ({ family: families[key], key });
const equipment = (key: string): SourceRef => ({ kind: "equipment", equipment: eq(key) });
const part = (key: string, part_kind: string, local: string): SourceRef =>
  ({ kind: "part", equipment: eq(key), part_kind, part: local });
const terminal = (bus: string, t: string): SourceRef =>
  ({ kind: "terminal", bus, terminal: t });
const pin = (key: string, local: string): PinRef => ({ equipment: eq(key), pin: local });
const point = (bus: string, t: string) => `${bus}.${t}`;

function empty(mode: DiagramProjection["mode"]): DiagramProjection {
  return {
    schema_version: 1, model_ref: model, mode,
    symbols: [], ports: [], connections: [], groups: [], decorations: [], notices: [],
  };
}

function symbol(p: DiagramProjection, id: string, glyph: DiagramSymbol["glyph"],
  sources: SourceRef[], label = id, group?: string, badges: string[] = []) {
  p.symbols.push({ id, glyph, label, sources, group, badges });
}

function port(p: DiagramProjection, id: string, owner: string, points: string[],
  pins: PinRef[] = [], grouping: DiagramPort["grouping"] = "individual") {
  p.ports.push({ id, symbol: owner, grouping, points, pins });
}

function connection(p: DiagramProjection, id: string, from: string, to: string,
  meaning: WireMeaning, sources: SourceRef[]) {
  p.connections.push({ id, from, to, meaning, sources });
}

function lead(p: DiagramProjection, id: string, from: string, to: string,
  sources: SourceRef[]) {
  connection(p, id, from, to, { kind: "attachment" }, sources);
}

function line(p: DiagramProjection, id: string, key: string, from: string, to: string,
  paths: string[]) {
  connection(p, id, from, to, { kind: "conductor", equipment: eq(key), paths },
    paths.map(t => part(key, "conductor", t)));
}

function ground(p: DiagramProjection, from: string) {
  const refs = [part("T1", "grounding", "GS")];
  symbol(p, "GS", "ground", refs, "source neutral ground");
  port(p, "GS.ref", "GS", ["earth"]);
  connection(p, "GS.bond", from, "GS.ref", { kind: "grounding", grounding: "GS" }, refs);
}

export const singleLine = empty("single_line");
for (const [bus, ts] of Object.entries(terminals)) {
  symbol(singleLine, bus, "bus_bundle", [{ kind: "bus", bus }]);
  port(singleLine, `${bus}.bundle`, bus, ts.map(t => point(bus, t)), [], "bundle");
}
symbol(singleLine, "U", "source", [equipment("U")], "upstream source");
port(singleLine, "U.out", "U", terminals.H.map(t => point("H", t)),
  terminals.H.map(t => pin("U", t)), "bundle");
symbol(singleLine, "T1", "transformer", [equipment("T1")], "T1", undefined, ["Δ / Yg"]);
for (const [end, bus] of [["p", "H"], ["s", "S"]] as const) {
  port(singleLine, `T1.${end}`, "T1", terminals[bus].map(t => point(bus, t)),
    terminals[bus].map(t => pin("T1", `${end}.${t}`)), "bundle");
}
for (const [load, bus] of [["Q3", "F"], ["Q1", "L"]] as const) {
  symbol(singleLine, load, "load", [equipment(load)], load,
    undefined, [load === "Q3" ? "Y, ABCN" : "A–N"]);
  port(singleLine, `${load}.in`, load, terminals[bus].map(t => point(bus, t)),
    terminals[bus].map(t => pin(load, t)), "bundle");
}
lead(singleLine, "U.lead", "U.out", "H.bundle", [equipment("U")]);
lead(singleLine, "T1.p.lead", "H.bundle", "T1.p", [part("T1", "end", "p")]);
lead(singleLine, "T1.s.lead", "T1.s", "S.bundle", [part("T1", "end", "s")]);
line(singleLine, "L1", "L1", "S.bundle", "F.bundle", terminals.S);
// The lateral occupies only A and N of the four-member F bundle.
line(singleLine, "L2", "L2", "F.bundle", "L.bundle", terminals.L);
lead(singleLine, "Q3.lead", "F.bundle", "Q3.in", [equipment("Q3")]);
lead(singleLine, "Q1.lead", "L.bundle", "Q1.in", [equipment("Q1")]);
// A separate port avoids representing all four phases as grounded.
port(singleLine, "S.neutral", "S", ["S.N"]);
ground(singleLine, "S.neutral");
singleLine.notices.push({
  sources: [equipment("T1"), equipment("Q3"), equipment("Q1")],
  reason: "collapsed", message: "Winding and load element detail is collapsed.",
}, {
  sources: [equipment("U")], reason: "unavailable",
  message: "Upstream equivalent internal connections are outside this fixture.",
});

export const multiWire = empty("multi_wire");
for (const [bus, ts] of Object.entries(terminals)) {
  symbol(multiWire, bus, "terminal_rails", [{ kind: "bus", bus }]);
  for (const t of ts) port(multiWire, point(bus, t), bus, [point(bus, t)]);
}
symbol(multiWire, "U", "source", [equipment("U")], "upstream source");
for (const t of terminals.H) {
  port(multiWire, `U.${t}`, "U", [point("H", t)], [pin("U", t)]);
  lead(multiWire, `U.${t}.lead`, `U.${t}`, point("H", t), [part("U", "pin", t)]);
}
multiWire.groups.push(
  { id: "T1", label: "T1", sources: [equipment("T1")] },
  { id: "T1.p", parent: "T1", label: "primary Δ", sources: [part("T1", "winding", "p")] },
  { id: "T1.s", parent: "T1", label: "secondary Y", sources: [part("T1", "winding", "s")] },
  { id: "Q3", label: "Q3", sources: [equipment("Q3")] },
);
for (const [winding, bus, pairs] of [
  ["p", "H", [["A", "B"], ["B", "C"], ["C", "A"]]],
  ["s", "S", [["A", "N"], ["B", "N"], ["C", "N"]]],
] as const) {
  for (const [a, b] of pairs) {
    const local = `${winding}${a}${b}`;
    const id = `T1.${local}`;
    const refs = [part("T1", "coil", local)];
    symbol(multiWire, id, "coil", refs, local, `T1.${winding}`, ["polarity unknown"]);
    for (const [side, t] of [["a", a], ["b", b]] as const) {
      port(multiWire, `${id}.${side}`, id, [point(bus, t)], [pin("T1", `${winding}.${t}`)]);
      lead(multiWire, `${id}.${side}.lead`, point(bus, t), `${id}.${side}`, refs);
    }
  }
}
// The model specifies a transformer but does not claim individual magnetic leg associations.
multiWire.decorations.push({
  id: "T1.coupling", kind: "magnetic_coupling",
  symbols: ["T1.pAB", "T1.pBC", "T1.pCA", "T1.sAN", "T1.sBN", "T1.sCN"],
  sources: [equipment("T1")],
});
for (const [key, from, to, ts] of [
  ["L1", "S", "F", terminals.S], ["L2", "F", "L", terminals.L],
] as const) {
  for (const t of ts) line(multiWire, `${key}.${t}`, key, point(from, t), point(to, t), [t]);
}
for (const [key, bus, phases] of [["Q3", "F", ["A", "B", "C"]], ["Q1", "L", ["A"]]] as const) {
  for (const phase of phases) {
    const id = `${key}.${phase}N`;
    const refs = [part(key, "element", `${phase}N`)];
    symbol(multiWire, id, "load", refs, id, key === "Q3" ? "Q3" : undefined);
    for (const [side, t] of [["a", phase], ["b", "N"]] as const) {
      port(multiWire, `${id}.${side}`, id, [point(bus, t)], [pin(key, t)]);
      lead(multiWire, `${id}.${side}.lead`, point(bus, t), `${id}.${side}`, refs);
    }
  }
}
ground(multiWire, "S.N");
multiWire.notices.push({
  sources: [equipment("U")], reason: "unavailable",
  message: "Upstream equivalent internal connections are outside this fixture.",
}, {
  sources: [equipment("T1")], reason: "unavailable",
  message: "Coil polarities and magnetic leg associations are unspecified; draw no polarity dots or pairwise coupling claims.",
});
