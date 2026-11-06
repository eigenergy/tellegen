# ZIP and folder case collections

Drop a ZIP onto Tellegen, choose it with **Open a case file**, or use **Open
folder**. Folder drops preserve relative paths. Each ZIP has its own namespace:
a missing OpenDSS include cannot be supplied by another uploaded archive.

The chooser lists candidate paths and formats before parsing. Select the cases
you want and choose **Import selected cases**. Several masters may share the
same topology or profile files, and a MATPOWER or JSON comparison case remains
independently selectable. Identical circuit names do not hide their source paths.
Import does not run every discovered case. Close the chooser to inspect a case
and choose its calculation.

Successful imports remain available when another selection fails or you cancel
the remaining work. Missing dependencies are reported separately from malformed
cases and solver restrictions. Distribution cases show their existing snapshot
power-flow capability and the reason when they are view-only. Transport success
does not enable unsupported controls, load laws or transformer models.

Use **Choose a nonstandard entry or document group** for an unusual master
suffix or a format that needs an explicit choice. XML is not assumed to be one
CGMES network: choose XIIDM for independent networks, or explicitly group the
profiles belonging to one CGMES case. GO problem/solution grouping is likewise
explicit. Nested transport ZIPs are not recursively flattened; extract them and
open the folder. Format-owned native ZIPs can be selected with their reader
format. Unsupported structural values, such as time series in a snapshot-only
view, are reported rather than flattened.

Files are read locally. PowerIO owns discovery, dependency resolution and
electrical parsing; no OpenDSS commands are executed by the browser and no
missing file is fetched from the host or network. Geographic sidecars are not
attached to every case in a mixed archive. Import first, select the intended
case, and attach its geographic file using the existing sidecar workflow.

The chooser limits one batch to 4096 members and 64 MiB of uploaded/expanded
file bytes, at most 32 ZIP uploads and 32 selected imports per operation. ZIP
members have a 200:1 expansion-ratio limit. PowerIO also bounds inspection and
cumulative parser input work; repeated operations can exhaust a session budget.
Decoded electrical models may use more memory than their source files. Parsing
is serial. Cancellation terminates the collection worker; on a synchronous
fallback it discards the result when the current operation returns.

While the chooser is open, **Download original ZIP** returns the exact uploaded
bytes. **Export selected source ZIP** makes a new archive with an explicit scope:
selected members alone, or the entire permitted project (which can include
other cases). A single master by itself need not be a complete project. Closing
the chooser releases source collections. Imported PowerIO modules retain
structured source origin and diagnostics but do not embed the archive; saving
and restoring a module does not restore an original-ZIP download.

## OpenDSS snapshot power flow

Qualified OpenDSS imports can run **Studies → Run power flow** after import.
The initial profile covers normalized finite sources with grounded references,
explicit conductor impedances, bounded ZIP and exponential loads (including
OpenDSS voltage bands and ZIP dropout), and supported finite-leakage transformers.
An explicit fourth neutral conductor is supported. Source-defined load bounds
always take precedence over the optional solver envelope; disabling that option
does not remove the source model's own voltage bounds.

Passive definitions, profiles and monitoring records remain in the module.
This is a fixed equipment snapshot: daily/yearly simulation, control execution,
and an OpenDSS script's sequence of solves are not run. Active regulator/control
objects, unresolved geometry or dependencies, floating source references,
general three-winding transformers, and unqualified transformer settings remain
view-only with a reason. Convergence and operating voltage-limit compliance are
reported separately. A runnable model can still fail to converge.
