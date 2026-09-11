import type {
  DistEdgeKind,
  DistNetworkDetails,
  NetworkBranch,
  NetworkBus,
} from "@tellegen/engine";

export type InspectorRecord = Record<string, unknown>;

export interface InspectorRecordGroup {
  label: string;
  records: InspectorRecord[];
}

export interface ElementModelDetails {
  family: "transmission" | "distribution";
  kind: "bus" | "edge";
  typeLabel: string;
  id: string;
  record: InspectorRecord | null;
  related: InspectorRecordGroup[];
}

function object(value: unknown): InspectorRecord | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as InspectorRecord)
    : null;
}

function records(value: unknown): InspectorRecord[] {
  return Array.isArray(value)
    ? value.flatMap((entry) => (object(entry) ? [object(entry)!] : []))
    : [];
}

function sameId(left: unknown, right: unknown): boolean {
  return String(left).toLowerCase() === String(right).toLowerCase();
}

/** Return the canonical balanced-network payload from a retained PowerIO IR.
 * Invalid or older documents simply produce no raw record; the inspector still
 * renders the map projection and live solution in that case. */
export function transmissionNetworkData(
  moduleJson: string | null | undefined,
): InspectorRecord | null {
  if (!moduleJson) return null;
  try {
    const root = object(JSON.parse(moduleJson));
    return object(object(root?.value)?.data) ?? object(root?.data) ?? null;
  } catch {
    return null;
  }
}

function transmissionBusRecord(
  data: InspectorRecord,
  bus: NetworkBus,
): InspectorRecord | null {
  const buses = records(data.buses);
  return (
    buses.find((record) => sameId(record.id, bus.id)) ??
    (bus.uid
      ? buses.find((record) => sameId(record.uid, bus.uid))
      : undefined) ??
    null
  );
}

function transmissionBranchRecord(
  data: InspectorRecord,
  branch: NetworkBranch,
): InspectorRecord | null {
  const branches = records(data.branches);
  return (
    (branch.uid
      ? branches.find((record) => sameId(record.uid, branch.uid))
      : undefined) ??
    branches.find((record) => sameId(record.id, branch.id)) ??
    branches[branch.id] ??
    (branch.id > 0 ? branches[branch.id - 1] : undefined) ??
    null
  );
}

function atBus(
  data: InspectorRecord,
  table: string,
  busId: number,
): InspectorRecord[] {
  return records(data[table]).filter((record) => sameId(record.bus, busId));
}

export function transmissionBusDetails(
  moduleJson: string | null | undefined,
  bus: NetworkBus,
): ElementModelDetails {
  const data = transmissionNetworkData(moduleJson);
  const related = data
    ? [
        ["Loads", "loads"],
        ["Generators", "generators"],
        ["Shunts", "shunts"],
        ["Static var compensators", "static_var_compensators"],
      ]
        .map(([label, table]) => ({
          label,
          records: atBus(data, table, bus.id),
        }))
        .filter((group) => group.records.length > 0)
    : [];
  return {
    family: "transmission",
    kind: "bus",
    typeLabel: "bus",
    id: String(bus.id),
    record: data ? transmissionBusRecord(data, bus) : null,
    related,
  };
}

export function transmissionBranchDetails(
  moduleJson: string | null | undefined,
  branch: NetworkBranch,
): ElementModelDetails {
  const data = transmissionNetworkData(moduleJson);
  return {
    family: "transmission",
    kind: "edge",
    typeLabel: "line",
    id: branch.uid ?? String(branch.id),
    record: data ? transmissionBranchRecord(data, branch) : null,
    related: [],
  };
}

function namedRecord(
  details: DistNetworkDetails,
  table: string,
  id: string,
): InspectorRecord | null {
  return (
    records(details[table]).find((record) =>
      sameId(record.name ?? record.id, id),
    ) ?? null
  );
}

function distAtBus(
  details: DistNetworkDetails,
  table: string,
  busId: string,
): InspectorRecord[] {
  return records(details[table]).filter((record) => sameId(record.bus, busId));
}

export function distributionBusDetails(
  details: DistNetworkDetails,
  busId: string,
): ElementModelDetails {
  const related = [
    ["Loads", "loads"],
    ["Generators", "generators"],
    ["Inverter-based resources", "ibrs"],
    ["Shunts", "shunts"],
    ["Capacitors", "capacitors"],
    ["Voltage sources", "sources"],
  ]
    .map(([label, table]) => ({
      label,
      records: distAtBus(details, table, busId),
    }))
    .filter((group) => group.records.length > 0);
  const transformers = records(details.transformers).filter((record) =>
    records(record.windings).some((winding) => sameId(winding.bus, busId)),
  );
  if (transformers.length > 0)
    related.push({ label: "Transformers", records: transformers });
  return {
    family: "distribution",
    kind: "bus",
    typeLabel: "bus",
    id: busId,
    record:
      records(details.buses).find((record) => sameId(record.id, busId)) ?? null,
    related,
  };
}

export function distributionEdgeDetails(
  details: DistNetworkDetails,
  edgeKind: DistEdgeKind,
  edgeId: string,
): ElementModelDetails {
  const table =
    edgeKind === "line"
      ? "lines"
      : edgeKind === "switch"
        ? "switches"
        : "transformers";
  const record = namedRecord(details, table, edgeId);
  const related: InspectorRecordGroup[] = [];
  if (edgeKind === "line" && typeof record?.linecode === "string") {
    const linecode = namedRecord(details, "linecodes", record.linecode);
    if (linecode) related.push({ label: "Linecode", records: [linecode] });
  }
  return {
    family: "distribution",
    kind: "edge",
    typeLabel: edgeKind,
    id: edgeId,
    record,
    related,
  };
}

export function inspectorLabel(key: string): string {
  return key.replaceAll("_", " ").replace(/\bibr\b/i, "IBR");
}

export function inspectorValue(value: unknown): string | null {
  if (value === null || value === undefined) return "—";
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return String(value);
    return Math.abs(value) >= 1000
      ? value.toLocaleString(undefined, { maximumFractionDigits: 3 })
      : String(Number(value.toPrecision(7)));
  }
  if (typeof value === "string") return value || "—";
  if (
    Array.isArray(value) &&
    value.every((entry) =>
      ["string", "number", "boolean"].includes(typeof entry),
    )
  ) {
    return value.length === 0
      ? "—"
      : value.map((entry) => inspectorValue(entry)).join(", ");
  }
  return null;
}
