# Texas7k reconversion and island audit — 30 October 2026

**Current case:** the requested main-island derivative is in
`data/prototypes/texas7k-main-island/`. It contains one supplied component,
22,076 buses and all 15,299 loads. Five unsupplied buses, three lines, three
newly unused line codes and five coordinate features have been removed.
The full reconversion below remains the parent and audit reference.

The compact shipping bundle and its native/browser checks are described in
[SHIPPING.md](SHIPPING.md).

`main-island-validation.json` records a successful ordinary native solve,
without island-selection metadata. Every numerical result and equipment port
matches the full parent's explicitly selected solve exactly. No load, source,
transformer, surviving electrical value or remaining coordinate changed.
The derivative's source notice describes the changes and retains CC-BY-3.0-US
attribution. Its removal sidecar preserves the removed records and parent hash.

The two unsupplied islands are present in the original OpenDSS equipment.
They are short line sections disconnected by disabled pad switches at both
ends. Neither island contains a load, transformer, or voltage source. OpenDSS
assigns exactly zero voltage to their 15 phase nodes. The conversion did not
create them.

The full `p1uhs0_1247` six-feeder case was regenerated using the latest published
[PowerIO 0.11.7](https://github.com/eigenergy/powerio/releases/tag/v0.11.7),
release commit `d104aa5b` (full SHA in `software_provenance.json`). All 62 source
file sizes and SHA-256 hashes match the original manifest. Scripts and source
files in BMOPFDraftData were read without modification.

## Connectivity findings

| Component | Original OpenDSS buses | Converted buses | Loads |
| --- | ---: | ---: | ---: |
| Supplied substation and six feeders | 19,593 | 22,076 | 15,299 |
| Disconnected `p1udt1373–p1udt3224` section | 2 | 2 | 0 |
| Disconnected `p1udt4106–p1umv2` section | 3 | 3 | 0 |

The first disconnected section contains `l(r:p1udt1373-p1udt3224)`, between
the disabled switches ending in `p1u_27814` and `p1u_27815`. Its source
definitions are in `p1uhs0_1247--p1udt1566/Lines.dss`, lines 7557, 9041 and 9043.

The second contains `l(r:p1udt4106-p1umv2)_s0` and `_s1`, between disabled
switches ending in `p1u_27820` and `p1u_27821`. Its source definitions are in
`p1uhs0_1247--p1udt1746/Lines.dss`, lines 3693, 3695, 4487 and 4489.

These switches explicitly state `enabled=n`; this is distinct from an enabled
switch with an open conductor. There are 20 disabled line/switch definitions
in the source overall. The adapter excludes those disabled branches from the
electrical model while retaining their effective parameters in the parameter
archive. It preserves the three unsupplied line sections.

`connectivity.json` independently checks bus connectivity through enabled,
closed lines and every transformer winding, without joining buses through
ground. The same five unsupplied bus identities occur in original OpenDSS,
the normalized OpenDSS adapter input, the previous BMOPF file and the new BMOPF
file. Both OpenDSS stages have exactly zero voltage on those buses. This is a
bus connectivity audit; the voltage comparison supplies the conductor-level
numerical check.

## Updated conversion

PowerIO 0.11.7 materializes finite OpenDSS source impedance itself. The old
adapter manually inserted that impedance and replaced the DSS source with a
tiny `1e-12` ohm source. That workaround is removed from this rerun. PowerIO
now supplies `_pio_vsource_source_emf` and `_pio_vsource_source_impedance`,
replacing `__source_internal` and `__source_impedance`. The generated internal
bus inherits the original source location. Original physical bus, line and
load identities remain.

The remaining bounded adapter steps are retained: nominal written loads,
solved then frozen regulator taps, finite closed-switch impedances, asymmetric
split-phase leakage-star equivalents, secondary-tap impedance rescaling,
continuous line ratings and geographic coordinates. The converted load and
transformer tables are exactly equal to the previous pilot's tables.

PowerIO now emits a separate effective line code per line: 16,822 records
instead of 2,440. The regenerated formatted BMOPF is 44,920,224 bytes. This
run does not apply the PR's representation reducer or change its staged
dataset pin.

## Validation

Latest PowerIO requires explicit handling of source-less islands before
constructing a calculation instance. A direct full-network `mc_pf` call
correctly rejects this case. The custom runner uses Tellegen's explicit
energized-network selection, preserves the full portable input, and records
the two excluded islands: five buses, three lines, zero loads. No voltages
are fabricated for the excluded buses.

| Check | Result |
| --- | --- |
| BMOPF 0.2.0 electrical schema projection | Pass |
| Geographic coordinates | All 22,081 buses checked |
| Supplied OpenDSS phase nodes compared | 38,203 |
| Unsupplied OpenDSS phase nodes separately checked | 15; exactly zero |
| Maximum complex-voltage error vs original OpenDSS | 2.2369021453e-6 pu |
| Maximum error with only OpenDSS numerical antifloat shunts removed | 1.3476659803e-8 pu |
| Native PF | Converged in 23 iterations, one factorization |
| Matrix dimension | 40,685, excluding 15 unsupplied phase unknowns |
| Physical KCL residual | 5.3048133850e-6 A |
| Source import | 102.828995 MW / 46.278951 MVAr |

The error thresholds remain the original pilot's 3e-6 pu and 5e-8 pu. Source
import differs from original OpenDSS by about +6.98 W and -572.86 var. All
15,299 loads are retained with their original constant-power law. The case
still has voltage and loading violations; its operating point is unchanged
within the comparison tolerance.

Native validation uses fresh Tellegen checkout
`7f1852cc9de537c3abf72a44b916c975c37f6114`, with published PowerIO 0.11.7
dependencies and the companion `energized-runner.rs`. This checkout includes
explicit island-selection support and is newer than PR #169's current head.
This evidence does not claim that the PR's existing browser build was rerun.
Both executable hashes, Python versions and the original workflow hashes are
recorded. `direct-powerio-summary.json` additionally confirms that 0.11.7 reads
the original frozen equipment without the transformer adapter; it is an import
inventory, not a PF qualification of that direct representation.

## Local artifacts and reproduction

Generated artifacts are under the ignored directory
`data/prototypes/texas7k-powerio-0.11.7/`: full BMOPF, full portable PowerIO
module with explicit solve selection, compressed references, diagnostics,
transformer map, PF figures, options, source notice and license. The original
BMOPFDraftData pilot remains unchanged. `artifact-manifest.json` pins the
generated deliverables. No hosted data was staged or deployed.

Use a fresh Python 3.12 environment with
`BMOPFDraftData/scripts/Texas7k/requirements.txt`. Build the PowerIO CLI at tag
`v0.11.7` with `cargo build --release --locked -p powerio-cli`, and use its
vendored `powerio-dist/schemas/bmopf/0.2.0/bmopf.schema.json`.

From this checkout, select the native validation checkout explicitly:

```sh
python evidence/studies/texas7k-reconversion/build-runner.py \
  --tellegen-root /path/to/tellegen-at-7f1852cc \
  --build-dir /tmp/texas7k-runner-build \
  --target-dir /tmp/texas7k-runner-target

python evidence/studies/texas7k-reconversion/rerun.py \
  --bmopf-root /path/to/BMOPFDraftData \
  --workdir /tmp/texas7k-fresh-run \
  --powerio /path/to/powerio-0.11.7/target/release/powerio \
  --schema /path/to/powerio-0.11.7/powerio-dist/schemas/bmopf/0.2.0/bmopf.schema.json \
  --runner /tmp/texas7k-runner-target/release/texas7k-energized-runner

python evidence/studies/texas7k-reconversion/audit-connectivity.py \
  --snapshot /tmp/texas7k-fresh-run/output/fixed_snapshot.dss \
  --normalized-snapshot /tmp/texas7k-fresh-run/output/normalized_snapshot.dss \
  --bmopf /tmp/texas7k-fresh-run/output/p1uhs0_1247.bmopf.json \
  --previous /path/to/BMOPFDraftData/output/Texas7kPilot/p1uhs0_1247.bmopf.json \
  --output /tmp/texas7k-fresh-run/connectivity.json
```

`rerun.py` creates a private workflow copy and applies checked replacements;
it refuses reused working directories. BMOPFDraftData revision
`01986ddd858e6836f063faf403e36ab012fc0ae2` supplies the workflow used here;
the source manifest is verified again before every conversion.

## Reproduce the main-island derivative

After the full conversion, run the extraction into a fresh directory. The
ordinary `mc_pf` example and the full-case validation runner should use the
same native Tellegen checkout and PowerIO 0.11.7 dependencies:

```sh
python evidence/studies/texas7k-reconversion/extract-main-island.py \
  --source /tmp/texas7k-fresh-run/output/p1uhs0_1247.bmopf.json \
  --output /tmp/texas7k-main-island \
  --schema /path/to/powerio-0.11.7/powerio-dist/schemas/bmopf/0.2.0/bmopf.schema.json \
  --powerio /path/to/powerio-0.11.7/target/release/powerio \
  --mc-pf /path/to/tellegen-at-7f1852cc/target/release/examples/mc_pf \
  --full-runner /tmp/texas7k-runner-target/release/texas7k-energized-runner
```

The source directory must also contain the license, source notice and solver
options. The local full-conversion artifact directory already includes them;
when starting from the raw rerun directory, copy `License.md` and
`SOURCE_NOTICE.md` from the original pilot first. The helper verifies the
electrical schema projection, coordinates, surviving records, connectivity,
zero PowerIO ingestion diagnostics and paired PF results before writing its
validation report. It emits a fresh portable module with no island-selection
extension; no excluded equipment remains in the electrical tables.
