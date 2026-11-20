# @tellegen/svelte

## 0.10.3

### Patch Changes

- [#440](https://github.com/eigenergy/tellegen/pull/440) [`f2a739d`](https://github.com/eigenergy/tellegen/commit/f2a739dd2758b3f57b188b1e6ff42a82fe781ec3) Thanks [@samtalki](https://github.com/samtalki)! - The engine builds on PowerIO 0.12.4. AC power flow on a case with HVDC lines or on a PowerIO module solves for the voltage at a bus coded PV whose generators are all out of service, and reports the bus in a `TRANSFORM.INSTANCE.ASSUMPTION` diagnostic; it held that bus at its stated voltage before. `@tellegen/svelte` is rebuilt on the new engine.
- Updated dependencies [[`f2a739d`](https://github.com/eigenergy/tellegen/commit/f2a739dd2758b3f57b188b1e6ff42a82fe781ec3)]:
  - @tellegen/engine@0.10.3

## 0.10.2

### Patch Changes

- [#436](https://github.com/eigenergy/tellegen/pull/436) [`16b60f3`](https://github.com/eigenergy/tellegen/commit/16b60f32e23aa32f03cc2729bfca0f9316e463f9) Thanks [@samtalki](https://github.com/samtalki)! - Decide an infeasible solve from the engine's error code and a permanent wasm load failure from the error type, reading the message text only when those are missing. Reading a Study's `bundle` or the journal's `records` copies less, and `@tellegen/svelte` exports the `plain` and `signed` number formatters.

- [#433](https://github.com/eigenergy/tellegen/pull/433) [`9dd99ae`](https://github.com/eigenergy/tellegen/commit/9dd99ae1a6593e22fcbb52df81b75717dd7d9b40) Thanks [@samtalki](https://github.com/samtalki)! - Say that written equipment settings also hold every fuse closed.
- Updated dependencies [[`16b60f3`](https://github.com/eigenergy/tellegen/commit/16b60f32e23aa32f03cc2729bfca0f9316e463f9), [`9411a0b`](https://github.com/eigenergy/tellegen/commit/9411a0b1cd70c931e52e1f8fe99f2c46bc5feddb)]:
  - @tellegen/engine@0.10.2
  - @tellegen/webmcp@0.4.2

## 0.10.1

### Patch Changes

- [#415](https://github.com/eigenergy/tellegen/pull/415) [`05c4a55`](https://github.com/eigenergy/tellegen/commit/05c4a557389da68422124e30c2450b7bc2a9b8ce) Thanks [@samtalki](https://github.com/samtalki)! - Show AC OPF price sensitivities for selected buses and branch limits using the dedicated browser solver, and cancel superseded selections.

- [#424](https://github.com/eigenergy/tellegen/pull/424) [`128a97c`](https://github.com/eigenergy/tellegen/commit/128a97c829c9fe5d7e1c76f7b0a6ca523037c15c) Thanks [@samtalki](https://github.com/samtalki)! - Cancel pending diagram label updates when switching cases so a replaced diagram cannot read another case's buses.

- [#423](https://github.com/eigenergy/tellegen/pull/423) [`4803fce`](https://github.com/eigenergy/tellegen/commit/4803fce75e660a26f4e8e7285c6eca22dff96d87) Thanks [@samtalki](https://github.com/samtalki)! - Separate case imports, balanced and distribution calculations, settings, and result queries behind the existing controller API.

- [#419](https://github.com/eigenergy/tellegen/pull/419) [`2644761`](https://github.com/eigenergy/tellegen/commit/2644761b8847d96ebd7dba706445be68516cf189) Thanks [@samtalki](https://github.com/samtalki)! - Validate browser and HTTP solve, multiconductor, Study and settings results against schemas generated from Rust before exposing them to application code. Refuse malformed streamed results while retaining the previous display. Generate MCP argument models from the same Rust definitions.

- [#405](https://github.com/eigenergy/tellegen/pull/405) [`e9dd0c1`](https://github.com/eigenergy/tellegen/commit/e9dd0c110f439e3edc7a698772dacde41e6ceee0) Thanks [@samtalki](https://github.com/samtalki)! - A generated grid reaches its first DC OPF result sooner. `BrowserSynthSession.ingestWithStudy()` starts the Study while the payload is prepared, and `openBalancedModule` takes that Study through its new `study` option.

- [#406](https://github.com/eigenergy/tellegen/pull/406) [`7087c1c`](https://github.com/eigenergy/tellegen/commit/7087c1cc8f1c203810402aa97b62f18c46073b4a) Thanks [@samtalki](https://github.com/samtalki)! - The toast and the Log tab show the same messages from one store, `app.log`, and the Log tab counts the errors and warnings not yet seen. The Edited mark in the case menu lines up and shows for local cases, and every calculation menu follows one rule, `calculationAvailable`.

- [#408](https://github.com/eigenergy/tellegen/pull/408) [`342cee6`](https://github.com/eigenergy/tellegen/commit/342cee66cc66cb88ce0432b7ec08ecc9c61b3eb0) Thanks [@samtalki](https://github.com/samtalki)! - Include ZIP and folder imports in browser builds and let users open ZIPs inside ZIPs without mixing their files or cases.

- [#423](https://github.com/eigenergy/tellegen/pull/423) [`be2ee8f`](https://github.com/eigenergy/tellegen/commit/be2ee8f4563c07f7d52e25f14b824a9e28a49502) Thanks [@samtalki](https://github.com/samtalki)! - Retain detailed node-breaker connectivity for drawings, add editable saved engineering drawings with portable JSON and SVG export, and move remaining source drawings to the shared diagram canvases.

- [#414](https://github.com/eigenergy/tellegen/pull/414) [`880ee1c`](https://github.com/eigenergy/tellegen/commit/880ee1c51f9ccb13a8809e8dae88aa0a724da18a) Thanks [@samtalki](https://github.com/samtalki)! - Use solve metadata to label feasibility results. Keep demo links and branding in the demo, scope shared styles to each provider, and provide distribution results for custom applications.

- [#409](https://github.com/eigenergy/tellegen/pull/409) [`6aa38e4`](https://github.com/eigenergy/tellegen/commit/6aa38e4f3d7e7f6361a9fe75b7eb2f15a054e8be) Thanks [@samtalki](https://github.com/samtalki)! - Keep saved studies in one browser database, migrate distribution saves, and support renaming and deleting. Reopening saved distribution results reuses the case without moving the camera.

- [#422](https://github.com/eigenergy/tellegen/pull/422) [`636e106`](https://github.com/eigenergy/tellegen/commit/636e106156297256a8daf718a3dad0acf822c389) Thanks [@samtalki](https://github.com/samtalki)! - Show generated synchronous areas, their planning basis and fixed synthetic HVDC transfers. Link area and converter-bus inspection to the viewer's existing selection.

- [#417](https://github.com/eigenergy/tellegen/pull/417) [`713a2a0`](https://github.com/eigenergy/tellegen/commit/713a2a09dfe2a6b268922391692f692548d45e7a) Thanks [@samtalki](https://github.com/samtalki)! - Offer an explicit written-equipment-settings action for qualified distribution cases. Create an independent case and show the choice after saving and reopening.
- Updated dependencies [[`85a787c`](https://github.com/eigenergy/tellegen/commit/85a787c8194702cbb833c9a12f9c795dfdd10e28), [`05c4a55`](https://github.com/eigenergy/tellegen/commit/05c4a557389da68422124e30c2450b7bc2a9b8ce), [`c3e6176`](https://github.com/eigenergy/tellegen/commit/c3e61769787ee87787d67d229149b81f560a531c), [`213e7c5`](https://github.com/eigenergy/tellegen/commit/213e7c520e99d55e77013d59efdb93c5cce80752), [`2644761`](https://github.com/eigenergy/tellegen/commit/2644761b8847d96ebd7dba706445be68516cf189), [`e9dd0c1`](https://github.com/eigenergy/tellegen/commit/e9dd0c110f439e3edc7a698772dacde41e6ceee0), [`7087c1c`](https://github.com/eigenergy/tellegen/commit/7087c1cc8f1c203810402aa97b62f18c46073b4a), [`342cee6`](https://github.com/eigenergy/tellegen/commit/342cee66cc66cb88ce0432b7ec08ecc9c61b3eb0), [`be2ee8f`](https://github.com/eigenergy/tellegen/commit/be2ee8f4563c07f7d52e25f14b824a9e28a49502), [`6aa38e4`](https://github.com/eigenergy/tellegen/commit/6aa38e4f3d7e7f6361a9fe75b7eb2f15a054e8be), [`a08743d`](https://github.com/eigenergy/tellegen/commit/a08743d7516412fe29f6995fa47c292675407d9b), [`f235130`](https://github.com/eigenergy/tellegen/commit/f23513037e05063987304eea918f1974f1ba1a64), [`8c867c4`](https://github.com/eigenergy/tellegen/commit/8c867c45b1e38f901eec1a8d8d47f04a060abe29), [`713a2a0`](https://github.com/eigenergy/tellegen/commit/713a2a09dfe2a6b268922391692f692548d45e7a), [`f0d89e4`](https://github.com/eigenergy/tellegen/commit/f0d89e4c07250b91c6dbd2c4076ff79b78f35be9)]:
  - @tellegen/engine@0.10.1
  - @tellegen/webmcp@0.4.1

## 0.10.0

### Minor Changes

- [#396](https://github.com/eigenergy/tellegen/pull/396) [`403248b`](https://github.com/eigenergy/tellegen/commit/403248b9a6c87af1adc933f76bfd56f0ae7bd4b1) Thanks [@samtalki](https://github.com/samtalki)! - Settings > Model > DC line model chooses how DC calculations model a line, with the earlier model as Default. The choice is kept in saved cases and studies and reported with each result.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`c2597bd`](https://github.com/eigenergy/tellegen/commit/c2597bd3e78319c237b09f61a0db78d842f1b490) Thanks [@samtalki](https://github.com/samtalki)! - Choosing a calculation runs it, the Log tab counts new messages, and on a phone the panel sheet has a handle to resize or close it. New exports: `PanelLayout.show(id)` and an optional `badge` on panels.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`81ed298`](https://github.com/eigenergy/tellegen/commit/81ed2984bb313a90fd9275d2c0be81dec1da72a6) Thanks [@samtalki](https://github.com/samtalki)! - Dynamics is now an experimental feature, off until Settings > Experimental features is turned on. While it is off, `.dyr` files are refused with a short message and agents are not offered `simulate_dynamics`; `AppState.dynamicsEnabled` and `UserPreferences.subscribe` are new.

- [#397](https://github.com/eigenergy/tellegen/pull/397) [`b4eab26`](https://github.com/eigenergy/tellegen/commit/b4eab26807dee1d4a2207ad9b019f40b9cb6e045) Thanks [@samtalki](https://github.com/samtalki)! - Add a Dynamics mode: attach a PSS/E `.dyr` file to a `.raw` case, or open the WECC 240 demo, pick an event and run an RMS simulation. The map plays the run back, with a timeline, traces and a result card.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`2bcc94d`](https://github.com/eigenergy/tellegen/commit/2bcc94dd25bae08764bd98caeb80e6c1bc97d347) Thanks [@samtalki](https://github.com/samtalki)! - Results show as labelled numbers, such as Operational cost and Lines at limit, and Settings is a list of short rows grouped by topic that apply and rerun at once. New export: `StatBoxes`.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`450a0f8`](https://github.com/eigenergy/tellegen/commit/450a0f8ce444d5d6b102ace665be5053c87d3430) Thanks [@samtalki](https://github.com/samtalki)! - New look: cool grey panels, a cobalt accent, red and amber only for line limits, and one case menu in the header in place of the case tabs. The tab strip and left dock form one panel, the solver result shows in the Network panel, and error notices name what failed with the full message under Details.

### Patch Changes

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`d8eabb5`](https://github.com/eigenergy/tellegen/commit/d8eabb5f25a021dc1e9e32dbd5818f00c721882b) Thanks [@samtalki](https://github.com/samtalki)! - AC power flow results show four boxes: losses, max loading, lowest voltage and highest voltage.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`b74af3b`](https://github.com/eigenergy/tellegen/commit/b74af3b2a68b297d1730d1240371aae218fa544e) Thanks [@samtalki](https://github.com/samtalki)! - When the case list fails to load, Details shows the server's status and message.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`723d12e`](https://github.com/eigenergy/tellegen/commit/723d12e0530fb7b0008632f65e8348b1652f2e1d) Thanks [@samtalki](https://github.com/samtalki)! - Only cases you added have a remove button in the case menu.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`a03fcab`](https://github.com/eigenergy/tellegen/commit/a03fcab12efe77033b943c0cc9317a26c8031af7) Thanks [@samtalki](https://github.com/samtalki)! - Coordinates that place every bus no longer raise a notice; the Log keeps the note.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`4ad0d4a`](https://github.com/eigenergy/tellegen/commit/4ad0d4a913be33945cf9612d53bf89576b5b9694) Thanks [@samtalki](https://github.com/samtalki)! - In dark mode, checked boxes and primary buttons use a deeper blue that reads as active. On touch screens only the open panel tab is highlighted.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`089b679`](https://github.com/eigenergy/tellegen/commit/089b67993a979f486e7dbdc85feb15178af2127b) Thanks [@samtalki](https://github.com/samtalki)! - The diagram's Bus values view has a title and Docs link, and its list is named Buses, Lines or Buses and lines. The Dynamics panel keeps its Case row and puts its actions below the rows.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`d1c990d`](https://github.com/eigenergy/tellegen/commit/d1c990dd9bad50c7aaaed9ba037b54619f065ddd) Thanks [@samtalki](https://github.com/samtalki)! - The diagram's Voltage card has a Docs link and a collapse button like the other cards, and loadings read with one decimal everywhere.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`2b96cea`](https://github.com/eigenergy/tellegen/commit/2b96ceabe0fd1918be1d5eb9ec5fe17c6d8b54c7) Thanks [@samtalki](https://github.com/samtalki)! - In the diagram a line at 100% loading has a red label in every view, and the view that follows the map's colours is named Bus values.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`846a094`](https://github.com/eigenergy/tellegen/commit/846a09492574a491fb6d9c0d2185133f9a29bbd0) Thanks [@samtalki](https://github.com/samtalki)! - Loading labels on the diagram spread out instead of covering each other: red at or over the rating, amber near it. Hover cards say Price instead of LMP.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`b4a7d09`](https://github.com/eigenergy/tellegen/commit/b4a7d09115a9d56ab22e417804eb8f2924417be4) Thanks [@samtalki](https://github.com/samtalki)! - In the diagram's line list, lines near their limit are amber. The voltage key names each group by its kV range, and an overload that rounds to 100.0% shows two decimals.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`0b6dd37`](https://github.com/eigenergy/tellegen/commit/0b6dd377372a4137d4061d8ed97b81f385268f49) Thanks [@samtalki](https://github.com/samtalki)! - The Dynamics panel shows the dynamic model as two counts, with the engine's notes in the Log. A local case's save, export and layout actions sit in one File menu.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`8c4cf93`](https://github.com/eigenergy/tellegen/commit/8c4cf930c66ae7982ad50d6197737ff111408f3f) Thanks [@samtalki](https://github.com/samtalki)! - Dynamics Details no longer repeat the result boxes, and the dynamics map legend appears only once a simulation has run.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`2c2d662`](https://github.com/eigenergy/tellegen/commit/2c2d6628cdb4f33cc6b336246e7c76250277b207) Thanks [@samtalki](https://github.com/samtalki)! - The Modes tab shows unstable modes and lowest damping as labelled numbers and plots each mode by damping and frequency.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`16267ff`](https://github.com/eigenergy/tellegen/commit/16267ffe3a1e1fbc9d312d79ae2ea8f886d61dcf) Thanks [@samtalki](https://github.com/samtalki)! - The Dynamics panel lists its data controls before Run simulation, and a result has one Details section.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`f1a78dd`](https://github.com/eigenergy/tellegen/commit/f1a78ddf856b11304c1bc5a96f42e1101540ef34) Thanks [@samtalki](https://github.com/samtalki)! - The dynamics colour key sits in the Network panel instead of over the map, and results use plain names such as Generators in sync.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`57585b0`](https://github.com/eigenergy/tellegen/commit/57585b0dbc9c81a1d6e3abfc0ae158d19c676895) Thanks [@samtalki](https://github.com/samtalki)! - Run simulation stays in view at the bottom of the Dynamics panel. Two dynamics rows have plainer names: Starts settled and Matches reference.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`283801e`](https://github.com/eigenergy/tellegen/commit/283801e275c76ec2a9863218b453c5e647db2e4e) Thanks [@samtalki](https://github.com/samtalki)! - Run simulation waits until an event is chosen, and the Event picker keeps showing the chosen scenario.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`da64381`](https://github.com/eigenergy/tellegen/commit/da64381cf3f823749fb9e57facf686103b4a0820) Thanks [@samtalki](https://github.com/samtalki)! - When a calculation fails, the status line and the notice name it, such as AC power flow failed, while the previous result stays on screen.

- [#390](https://github.com/eigenergy/tellegen/pull/390) [`32f7c3c`](https://github.com/eigenergy/tellegen/commit/32f7c3c5e9b93d8ef883a2e1db7ce6ba96f5cecb) Thanks [@samtalki](https://github.com/samtalki)! - A case opened without edits shows its first result sooner, because the browser no longer solves the base case twice or parses the case text three times.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`0d3a0f6`](https://github.com/eigenergy/tellegen/commit/0d3a0f6d7ca95947f898694451c235eab954939d) Thanks [@samtalki](https://github.com/samtalki)! - With no case loaded the map reaches the bottom of the page, and a failed case list offers Open a case.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`e09cf03`](https://github.com/eigenergy/tellegen/commit/e09cf0383a45830453f76cca168a2eca284b66b3) Thanks [@samtalki](https://github.com/samtalki)! - On a phone the dynamics playback bar sits under the open sheet. AC power flow is unavailable while a line rating is edited.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`b55a82f`](https://github.com/eigenergy/tellegen/commit/b55a82f1b6b27c61051dd113096ad943d406dfc1) Thanks [@samtalki](https://github.com/samtalki)! - Power flow results count Lines over limit and OPF results count Lines at limit, in the panel, map key and diagram. A case without costs shows no cost box, and selected bus and line values carry their units.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`d537bf6`](https://github.com/eigenergy/tellegen/commit/d537bf6db4d2a6c4eefa4807ba100d21c1223b99) Thanks [@samtalki](https://github.com/samtalki)! - The diagram's Limits panel counts lines Near limit, and the line rating slider shows its ends as ticks.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`e766651`](https://github.com/eigenergy/tellegen/commit/e7666518ced96f327280119fd748e98196a050e6) Thanks [@samtalki](https://github.com/samtalki)! - The map draws a line red only at its rating and amber near it, as the diagram does.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`4f6e7d6`](https://github.com/eigenergy/tellegen/commit/4f6e7d61d483ff0afd71058f9d849ba710e9e197) Thanks [@samtalki](https://github.com/samtalki)! - The Network panel lists the lines at their limit once, under the stat box, without a second heading. Stat box numbers are the same size in every panel.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`d87919a`](https://github.com/eigenergy/tellegen/commit/d87919a3566ee73f5bf1415a54841e635279abc6) Thanks [@samtalki](https://github.com/samtalki)! - A uniform price change reads as one labelled value, and on a phone Show diagram sits beside File.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`575e423`](https://github.com/eigenergy/tellegen/commit/575e423c77d41730175631d18280a9daddc1546d) Thanks [@samtalki](https://github.com/samtalki)! - AC power flow results show voltage range and losses, and a dropped case lists buses and lines with the rest under Case details.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`78365f6`](https://github.com/eigenergy/tellegen/commit/78365f6d964a94fd6e8f6f763fd53027bb31f7fb) Thanks [@samtalki](https://github.com/samtalki)! - The Bus values panel can be collapsed, and the solver chart shows its scale from 1 to 10⁻⁹.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`202c9b4`](https://github.com/eigenergy/tellegen/commit/202c9b4b8a29f68846a94685416105d749a19266) Thanks [@samtalki](https://github.com/samtalki)! - A notice's Details open inside the notice, and the Log lists an error by its short title with the raw message under Details.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`b4a7d09`](https://github.com/eigenergy/tellegen/commit/b4a7d09115a9d56ab22e417804eb8f2924417be4) Thanks [@samtalki](https://github.com/samtalki)! - Prices show two decimals everywhere: stat boxes, colour keys, bus readouts and map tooltips.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`30f15da`](https://github.com/eigenergy/tellegen/commit/30f15da4a68c1918729131d3661fe5b32868efd8) Thanks [@samtalki](https://github.com/samtalki)! - The command palette can run each calculation, and Zen mode is now called Hide panels.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`80dbcd6`](https://github.com/eigenergy/tellegen/commit/80dbcd60ffefe731206686c636f7d55ee5e5930c) Thanks [@samtalki](https://github.com/samtalki)! - On a phone the panel sheet runs edge to edge, is only as tall as its content, and leaves the map in full above it.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`b5aec80`](https://github.com/eigenergy/tellegen/commit/b5aec8038d091193f968b38fcb1368938a5fbfa7) Thanks [@samtalki](https://github.com/samtalki)! - Dynamics playback reads Time 1.038 s, and its legend names generator speed.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`9b3dd04`](https://github.com/eigenergy/tellegen/commit/9b3dd0445f57613f724edc3581602be9c5b083d5) Thanks [@samtalki](https://github.com/samtalki)! - A dynamics run shows its results as labelled numbers with pass or fail checks, and the details behind them under Details.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`e78a849`](https://github.com/eigenergy/tellegen/commit/e78a84953ebf53dc80dd440220d176bfe182ca92) Thanks [@samtalki](https://github.com/samtalki)! - Each Log entry is one short line with its full text under Details. The diagram's Limits panel shows only counts that are not zero.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`c00575c`](https://github.com/eigenergy/tellegen/commit/c00575c55ce9e2987f0065a3cd55a51160c97aa1) Thanks [@samtalki](https://github.com/samtalki)! - The diagram cards use the same stat boxes as the Network panel, and a line at or over its limit is red in every view. The SOCWR calculation is now labelled Relaxed AC OPF, and the LMP colour option is named Price.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`19bb95e`](https://github.com/eigenergy/tellegen/commit/19bb95efab383247192cad2d19cf16c816aba9be) Thanks [@samtalki](https://github.com/samtalki)! - The cost box shows only the operational cost, and the bus and line inspectors drop empty rows. Reset demand and Reset rating undo only the selected bus or line.

- [#376](https://github.com/eigenergy/tellegen/pull/376) [`aafc65f`](https://github.com/eigenergy/tellegen/commit/aafc65f4e0c3fbe1d2c10d2a75a92276ae3fc6fb) Thanks [@samtalki](https://github.com/samtalki)! - The engine reads and writes cases with PowerIO 0.12.0, so saved files carry version 0.12.0 and PowerIO 0.11 readers refuse them; files from earlier releases still open. An unrated branch is now saved as an infinite rating, and the multiconductor solvers follow PowerIO 0.12 for inverter power factor signs and type-A regulator taps.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`5c0a03f`](https://github.com/eigenergy/tellegen/commit/5c0a03f0c8c07c4685f2ce94341abe9a5f36d2bf) Thanks [@samtalki](https://github.com/samtalki)! - Each calculation shows its own result boxes, and a long value spans the row instead of being cut off. A case without generator costs shows its max loading instead of a $0 cost.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`a508e37`](https://github.com/eigenergy/tellegen/commit/a508e371985e0c98cedbf25cc7fbda30a8b7d876) Thanks [@samtalki](https://github.com/samtalki)! - Result boxes keep two to a row for long costs, the list of lines at limit has a heading, shows five lines with Show all, and keeps the selected line marked. Price changes too small to matter read 0, buses draw smaller on a phone map, and a file that cannot be opened offers no Retry.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`154dbf0`](https://github.com/eigenergy/tellegen/commit/154dbf0d60edc5e589c73f637ca7e8405a66bb1d) Thanks [@samtalki](https://github.com/samtalki)! - A saved state reads like the live result, with the same Operational cost and Lines at limit boxes. Price, Angle and Voltage are always offered, with the ones the result lacks disabled.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`e78a849`](https://github.com/eigenergy/tellegen/commit/e78a84953ebf53dc80dd440220d176bfe182ca92) Thanks [@samtalki](https://github.com/samtalki)! - A selected bus shows its price, demand and generation, and line colours step from grey to amber at 90% and red at the limit.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`80d6103`](https://github.com/eigenergy/tellegen/commit/80d610324190850d13afe214caf41c9d97318132) Thanks [@samtalki](https://github.com/samtalki)! - Settings use plainer names, such as Starting voltages and DC line model, with one Default entry.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`2b9b41e`](https://github.com/eigenergy/tellegen/commit/2b9b41eb39bcb04e466e60aa9041662e1d5413bd) Thanks [@samtalki](https://github.com/samtalki)! - Notices and progress messages are one short line, such as "DC OPF has no solution" or "12 cases found in 3 files".

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`c843f22`](https://github.com/eigenergy/tellegen/commit/c843f2271e7ac49955cc50e4c05164f969a271f6) Thanks [@samtalki](https://github.com/samtalki)! - On a phone the map credit starts folded to its info button, and Studies buttons share one size.
- Updated dependencies [[`403248b`](https://github.com/eigenergy/tellegen/commit/403248b9a6c87af1adc933f76bfd56f0ae7bd4b1), [`81ed298`](https://github.com/eigenergy/tellegen/commit/81ed2984bb313a90fd9275d2c0be81dec1da72a6), [`32f7c3c`](https://github.com/eigenergy/tellegen/commit/32f7c3c5e9b93d8ef883a2e1db7ce6ba96f5cecb), [`2bcc94d`](https://github.com/eigenergy/tellegen/commit/2bcc94dd25bae08764bd98caeb80e6c1bc97d347), [`c00575c`](https://github.com/eigenergy/tellegen/commit/c00575c55ce9e2987f0065a3cd55a51160c97aa1), [`aafc65f`](https://github.com/eigenergy/tellegen/commit/aafc65f4e0c3fbe1d2c10d2a75a92276ae3fc6fb), [`106acf2`](https://github.com/eigenergy/tellegen/commit/106acf2e3449c1248f9b98e3f08913cae8038ee4), [`a7e6f57`](https://github.com/eigenergy/tellegen/commit/a7e6f57d28df48a39119f51ec1ee031aeceac352), [`82bba76`](https://github.com/eigenergy/tellegen/commit/82bba764f593e2294ea0a30fb780fa0090ddbbe5), [`a508e37`](https://github.com/eigenergy/tellegen/commit/a508e371985e0c98cedbf25cc7fbda30a8b7d876), [`9db9dac`](https://github.com/eigenergy/tellegen/commit/9db9dac92328c91f56045ce26bd40fdac2046916)]:
  - @tellegen/engine@0.10.0
  - @tellegen/webmcp@0.4.0

## 0.9.0

### Minor Changes

- [#184](https://github.com/eigenergy/tellegen/pull/184) [`e92378a`](https://github.com/eigenergy/tellegen/commit/e92378a956e1ad545bd71d51a1c9b638fd53a65c) Thanks [@nickagliano](https://github.com/nickagliano)! - Add a command palette, opened with Cmd+K (Ctrl+K) or the header's Commands button. It runs the calculation, switches cases, opens and closes panels, changes the theme, fits the network to the view, resets the panel layout, and toggles zen mode, which hides the panels until Escape and is saved with the display preferences. Components register their own commands with `getCommandRegistry().register()`, or live lists with `registerSource()`.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`1831a33`](https://github.com/eigenergy/tellegen/commit/1831a337f420171b1b930d46f4e185156da14b0c) Thanks [@samtalki](https://github.com/samtalki)! - Format every multiconductor quantity through one helper, `quantity(value, unit)` in `format.ts`: four significant figures with thousands separators for V, A, kW, kvar and ms, volts switching to kV at 1 kV, two decimals for degrees, one for percent (no space before `%`), four for pu, and `—` for a missing value. `quantityRange`, `voltsWithPu` and `degrees` build on it. The terminal results, overview, results table, multi-wire view, value labels, schematic text and map tooltips no longer round the same current three different ways. `formatPower` now always reports kW and kvar.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`11e1793`](https://github.com/eigenergy/tellegen/commit/11e179371a4d9efb9fb967f0a12fed25cba5affc) Thanks [@samtalki](https://github.com/samtalki)! - Color terminals by phase wherever they are listed: the terminal and equipment results, the overview's pair and conductor bars, and the results table's pair cells now carry the same phase dots as the map strands, inspector chips and multi-wire view. The neutral is one theme-aware gray, `MAP_PALETTE.neutral`, through `phaseColor(terminal, theme)` and the new `phaseCss`. Source, generator and inverter badges take wine, mustard and teal so they no longer share the phase B and C hues. The caution rule beside a qualified result uses a new `--warn` token with a dark-theme value.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`c8b02bb`](https://github.com/eigenergy/tellegen/commit/c8b02bbd1e2d0ad0290d700906e7e1e16f4f4768) Thanks [@samtalki](https://github.com/samtalki)! - Read terminals as phases everywhere a person sees them. `phaseLabel(terminal, neutral?)` maps OpenDSS node numbers 1/2/3/4 and named a/b/c/n to A/B/C/N (and the ground reference to G), passing other names through; `pairLabel`, `pairVoltageLabel` and `wiresLabel` build on it. The legend, inspector chips, terminal results, overview bars and issues, multi-wire view, map tooltip and schematic text use them, and every conductor count reads `3 phases + N`. The bus table's voltage columns are named after the pair their rows share (`V AN`, `V AB`, `V AG`) and filled by phase, so they no longer read as sequence voltages V1/V2/V3. Agent tools and every export, the bus CSV included, keep the source terminal names, so files round-trip with the case data.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`2f79014`](https://github.com/eigenergy/tellegen/commit/2f790148a690039cb491c4c994a666ea489f1389) Thanks [@samtalki](https://github.com/samtalki)! - The terminal results find a bus or a piece of equipment with the same search as the overview, instead of a `<select>` holding every bus (100,000 options on a large feeder) and another holding every piece of equipment. `McResults` takes the case's index as `searchIndex` and builds one from `graph` when it is left out. The two loss figures now have distinct names, "Passive loss" in the results and "Branch losses" in the overview, and the results state the sign convention once: source power positive into the network, equipment P and Q positive into the equipment.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`166959e`](https://github.com/eigenergy/tellegen/commit/166959e394cd0bfffaa1475854934e395c81fa6b) Thanks [@samtalki](https://github.com/samtalki)! - Show voltage angles in degrees in the balanced views too: the power flow inspector labels its angle `Voltage angle (°)` and the diagram's bus tooltip reads `-2.26°` instead of radians, both through the shared formatter. The overview's phase bars show per-unit beside volts (`221.7 V (0.9606 pu)`) when the bus's galvanic zone declares a winding or source rating, with phase–neutral and phase–ground pairs on the phase base; a value far from 1 pu, where the rating is not the pair's base, shows volts alone.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`e74e14b`](https://github.com/eigenergy/tellegen/commit/e74e14bc815b67ce5a97aaa779d4ed160e3fd3fe) Thanks [@samtalki](https://github.com/samtalki)! - Name the calculation one way: "Multiconductor power flow", as the WebMCP tool `solve_multiconductor_pf` does, on the results heading and region, the unavailable notice and the build and case reasons, with the case kind "Distribution case". The interface says "energized", shows one "Calculating…", one unavailable wording and one excluded-island sentence (`EXCLUDED_ISLAND_BUS`). The solve controls shared by the inspector and the study workspace (power flow settings, run or cancel, invalid-option note and "Why unavailable") are one new component, `McSolveControls`, which takes extra row buttons as an `actions` snippet. `MC_PF_NAME`, the phase-label helpers and `quantity` are exported from the package root.

- [#379](https://github.com/eigenergy/tellegen/pull/379) [`98b40ef`](https://github.com/eigenergy/tellegen/commit/98b40ef6fb208b621dc14d4bdc31da10af399ed1) Thanks [@samtalki](https://github.com/samtalki)! - Lead each result with one plain sentence. The Solver panel opens with what the calculation found and how long it took, for example "AC power flow converged in 4 iterations (38 ms).", or what failed and what to try next. The network summary states the OPF objective and how many lines are at their limit. The convergence plot, raw solver status, solver backend and fallback reason move under a "Solver details" disclosure. `statusWords()`, `solveVerdict()` and `outcomeVerdict()` in `diagram/verdict.ts` build these sentences.

  With no `theme` prop, no `data-theme` in the host template and no saved viewer choice, the provider now follows the operating system's `prefers-color-scheme`, and `styles.css` paints the dark tokens from the first frame.

  Notices are full sentences that say what happened and what to do; repeated ones are shared as `NOTICES`. The transmission result panels use the type and spacing tokens, set numbers in tabular mono, and read counts as "Exceptions (3)" rather than "Exceptions · 3".

- [#375](https://github.com/eigenergy/tellegen/pull/375) [`999d23e`](https://github.com/eigenergy/tellegen/commit/999d23ee53a791d3aba930ff6456ab8399d4a876) Thanks [@samtalki](https://github.com/samtalki)! - The WebMCP tools follow the current WebMCP draft and can now open and export cases.

  Every tool declares an `outputSchema`: the `{ ok, data }` or `{ ok, error }` envelope it resolves with, with the fields each tool reports. `schemaErrors` checks a response against it, so a host can test that its adapter matches. The `ToolAnnotations` type gains `consequentialHint` and `debugging`; no tellegen tool is consequential, because every change stays in the browser tab and can be reset.

  Two new tools sit on an optional `files` adapter capability, which `@tellegen/svelte` provides. `export_case` writes the displayed case through PowerIO (MATPOWER, PSS/E, PowerWorld, PSLF, PowerModels, pandapower, or a `.pio.json` module), downloads it for the user, and returns the file name, size and PowerIO's conversion findings. `import_case` opens case text the way a dropped file opens and returns the new case IDs and PowerIO's parse findings.

  Registration takes an `exposedTo` list of origins allowed to call the tools from an embedding page, and an opt-in `debugTool` that registers `inspect_webmcp_registration`, a read-only tool annotated `debugging`. A rejected registration now throws `TellegenRegistrationError`, which names the tool and the DOMException the browser gave (`InvalidStateError` for a duplicate name, `NotAllowedError`, `SecurityError`, `TypeError`), so its message changed. A finished activity event sets `callerCancelled` when the agent cancelled a call that still completed. A `TellegenToolDefinition` now carries `outputSchema`, so code that builds tool definitions by hand must add one.

### Patch Changes

- [#378](https://github.com/eigenergy/tellegen/pull/378) [`b212f1a`](https://github.com/eigenergy/tellegen/commit/b212f1a5fbda1fe1849099b483b4abde45ce4853) Thanks [@samtalki](https://github.com/samtalki)! - Add the `--agent`, `--agent-rgb` and `--agent-soft` theme tokens for agent activity: teal `#0e7490` on the light theme and `#8de7f7` on the dark theme, both above 4.5:1 for text on the panel paper.

- [#386](https://github.com/eigenergy/tellegen/pull/386) [`edf8bae`](https://github.com/eigenergy/tellegen/commit/edf8baeb0c113f5f838aa51fdc7e33d70c42b043) Thanks [@samtalki](https://github.com/samtalki)! - Show distribution case counts with thousands separators, call the dataset link "Source data", and drop the redundant "Distribution case" line.

- [#385](https://github.com/eigenergy/tellegen/pull/385) [`b8b88a7`](https://github.com/eigenergy/tellegen/commit/b8b88a7b82995343a40831be5cb25305c0bd259c) Thanks [@samtalki](https://github.com/samtalki)! - Show prices in $/MWh and costs in $/h instead of "objective units", and write the Agent panel's labels in sentence case without error codes.

- [#380](https://github.com/eigenergy/tellegen/pull/380) [`bb4ee4c`](https://github.com/eigenergy/tellegen/commit/bb4ee4cc59b3f5679debb6e18384362b025e9ab5) Thanks [@samtalki](https://github.com/samtalki)! - Set the header's organization line, case-tab sublabels and zen exit button in sentence case with the display face, instead of all-caps monospace. The default organization label is now "Eigenergy Group @ Michigan ECE".

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`80d0f2a`](https://github.com/eigenergy/tellegen/commit/80d0f2ad87f8b3ed843b5a01cf314060e6896c3b) Thanks [@samtalki](https://github.com/samtalki)! - The bus CSV's Pairs column keeps the source terminal names (`1–n 2–n 3–n`) so an export round-trips with the case data; the results table shows the phase labels (`A–N`) from a separate `pairLabels` row field that is not exported.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`6ccc56a`](https://github.com/eigenergy/tellegen/commit/6ccc56ae31cf3258672d7cc7b3c188fa1e97fca0) Thanks [@samtalki](https://github.com/samtalki)! - The header chip for an opened distribution case reads "Distribution", like the hosted distribution chips and the inspector's "Distribution case", instead of "Multiconductor". The chip carries `data-testid="distribution-case-chip"`.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`abc678c`](https://github.com/eigenergy/tellegen/commit/abc678cec273a37d8cae413962719afe10334535) Thanks [@samtalki](https://github.com/samtalki)! - Find a bus's neutral one way everywhere. The schematic's conductor marks (`conductorSet`), neutral earths and attachment ticks, and the overview's conductor bars (`conductorBars`) now call `neutralTerminal`: a declared neutral wins, a declared absence means none, and only a bus silent about it falls back to the aliases n, 4 and neutral. Before, the schematic honoured declared neutrals only and the bars assumed `n`.

- [#383](https://github.com/eigenergy/tellegen/pull/383) [`b0925a7`](https://github.com/eigenergy/tellegen/commit/b0925a76b78bd2baaf13fa1ad72fe7b21ae862a2) Thanks [@samtalki](https://github.com/samtalki)! - State an OPF result as its generation cost in $/h ("Generation cost is $1,688,055/h. One line is at its limit.") instead of a bare objective, and drop the coloured bar beside result sentences and notices.

- [#381](https://github.com/eigenergy/tellegen/pull/381) [`41a6d1a`](https://github.com/eigenergy/tellegen/commit/41a6d1adc0d3ca1343f4cee46387f1d1fba98c83) Thanks [@samtalki](https://github.com/samtalki)! - A saved study's bus detail shows its voltage angle in degrees and its voltage through the shared formatter, like the live power flow inspector.

- [#378](https://github.com/eigenergy/tellegen/pull/378) [`c8554f1`](https://github.com/eigenergy/tellegen/commit/c8554f192d1f0c94662aec48118971ae4ee134b2) Thanks [@samtalki](https://github.com/samtalki)! - WebMCP tool errors are now full sentences that say what happened and what to call next. `focus_network`, `preview_case_update`, `update_case` and `analyze_sensitivity` fly the map to the bus or branch they work on; only the camera moves, so the case revision an agent holds stays valid.
- Updated dependencies [[`698916a`](https://github.com/eigenergy/tellegen/commit/698916a7d2dfd0021c0853718ab927e2a434940d), [`bfb463b`](https://github.com/eigenergy/tellegen/commit/bfb463b60a089048a51dcfeb80034ef18c28458e), [`999d23e`](https://github.com/eigenergy/tellegen/commit/999d23ee53a791d3aba930ff6456ab8399d4a876)]:
  - @tellegen/engine@0.9.1
  - @tellegen/webmcp@0.3.0

## 0.8.0

### Minor Changes

- [#358](https://github.com/eigenergy/tellegen/pull/358) [`caffe32`](https://github.com/eigenergy/tellegen/commit/caffe32a4eb40d91253ff9b7a5f6ac20e6920c1c) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Add equipment inventories, automatic regulator studies, detailed DC-grid power flow, coupled AC/DC studies and joint nonlinear AC/DC dispatch with explicit model limits and operating diagnostics.

- [#300](https://github.com/eigenergy/tellegen/pull/300) [`bd39062`](https://github.com/eigenergy/tellegen/commit/bd3906247d1e322b2a0cde813de438d5b325204a) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Add interactive transmission diagrams with geographic and substation layouts, voltage-level keys, operating-limit and economic views, equipment inspection, accessible mobile controls and PNG export.

### Patch Changes

- Updated dependencies [[`caffe32`](https://github.com/eigenergy/tellegen/commit/caffe32a4eb40d91253ff9b7a5f6ac20e6920c1c), [`78e2768`](https://github.com/eigenergy/tellegen/commit/78e27688f0ce715488e738a46dc438d1273ee0f6), [`bd39062`](https://github.com/eigenergy/tellegen/commit/bd3906247d1e322b2a0cde813de438d5b325204a)]:
  - @tellegen/engine@0.9.0

## 0.7.0

### Minor Changes

- [#315](https://github.com/eigenergy/tellegen/pull/315) [`b3ad0bd`](https://github.com/eigenergy/tellegen/commit/b3ad0bd210d1b0d7e12bbc1d23ef187c140b8e64) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Discover and select independent cases from ZIP uploads and folders when the engine includes collection support. Hide unavailable controls in the ordinary PowerIO 0.11 build. Preserve successful imports and saved calculation settings, wait for an explicit calculation, and support cancellation, explicit document groups and source downloads.

- [#346](https://github.com/eigenergy/tellegen/pull/346) [`ab5f9a3`](https://github.com/eigenergy/tellegen/commit/ab5f9a3fb0f8c7c2c4107616dc0603fdce373537) Thanks [@samtalki](https://github.com/samtalki)! - Export embeddable calculation controls shared with the Settings panel. A run validates draft options and updates the accepted Study, map and exports together; failed or cancelled calculations keep the prior result. Hosts can load the optional AC OPF backend on demand through one cancellable controller probe.

- [#349](https://github.com/eigenergy/tellegen/pull/349) [`a2e7786`](https://github.com/eigenergy/tellegen/commit/a2e7786d49707d2c576f6a88bf7803ea81c8be6b) Thanks [@samtalki](https://github.com/samtalki)! - Expose the controller WebMCP adapter through the optional `@tellegen/svelte/webmcp` entry point. Hosts share case inspection, bounded queries, settings, revision checks, and serialized updates; planning approval and Study evidence remain explicit host-provided capabilities. Normal Svelte imports do not require the WebMCP peer.

- [#317](https://github.com/eigenergy/tellegen/pull/317) [`c62e577`](https://github.com/eigenergy/tellegen/commit/c62e577cad5048e2588cc2157d0e36f28f258069) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Add qualified OpenDSS snapshot support for PowerIO 0.12 candidate builds; the ordinary PowerIO 0.11.9 build keeps raw OpenDSS imports view-only. Preserve explicit bounded load laws and transformer excitation, retain passive source definitions, and report unsupported controls, floating sources and transformer profiles before solving.

### Patch Changes

- [#351](https://github.com/eigenergy/tellegen/pull/351) [`eee66d9`](https://github.com/eigenergy/tellegen/commit/eee66d9e268b34b2d4aa17aade20f1b83459ad65) Thanks [@samtalki](https://github.com/samtalki)! - Fit newly opened multiconductor schematics when the map is too far away or the feeder is hidden behind panels. Preserve a useful close map view, and center small drawings using the capped zoom.

- [#315](https://github.com/eigenergy/tellegen/pull/315) [`90e1add`](https://github.com/eigenergy/tellegen/commit/90e1addf7525871c37a94bb12db9254e65e96c4d) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Keep imported collection rows selectable for source ZIP export and discard stale export failures after cancellation.
- Updated dependencies [[`9e5e0b5`](https://github.com/eigenergy/tellegen/commit/9e5e0b5f07e4438ebd393707e71dbedb2dd2f138), [`c62e577`](https://github.com/eigenergy/tellegen/commit/c62e577cad5048e2588cc2157d0e36f28f258069), [`721f515`](https://github.com/eigenergy/tellegen/commit/721f51522e6c89f5b0b87ee16f9b6ca3dd2eb243)]:
  - @tellegen/engine@0.8.0

## 0.6.0

### Minor Changes

- [#341](https://github.com/eigenergy/tellegen/pull/341) [`4ed2a0b`](https://github.com/eigenergy/tellegen/commit/4ed2a0b2282551535bcc5dee4d1b9cf992ffb351) Thanks [@samtalki](https://github.com/samtalki)! - Add a common Settings panel and a Rust-owned calculation settings contract across TypeScript, WebMCP, native, CLI and HTTP interfaces. Applying settings stages the next explicit solve without replacing the accepted result. Retained balanced Studies keep their effective options and active constraint selections. Saved PowerIO modules record the calculation and only the options that differ from the engine defaults; reopening a file written by another engine version drops options this engine does not know and logs a warning instead of refusing the file. The generated settings types name calculations with the `Calculation` union. Browser display preferences persist separately, and API mutations check case and settings revisions. `tellegen solve-mc` now resolves its options through the same contract, so an unknown option key is an error rather than being silently ignored.

- [#341](https://github.com/eigenergy/tellegen/pull/341) [`425fb53`](https://github.com/eigenergy/tellegen/commit/425fb5345735eded7ef179bb1dbe45482591a5b5) Thanks [@samtalki](https://github.com/samtalki)! - Recognize multiconductor entries in the hosted case catalogue and load their
  portable PowerIO modules into the distribution viewer and browser AC power-flow
  workflow, with lazy loading, browser-local removal, and restore support.

  Hosted cases can supply source/association metadata and PF defaults. Preserve
  those defaults across UI/WebMCP solves and saved-study reopen; compact
  multiconductor exports keep large studies within the replay input limit.
  The public demo lists the Texas7k distribution case once an operator stages
  its bundle; deploys do not require it.

  Cancel hosted requests on controller disposal, discard late ingest completions,
  and report missing backends without starting a request.

  Keep hosted inputs parsed in the engine and initialize the shared solver settings
  from catalogue defaults, including saved-study replay. Free retained inputs when
  hosted requests are superseded or the catalogue is reloaded.

### Patch Changes

- Updated dependencies [[`4ed2a0b`](https://github.com/eigenergy/tellegen/commit/4ed2a0b2282551535bcc5dee4d1b9cf992ffb351), [`425fb53`](https://github.com/eigenergy/tellegen/commit/425fb5345735eded7ef179bb1dbe45482591a5b5)]:
  - @tellegen/engine@0.7.0

## 0.5.2

### Patch Changes

- [#331](https://github.com/eigenergy/tellegen/pull/331) [`5351e33`](https://github.com/eigenergy/tellegen/commit/5351e33af8924e5af11e3f2a1ef7840aa1cdaf7b) Thanks [@samtalki](https://github.com/samtalki)! - At low zoom the map draws one bus marker per crowded spot, so a state-sized grid no longer covers the map in overlapping markers. In each spot it keeps a generator before a substation before any other bus, as recorded in a grid made by tellegen-synth, such as one from MapToGrid, and then the bus with the most generation and demand. Zooming in shows the rest, and panning leaves the markers as they are. A selected bus always shows, with its selection ring, and every bus is still a bus of the case: the bus picker finds it by number and a line's tooltip names it. `TellegenMap` takes `declutterMarkers={false}` to draw every marker at every zoom, as before. `LocalCase` gains `busKinds`, each bus's kind in a generated grid.
- Updated dependencies [[`6972d33`](https://github.com/eigenergy/tellegen/commit/6972d33b6480d57502cd702e393223ec8e41e2b3)]:
  - @tellegen/engine@0.6.0

## 0.5.1

### Patch Changes

- [#313](https://github.com/eigenergy/tellegen/pull/313) [`803657c`](https://github.com/eigenergy/tellegen/commit/803657cec1ee1490fecd050aac17b94c2e341594) Thanks [@samtalki](https://github.com/samtalki)! - A source the host added to `TellegenMap`'s map counted as part of the basemap: if its tiles answered 401, 403 or 429, or kept failing to load, the map switched to the fallback basemap although the primary was fine, and the switch removed the host's own sources and layers. Now only the sources of the basemap style itself can trigger the switch, and a host source's errors are logged to the console. A real switch still replaces the whole style, so a host that adds sources and layers in `onMapReady` should add them on every `style.load`; the README shows how. `BasemapMapLike` gains an optional `getStyle`, which `attachBasemapFallback` reads to tell the basemap's sources from the host's.

- [#316](https://github.com/eigenergy/tellegen/pull/316) [`827a71f`](https://github.com/eigenergy/tellegen/commit/827a71f0f86dcd0530ae2ba1325d62e27942298f) Thanks [@samtalki](https://github.com/samtalki)! - The map no longer draws a marker for a line tap in a grid made by tellegen-synth, such as one from MapToGrid. A tap is a point where a line is tapped, with no load or machine, and its line already shows it on the map. On a state-sized grid every tap used to get its own marker, so the markers overlapped and covered the network. Taps are read from the `kind` that tellegen-synth records on each bus row of the module, and other cases draw every bus as before. A tap is still a bus of the case: the bus picker finds it by number, a line's tooltip names it, and a selected tap gets its marker and the selection ring back. `LocalCase` gains `tapBuses`, the tap bus ids, and `markerBuses`, the buses the map draws as markers.

- [#272](https://github.com/eigenergy/tellegen/pull/272) [`b02955a`](https://github.com/eigenergy/tellegen/commit/b02955aeec68aeef333c61910fb10483e81220f6) Thanks [@BurhanAbdullah](https://github.com/BurhanAbdullah)! - A branch route that crosses ±180 draws as one short path on the flat map and on the globe, instead of a line across the whole map. `TellegenMap` takes the short hop between consecutive route vertices in its route layers, and the selected-branch outline, the selected-bus marker and the framing of a selected branch follow a network across the seam. Stored and exported coordinates are unchanged: route longitudes stay in [-180, 180], so a host drawing routes in its own layers (`extraLayers`) takes the short hop itself.
- Updated dependencies [[`7c8919e`](https://github.com/eigenergy/tellegen/commit/7c8919eb809868fdbeab6e6cb7513990c106087d), [`0b34030`](https://github.com/eigenergy/tellegen/commit/0b340305d141e62d16b64b195adbe857c2467d81), [`b6461f3`](https://github.com/eigenergy/tellegen/commit/b6461f3ed81dc8672fb39582583a211b02cbf453), [`a20d241`](https://github.com/eigenergy/tellegen/commit/a20d24165ed2c6e7260cbdfe075ca77588e92fb0)]:
  - @tellegen/engine@0.5.1

## 0.5.0

### Minor Changes

- [#268](https://github.com/eigenergy/tellegen/pull/268) [`7f1852c`](https://github.com/eigenergy/tellegen/commit/7f1852cc9de537c3abf72a44b916c975c37f6114) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Detect source-less islands on import and show one context-sensitive solve button
  in Network and Studies, with exclusions visible before solving and shared solver
  settings. Scroll Studies to the operating-point overview after successful explicit
  solves. Add explicit energized-network preparation for multiconductor power flow. Retain
  the full input for inspection, identify excluded source-less islands and unsupplied
  loads, and preserve and validate network scope in saved studies, solution modules,
  and CSV exports. Use default voltage-change tolerance 1e-4 V, absolute KCL tolerance 1e-4 A,
  and relative KCL tolerance 1e-5; other solver settings remain unchanged.

  Prepare energized networks directly from retained inputs without an IR text round trip.
  Columnar operating-point results retain load-model qualification and excluded-island provenance.

- [#268](https://github.com/eigenergy/tellegen/pull/268) [`7f1852c`](https://github.com/eigenergy/tellegen/commit/7f1852cc9de537c3abf72a44b916c975c37f6114) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Expose multiconductor power flow settings and distinguish convergence from original load-model fidelity. Results and CSV exports retain qualification and effective settings. Simulation snapshots use version 2, with no canonical PowerIO solution for modified or unverified load equations; version 1 snapshots are reassessed on replay. Load-envelope defaults are unchanged and failed reruns retain the previous result.

- [#286](https://github.com/eigenergy/tellegen/pull/286) [`888859b`](https://github.com/eigenergy/tellegen/commit/888859b6ab4b11d63dabaadd6389e56d3227cd73) Thanks [@samtalki](https://github.com/samtalki)! - A multiconductor session can hand over its complete result as columns. `BrowserMcPfSession.resultTables()` returns `McPfResultTables`: the terminals, equipment ports, and source reactions of `result()`, row for row, with each name stored once and every value in a typed array. The worker transfers the arrays rather than copying them, and `mcPfResultTables(result)` gives a stored `McPfResult` the same form.

  The overview and the diagram now read a live session's operating point this way, so analysing a whole-city model no longer parses a 166 MiB JSON result into an object per terminal and port. `MulticonductorCase.overviewResult` holds either form.

- [#284](https://github.com/eigenergy/tellegen/pull/284) [`ef7e448`](https://github.com/eigenergy/tellegen/commit/ef7e448abf5959b489ac94b728283ea50bd67707) Thanks [@samtalki](https://github.com/samtalki)! - A multiconductor case can stay parsed in the engine instead of crossing the worker boundary as PowerIO IR text. `ingestJsonDrop` and `ingestDistCaseBytes` take `{ retain: true }` and then return the parsed module as `payload.input`, a `BrowserMcInput`, in place of `module_json`. `createMcPfSession` accepts that input, `BrowserMcPfSession.input()` returns the edited input in the same form, and `BrowserMcInput.moduleJson()` writes the IR text only when it is asked for. Without `retain`, both ingest calls return `module_json` as before.

  The Svelte controller imports multiconductor cases this way, so AC power flow on a whole-city model no longer fails with "input exceeds 128 MiB limit". For a 57,757-bus model, the pretty-printed IR was 187.6 MB. `MulticonductorCase.moduleJson` is now `MulticonductorCase.mcInput`, which holds either a `BrowserMcInput` or IR text.

### Patch Changes

- [#283](https://github.com/eigenergy/tellegen/pull/283) [`6e9b9d4`](https://github.com/eigenergy/tellegen/commit/6e9b9d4a2a7538f46f2a74e4d2dd087fe65ab4a4) Thanks [@samtalki](https://github.com/samtalki)! - Match a multiconductor case's routed lines to its edges through an index built once, instead of scanning every edge for every route. On a 57,757-bus city case the scan took over a minute of the load. Which route lands on which edge does not change.
- Updated dependencies [[`7f1852c`](https://github.com/eigenergy/tellegen/commit/7f1852cc9de537c3abf72a44b916c975c37f6114), [`7f1852c`](https://github.com/eigenergy/tellegen/commit/7f1852cc9de537c3abf72a44b916c975c37f6114), [`888859b`](https://github.com/eigenergy/tellegen/commit/888859b6ab4b11d63dabaadd6389e56d3227cd73), [`ef7e448`](https://github.com/eigenergy/tellegen/commit/ef7e448abf5959b489ac94b728283ea50bd67707), [`01184dd`](https://github.com/eigenergy/tellegen/commit/01184dd88035cfeb8c0186e8075685a3b6b6ec55), [`3aa86e6`](https://github.com/eigenergy/tellegen/commit/3aa86e6ca4c79d485cf452c6806065644ec5f7ce)]:
  - @tellegen/engine@0.5.0

## 0.4.7

### Patch Changes

- [#280](https://github.com/eigenergy/tellegen/pull/280) [`962eaec`](https://github.com/eigenergy/tellegen/commit/962eaec2d51a8487758c878cf2bdf0d274dcfdc5) Thanks [@samtalki](https://github.com/samtalki)! - Hide the far side of the globe by culling past the horizon in the shader, and draw the overlay on top again. 0.4.6's depth test against maplibre's globe also cut away parts of a network facing the camera. `TellegenMap` adds the horizon extension to its own layers and to `extraLayers`.

## 0.4.6

### Patch Changes

- [#277](https://github.com/eigenergy/tellegen/pull/277) [`fa48812`](https://github.com/eigenergy/tellegen/commit/fa48812d3a8c883e779108cffd132f57008febe7) Thanks [@samtalki](https://github.com/samtalki)! - On the globe, a network on the far hemisphere no longer draws through the earth. `TellegenMap`'s overlay depth-tests against the globe in globe projection and still draws on top of the flat map.

## 0.4.5

### Patch Changes

- [#273](https://github.com/eigenergy/tellegen/pull/273) [`5e9b3a8`](https://github.com/eigenergy/tellegen/commit/5e9b3a8a5b1ef616091366e53ce7059d97162d79) Thanks [@samtalki](https://github.com/samtalki)! - Add a structured log of errors and engine findings. `app.log` records what PowerIO reported while reading a case or a geographic layer, what each solve reported, what each export's writer reported, and every `app.error`, with severity, step, source, code, file, element and buses. `DiagnosticLogView` shows it grouped by step and file with filters, Copy and Download; the default shell adds a "Log" panel, and case import notes now list their findings as rows. `Solution` carries the solve's `diagnostics` (islands, HVDC transfers), which the browser Study already returned.
- Updated dependencies [[`5e9b3a8`](https://github.com/eigenergy/tellegen/commit/5e9b3a8a5b1ef616091366e53ce7059d97162d79)]:
  - @tellegen/engine@0.4.6

## 0.4.4

### Patch Changes

- [#194](https://github.com/eigenergy/tellegen/pull/194) [`65dd440`](https://github.com/eigenergy/tellegen/commit/65dd4403665c2682850175ef75f3dcbcfa86e4b2) Thanks [@samtalki](https://github.com/samtalki)! - Add an opt-in, capability-gated browser AC OPF client backed by a separate
  WASI worker, with hard worker cancellation and no main-thread fallback.

- [#228](https://github.com/eigenergy/tellegen/pull/228) [`20c0f73`](https://github.com/eigenergy/tellegen/commit/20c0f7348482106f286818ee3ecf4c0d50b2e17d) Thanks [@samtalki](https://github.com/samtalki)! - Multiconductor graphs carry declared voltage limits and ratings, and the Studies
  view gains an operating-point overview.

  In `@tellegen/engine`, `DistGraphBus.voltage_limits` holds the model's declared
  phase–neutral and phase–phase voltage bounds in volts, and `DistGraphEdge.rating`
  holds per-conductor ampacity (`i_max`) or, for a transformer, its largest winding
  rating at each bus it connects (`s_rating_by_bus`), the same on every winding-pair
  edge. Both are present only when the model declares them.

  In `@tellegen/svelte`, `McWorkbench` summarises a multiconductor power-flow result:
  violations, losses, voltage ranges per declared reference, maximum loading and
  maximum unbalance; a ranked issue list that selects the bus or branch; search
  over buses and branches; per-phase voltage bars against declared limits and
  per-conductor currents at both ends; sortable bus and branch tables; and CSV export. Cases
  larger than 25,000 buses compute the overview on request. Setting
  `MulticonductorCase.schematic` makes `TellegenMap` show the case as a deck.gl
  schematic or at its source positions, with an animated switch between them,
  result colouring, the shared selection and PNG/SVG export. `overviewResult` holds
  the operating point the overview and schematic share. The schematic layout runs in
  the engine: `mcSchematicLayout` in `@tellegen/engine` returns tidy or leaf-order
  geometry and the display forest, reserving room below each bus for its attached
  equipment. The schematic draws transformer, switch (open or closed) and attachment
  symbols (load, generator, inverter-based resource, source, shunt) with a legend.
  Lines show their conductors at close zoom; buses are named with short ids; parts not
  energised from any source are drawn faint; and outside selections fly into view.
  The schematic can be walked bus by bus from the keyboard, with a spoken description.
  Solved P/Q and power-flow arrows show at close zoom, hover is linked across the
  schematic and tables, and each LV network (galvanic zone) is outlined (on geographic
  positions, as a corridor along its lines) and tinted by its worst state, with a summary tag. MV lines draw heavier than LV, and LV
  detail fades when zoomed out. Sources, generators, large IBRs and transformers stay
  visible as fixed-size landmarks, placed by priority. Geographic positions draw over the main
  map's basemap, which fades in and out with the layout switch. The diagram opens on the case's own
  positions and switches to the schematic from there. Branches drawn along the same route fan
  apart on hover, with a card to pick between them, and buses at the same point spread
  apart the same way. The schematic can be downloaded as a PowerIO
  diagram layer, follows a case's own routes in source positions, and a selected
  branch gets a multi-wire view of its conductors. Unbalance is the voltage
  unbalance factor from terminal phasors, shown beside the magnitude spread.
  `controller.multiFullResult`
  reads the full operating point from a stored result or the live session, and
  `TellegenShell` keeps `overviewResult` current for the active case
  (`controller.followOverview`, `readOverview` and `overviewOnRequest`). A
  multi-winding transformer counts once in losses and issues, and its loading is
  judged per connected bus. Graph buses carry `nominal_voltage`, the declared rating
  of a winding or source there; the diagram colours lines by voltage level, and a
  hover shows the feeder and network of what it is over, with an optional path back
  to the source. Graph buses also carry `attachments` (each attachment's terminals in
  its own order and its connection) and transformer edges `windings` (bus, wye or
  delta, terminals, rating). The diagram draws each transformer once with its winding
  connections or centre tap, and each load or shunt with its conductors and, closest
  in, its terminals. The
  renderer-independent logic lives in `src/lib/diagram`, ported from the diagram
  projection draft ([#177](https://github.com/eigenergy/tellegen/issues/177)).

- [#229](https://github.com/eigenergy/tellegen/pull/229) [`feb9cb0`](https://github.com/eigenergy/tellegen/commit/feb9cb0a9f991479ab5888a0ac83e3548268fc33) Thanks [@samtalki](https://github.com/samtalki)! - The multiconductor overview ranks phase–ground voltages against declared
  phase-to-ground bounds, alongside the phase–neutral and phase–phase bands. Each
  declared reference gets its own pairs (`a–ground` and so on) in the issues, the
  voltage ranges, the phase bars, value labels, tables and CSV export; `BusLimits`
  holds one entry per reference. Non-uniform phase–phase bounds are ranked per pair,
  in the pair order the engine names. A case that declares no voltage limits or ratings
  says so plainly, and one whose limits the overview does not rank says that instead.

  The overview and the schematic share one result snapshot. `MulticonductorCase`
  derives `overviewSnapshot` from `overviewResult` once per operating point, and
  `overviewInputs` and `overviewSearch` once per graph. `McWorkbench` takes
  `snapshot`, `inputs` and `search` in place of `result`.

- [#194](https://github.com/eigenergy/tellegen/pull/194) [`c721344`](https://github.com/eigenergy/tellegen/commit/c7213444e5c6e9a904a9db73e3195c755b05d875) Thanks [@samtalki](https://github.com/samtalki)! - Expose AC OPF recovery diagnostics from the POUNCE 0.13.0 worker and show the
  total iteration count, including retries, in the solve card. Mark recovered
  solves while preserving support for responses without recovery diagnostics.

- [#225](https://github.com/eigenergy/tellegen/pull/225) [`12b79b5`](https://github.com/eigenergy/tellegen/commit/12b79b56a8e509aae12dbbbfa1c0375c2745b008) Thanks [@samtalki](https://github.com/samtalki)! - The multiconductor map hands its transformer symbol to deck.gl as a prepacked
  icon atlas, so nothing is fetched. It was a `data:` URL that deck.gl fetched,
  and a Content Security Policy whose `connect-src` leaves out `data:` blocked it,
  so the map drew without symbols.
- Updated dependencies [[`1fbc8f1`](https://github.com/eigenergy/tellegen/commit/1fbc8f1ee9f432aa9ee5bb390281b67af16f21ed), [`65dd440`](https://github.com/eigenergy/tellegen/commit/65dd4403665c2682850175ef75f3dcbcfa86e4b2), [`a54557f`](https://github.com/eigenergy/tellegen/commit/a54557f23d755ec77814f74e0dcca5d09ff11a82), [`d768cb2`](https://github.com/eigenergy/tellegen/commit/d768cb2a96f0fbd70b93169b9f7b50af3176e332), [`5bd83ed`](https://github.com/eigenergy/tellegen/commit/5bd83ed4ccd79e36290aef146702b3ca398b8d18), [`20c0f73`](https://github.com/eigenergy/tellegen/commit/20c0f7348482106f286818ee3ecf4c0d50b2e17d), [`21b8892`](https://github.com/eigenergy/tellegen/commit/21b8892712aa5995e78d2bb5ca01b362016e8c9b), [`feb9cb0`](https://github.com/eigenergy/tellegen/commit/feb9cb0a9f991479ab5888a0ac83e3548268fc33), [`c721344`](https://github.com/eigenergy/tellegen/commit/c7213444e5c6e9a904a9db73e3195c755b05d875)]:
  - @tellegen/engine@0.4.3

## 0.4.3

### Patch Changes

- [#219](https://github.com/eigenergy/tellegen/pull/219) [`5ebcad8`](https://github.com/eigenergy/tellegen/commit/5ebcad80c08495ec01d80dc06a885a2817585e40) Thanks [@samtalki](https://github.com/samtalki)! - The demand slider's Nearby and Full range buttons are named by their visible
  text again. 0.4.2 labelled them "Nearby demand range" and "Full demand range",
  which do not contain the visible "Full range", so speech input could not reach
  the button by what it shows (WCAG 2.5.3, axe `label-content-name-mismatch`).
  The "Demand range" group still names the pair.

## 0.4.2

### Patch Changes

- [#211](https://github.com/eigenergy/tellegen/pull/211) [`1f30672`](https://github.com/eigenergy/tellegen/commit/1f30672b5f7b05e0314d05b1248c0bf8bb4fa291) Thanks [@samtalki](https://github.com/samtalki)! - Components show data, labels and controls, and leave notes to the host. The
  "parsed in your browser" and "never uploaded" lines are gone from
  `LocalCaseDetails`, `DropZone` and `AppHeader`, and coordinate provenance is a
  labelled value (Coordinates: Generated, Synthetic layout, Manual layout, From
  file or From the geographic file's name) beside a File row. The solver card lists
  the calculation, iterations and time as labelled values, and the compute-off
  notices no longer carry a contact address.

  A host adds its own note through two optional snippets: `ControlPanel`'s
  `caseNote`, rendered under a local case's stats with the case as its argument,
  and `DropZone`'s children, rendered under the drop prompt.

  Visible text is in sentence case, and the monospace face is kept for numbers
  and identifiers. The case actions read Save case, Save solution, Export as and
  Save geography (Save drawing in diagram view). Hosts that match on the old
  strings need updating: the solver card no longer shows "OPF solve", the bus
  lookup's accessible name is "Bus lookup", and the full demand range button is
  named "Full demand range".

- [#214](https://github.com/eigenergy/tellegen/pull/214) [`4ea6b1c`](https://github.com/eigenergy/tellegen/commit/4ea6b1c7126b348bc07b6ac66d211b6d261bcde8) Thanks [@samtalki](https://github.com/samtalki)! - `TellegenProvider` disposes its controller when it unmounts. The new
  `Controller.dispose()` frees every case's browser Study, including one still
  being built, and every multiconductor session, cancels their pending edits and
  in-flight requests, and builds nothing afterwards, so a provider's Studies and
  sessions no longer stay in the engine worker.

  `theme` and `cartoApiKey` on `TellegenProvider` and `TellegenViewer` follow
  their props: switching either updates the root `data-theme` attribute and
  rebuilds the default shell's map in place. The other props are still read once,
  at mount.

  `TellegenMap` assigns its teardown before it calls `onMapReady`, so a host
  callback that throws no longer keeps the MapLibre map and its window and
  document listeners alive.

- [#215](https://github.com/eigenergy/tellegen/pull/215) [`2abf68f`](https://github.com/eigenergy/tellegen/commit/2abf68f1d659f77344934e4200719d0c543194f5) Thanks [@samtalki](https://github.com/samtalki)! - `maplibre-gl`, `@deck.gl/core`, `@deck.gl/layers` and `@deck.gl/maplibre` are
  peer dependencies instead of dependencies, with the same ranges. The map shares
  their instances with the host through `extraLayers` and `onMapReady`, so the
  host installs them and one copy of each resolves. npm 7 and later and pnpm 8
  and later install missing peers on their own; with Yarn or
  `--legacy-peer-deps`, add them to the app's dependencies.
- Updated dependencies [[`2abf68f`](https://github.com/eigenergy/tellegen/commit/2abf68f1d659f77344934e4200719d0c543194f5), [`0cc5d77`](https://github.com/eigenergy/tellegen/commit/0cc5d77bfb79760fd326943aacafd0c89db14640)]:
  - @tellegen/engine@0.4.1

## 0.4.1

### Patch Changes

- [#209](https://github.com/eigenergy/tellegen/pull/209) [`107aa8c`](https://github.com/eigenergy/tellegen/commit/107aa8c9e68e7b19531f33ef18029a7cc7e081de) Thanks [@samtalki](https://github.com/samtalki)! - Show bus demand as colour. A `demand` display mode shades buses by their
  demand in MW, committed changes included, on a square-root scale with a cool
  viridis ramp that cannot be mistaken for the nodal value ramp.
  `AppState.busSizing` chooses how markers are sized: `power` (the default)
  scales them by the larger of demand and generation as before, and `uniform`
  draws every bus alike. Uniform markers on the map keep one ground radius
  between 2 and 4.5 pixels, so a whole country reads as a mesh with its lines
  showing and a closer view as distinct buses. Changing the calculation keeps a
  demand colouring, and the bus tooltip does not repeat demand under it.
  `@tellegen/svelte/map` exports `busMarkerRadius`, `busRadiusFor`,
  `demandColor`, `demandDomain`, `demandGradient`, `UNIFORM_BUS_RADIUS`,
  `UNIFORM_BUS_MIN_RADIUS` and `UNIFORM_BUS_GROUND_RADIUS`.

## 0.4.0

### Minor Changes

- [#154](https://github.com/eigenergy/tellegen/pull/154) [`a3fdbd3`](https://github.com/eigenergy/tellegen/commit/a3fdbd3923b964654582bb384b57bd0b4ab90887) Thanks [@samtalki](https://github.com/samtalki)! - Multiconductor power-flow sessions answer a load edit with a constant-size
  summary instead of the complete terminal and equipment result, in one worker
  round trip. `BrowserMcPfSession.replaceLoadPowers` now resolves to
  `McPfSummary`; session creation returns the initial summary; detail is
  fetched on demand with `detail()` (one bus and a page of equipment ports) and
  `terminalVoltages()`/`terminalCurrents()` (transferred `Float64Array`s), and
  `result()` still returns the full result for export. Sessions report the
  engine, transfer, and parse time of each edit and the engine's live and peak
  heap. In `@tellegen/svelte`, `MulticonductorCase.result` is now the summary,
  `mcFullResult` holds a stored full result when no session does, and
  `McResults` accepts either with an optional `detail` loader.

- [#202](https://github.com/eigenergy/tellegen/pull/202) [`41471e1`](https://github.com/eigenergy/tellegen/commit/41471e191c520531b1392ab4e55fe19f9e87520b) Thanks [@samtalki](https://github.com/samtalki)! - Accessibility fixes in the panels:

  - Text in the Network panel, the solver card, the size legend and the residual sparkline is at least 12 px; sizes from 10 to 11.5 px grow by 2 px, so the hierarchy holds.
  - The dark `--ink-faint` is `#95a6b6`, 7.14:1 against the panel composited over `#4a4a4a`.
  - The dark `--border-strong`, which marks a panel's edge over the map, is `#859cb4`. It measures 6.30:1 against the panel and 3.13:1 against bare `#4a4a4a`; the old `#6f8aa6` measured only 2.47:1 against the grey.
  - The panel close button has a 44 px target under coarse pointers.
  - When a panel closes, focus moves to its toolbar button. Without one, it returns to the control that opened the panel, else to the panel's dock (a region named "Left panels" or "Right panels"), or to the drawer on narrow screens.
  - Panel content that overflows becomes a focusable, labelled region.
  - The demand range toggle is a labelled group, and the "full range" button's accessible name now contains its visible label.
  - Arrow keys on the demand and rating sliders step about 1% of the range, and never less than 0.5 MW, so every press leaves the 0.25 MW dead zone.
  - An infeasible DC OPF reports "No feasible dispatch: demand cannot be served within line and generator limits". The solver text moves to the new `app.errorDetail` and into the notice's disclosure.
  - The `neutral` entry of `MAP_PALETTE` (new in this release) fills a neutral terminal in the multiconductor detail, whose outline now follows `--ink`.

- [#196](https://github.com/eigenergy/tellegen/pull/196) [`02c308e`](https://github.com/eigenergy/tellegen/commit/02c308e4504ffbba9e296c55670331189595bfa0) Thanks [@samtalki](https://github.com/samtalki)! - `TellegenMap` takes its basemap from props instead of a hardcoded CARTO URL,
  and the default order depends on a CARTO basemap key. CARTO's terms ask for
  one; keys are free for non-commercial use from carto.com/basemaps/apikey.

  - With `cartoApiKey`, the keyed CARTO style leads (positron-nolabels or
    dark-matter-nolabels), and OpenFreeMap is the fallback.
  - Without a key, OpenFreeMap leads, and the keyless CARTO style is the
    fallback. The key is trimmed, so a blank one counts as none.

  Whenever an OpenFreeMap style renders, road lines start at zoom 8. In the dark
  theme, water and land are recoloured towards CARTO Dark Matter
  (`openFreeMapEdits`, matched by `source-layer`).

  New props:

  - `mapStyle` and `fallbackMapStyle`;
  - `cartoApiKey`;
  - `theme` (`'light' | 'dark'`);
  - `projection` (`'mercator' | 'globe'`, re-applied after every style load);
  - `initialCamera`;
  - `hideBasemapLabels`;
  - `extraLayers`;
  - `attribution` (`'compact' | 'full'`, never off);
  - `onMapReady(map, overlay)`, `onBasemapFallback` and `onBasemapError`.

  `basemap.ts` exports `OPENFREEMAP_STYLES`, `CARTO_STYLES`, `cartoStyleUrl`,
  `defaultBasemap`, `resolveBasemap`, `attachBasemapFallback`,
  `isOpenFreeMapStyle` and `openFreeMapEdits`. `defaultBasemap` and
  `resolveBasemap` name the provider of each style in `primaryProvider` and
  `fallbackProvider` (`'carto'`, `'openfreemap'`, or `'custom'` for a style the
  host passed; `defaultBasemap` returns the narrower `DefaultBasemap`, never
  custom and never without a fallback), and the reasons given to `onBasemapFallback` and
  `onBasemapError` carry `from` and `to` in the same terms. The overlay palette
  is `MAP_PALETTE[theme]`. `TellegenProvider` and `TellegenViewer` take
  `cartoApiKey`, and the default shell passes it and `theme` to its map.

  The map now runs in a host's Vite dev server with no `optimizeDeps` settings.
  MapLibre's worker URL comes from a new internal `./maplibre-worker` export,
  whose `development` condition keeps the worker's `?worker&url` import out of
  the dev server's pre-bundle, which stopped the dev server at startup.
  `maplibre-gl` is now `^6.15.0`.

  The controller gains `openBalancedModule({ name, moduleJson | payload,
geoLayer?, formulation?, activate?, frame? })` and
  `replaceLocalCase(previous, input)` to open a balanced powerio module from
  memory, `ControllerOptions.loadDefaultCases` and `apiBase: null`, and an
  internal gate so `load()` fetches nothing when default cases are off;
  `createApiClient({ apiBase: null })` returns `null`. `TellegenProvider`
  accepts `theme="dark"` (sets `data-theme` on the document root for the new
  dark tokens in `styles.css`) and marks the shell loaded when default cases are
  off. `@tellegen/svelte/components` exports `PanelHost` (with `showToolbar`
  and `--tellegen-dock-left/right/bottom` insets), `NotificationCenter` and the
  renderless `ShellEnvironment` for custom shells. Hosts with a Content Security
  Policy must allow `https://tiles.openfreemap.org` and `https://*.cartocdn.com`
  in `connect-src` and `img-src`.

### Patch Changes

- [#203](https://github.com/eigenergy/tellegen/pull/203) [`6f661e1`](https://github.com/eigenergy/tellegen/commit/6f661e1b007555e886ebf5b05be5d62e677621ba) Thanks [@samtalki](https://github.com/samtalki)! - Stop showing an in-progress calculation after a failed solve, and distinguish
  map initialization failures from case-file errors in notifications.

  Reject a missing AC/SOCWR browser input instead of silently falling back to the
  DC-only server solver, carrying forward the fail-closed boundary from the
  experimental AC OPF integration.

- [#203](https://github.com/eigenergy/tellegen/pull/203) [`13c761a`](https://github.com/eigenergy/tellegen/commit/13c761abb465fb17b57ddef807d7fcc151e236da) Thanks [@samtalki](https://github.com/samtalki)! - The basemap also switches to its fallback when the tile server answers HTTP
  5xx for every tile, not only when tile requests fail at the network level.

- [#197](https://github.com/eigenergy/tellegen/pull/197) [`ef46197`](https://github.com/eigenergy/tellegen/commit/ef461978ddf59b077ad801c9e8a32356e2300e72) Thanks [@samtalki](https://github.com/samtalki)! - The local case export menu offers PSS/E v35 (`psse35`) beside the v33 RAW.

- [#197](https://github.com/eigenergy/tellegen/pull/197) [`708d2f9`](https://github.com/eigenergy/tellegen/commit/708d2f9ae17fc19e9cf737c7079498d794cc0fa0) Thanks [@samtalki](https://github.com/samtalki)! - The case panel describes the coordinates of a generated grid (an ingest
  payload with `coords_method: "generated"`) as a synthetic grid sited on
  real geography, no longer as a topology layout.

- [#203](https://github.com/eigenergy/tellegen/pull/203) [`295c0f3`](https://github.com/eigenergy/tellegen/commit/295c0f3a03f977284e960e389d1309531088082c) Thanks [@samtalki](https://github.com/samtalki)! - Close the panel options menu when clicking outside it.
- Updated dependencies [[`a3fdbd3`](https://github.com/eigenergy/tellegen/commit/a3fdbd3923b964654582bb384b57bd0b4ab90887), [`2ccb61c`](https://github.com/eigenergy/tellegen/commit/2ccb61c3429e4488c06c90063fbd927614d445de), [`d476c3d`](https://github.com/eigenergy/tellegen/commit/d476c3d0fba3306a8fc5695e29cb936499c6112a), [`01f9a1e`](https://github.com/eigenergy/tellegen/commit/01f9a1e990bbec5a0adbc0545b9abb6edeb483dc), [`786ebc8`](https://github.com/eigenergy/tellegen/commit/786ebc893b2b17b0ef55be3e6cb608dc4cf545bf)]:
  - @tellegen/engine@0.4.0

## 0.3.0

### Minor Changes

- [#97](https://github.com/eigenergy/tellegen/pull/97) [`5d93591`](https://github.com/eigenergy/tellegen/commit/5d93591d74ecbba1b80fc1e0f9a9fc8e4de4cc31) Thanks [@samtalki](https://github.com/samtalki)! - Save a case with its changes and results as a portable Study. Planning goals are
  optional. Preserve geographic positions, line paths, drawings, camera settings,
  and unsolved cases. Keep cumulative demand changes and reset electrical inputs
  to the base case while retaining history.

  Add searchable equipment selection and bounded capacity or demand planning.
  Share supported objective expressions, verified candidate results, activity,
  and explicit proposal approval across browser and native tools. Every attempted
  solve counts against the stated budget.

  Restore Texas7k through reusable generator-cost preparation, with original cost
  data and measured fit errors available in Model details. Show every configured
  demo case, including an explanation when a case is unavailable.

  Add movable, resizable panels with saved layouts. Display geographic data on
  the map and drawing coordinates on a plain canvas. Include balanced AC power
  flow with voltage and angle results.

  Introduce `@tellegen/webmcp` at 0.1.0. Agents can list and select cases, query the
  same saved state displayed on screen, and continue a Study. Combine connection
  instructions and recorded tool activity in one Agent panel.

- [#129](https://github.com/eigenergy/tellegen/pull/129) [`4101e61`](https://github.com/eigenergy/tellegen/commit/4101e61cb6918e1c74164412de4ca0b5aee4c26c) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Add retained fixed-point multiconductor power-flow sessions. Reuse the sparse
  factorization and last converged voltage for automatic load P/Q edits, expose
  the live session through the browser engine, and materialize portable snapshots
  only when they are needed for save, export, or geography operations.

- [#113](https://github.com/eigenergy/tellegen/pull/113) [`653bf18`](https://github.com/eigenergy/tellegen/commit/653bf186dbc8528d95633364cc42e2529708542f) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Add multiconductor AC power flow for supported BMOPF networks and typed PowerIO
  inputs, including explicit neutral conductors and finite-leakage transformers.
  Inspect terminal voltages, currents, source powers, convergence, and KCL residuals.
  Save and reopen distribution studies with their input and solution modules.

  Use published PowerIO 0.11.1 dependencies. Portable electrical modules continue
  to use PowerIO IR generation 2.

  Apply the OpenDSS load-voltage envelope and report voltage validity separately
  from convergence. Support fixed-tap single-phase autotransformers, including
  ANSI Type A/B ratios, winding impedance, and excitation shunts.

  Retain electrical results when attaching geography to saved distribution studies.
  Share cancellable calculations between browser controls and WebMCP, expose
  terminal results to agent queries, and add compact notifications and
  privacy-filtered usage events.

### Patch Changes

- [#141](https://github.com/eigenergy/tellegen/pull/141) [`9c46ae4`](https://github.com/eigenergy/tellegen/commit/9c46ae441a60979b0cc42eedc7eb9934db5f03b9) Thanks [@samtalki](https://github.com/samtalki)! - Show the case file button on every device. It was hidden whenever the primary
  input could not hover, which included touchscreen laptops with a mouse and all
  phones, so local files could not be opened there at all. Drag and drop is now
  gated on `any-hover`/`any-pointer`, and the button no longer depends on it.
  Diagnosed by @MohamedNumair in [#110](https://github.com/eigenergy/tellegen/issues/110).

- [#126](https://github.com/eigenergy/tellegen/pull/126) [`7695a83`](https://github.com/eigenergy/tellegen/commit/7695a8397fd935859d498291f3629afd21df41ea) Thanks [@samtalki](https://github.com/samtalki)! - Restore the geographic map background in production builds by bundling the
  MapLibre worker and its dependencies. Grid overlays and map tiles render together.

- [#119](https://github.com/eigenergy/tellegen/pull/119) [`f645948`](https://github.com/eigenergy/tellegen/commit/f64594898b6f9d8c6f7c780946e71813f0af2712) Thanks [@dependabot](https://github.com/apps/dependabot)! - Update MapLibre GL to a release that passes the dependency audit.

  Keep Deck.gl packages on the same version and use the official MapLibre overlay for MapLibre 6 so the interleaved map remains usable.

- Updated dependencies [[`5d93591`](https://github.com/eigenergy/tellegen/commit/5d93591d74ecbba1b80fc1e0f9a9fc8e4de4cc31), [`4101e61`](https://github.com/eigenergy/tellegen/commit/4101e61cb6918e1c74164412de4ca0b5aee4c26c), [`653bf18`](https://github.com/eigenergy/tellegen/commit/653bf186dbc8528d95633364cc42e2529708542f)]:
  - @tellegen/engine@0.3.0

## 0.2.0

### Minor Changes

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`75021aa`](https://github.com/eigenergy/tellegen/commit/75021aa821ec5a93f35ff63ca87506aea52b42cf) Thanks [@samtalki](https://github.com/samtalki)! - Add one-pass typed JSON ingestion and bound browser file-drop batches by count
  and bytes.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`bf35bb3`](https://github.com/eigenergy/tellegen/commit/bf35bb3988a03efc43c1201ae960859c152d26b4) Thanks [@samtalki](https://github.com/samtalki)! - Recognize PowerIO 0.9 `.pio.json` packages, accept `.epc` and `.pwb` case files,
  and export studies as PowerWorld `.aux` or PSLF `.epc`.

- [#83](https://github.com/eigenergy/tellegen/pull/83) [`670958b`](https://github.com/eigenergy/tellegen/commit/670958b95cc05c7f24fe04694c51dfc9dd907e9a) Thanks [@samtalki](https://github.com/samtalki)! - A saved study now states the powerio release that wrote it, and a study saved by
  an earlier build no longer loads. Open the source case and save the study again.
  Case uploads are parsed as bytes, so a `.raw` or `.aux` exported in CP1252 is
  refused rather than silently mangled.

### Patch Changes

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`e6da333`](https://github.com/eigenergy/tellegen/commit/e6da333a54fbb2972cc5072a81c16601c2f1fe85) Thanks [@samtalki](https://github.com/samtalki)! - Keep valid network coordinates visible when a case also contains points that
  cannot be rendered on a Web Mercator map.

- [#85](https://github.com/eigenergy/tellegen/pull/85) [`de85124`](https://github.com/eigenergy/tellegen/commit/de851243086697c893a69d2ecabb8465fd8fbb75) Thanks [@samtalki](https://github.com/samtalki)! - Require deck.gl 9.3.10 or newer. The declared range moved from `^9.1.0`, so a
  consumer resolving an older 9.x now gets the newer one.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`4d5c2a4`](https://github.com/eigenergy/tellegen/commit/4d5c2a44a7bef2e2d0bfd0c16ab6a57a8c3daebb) Thanks [@samtalki](https://github.com/samtalki)! - Retire the wasm instance after a trap instead of serving the next request from
  it, bound the string entry points at the same 128 MiB limit as their byte
  counterparts, frame a single-point selection instead of clamping to maximum
  zoom, and keep the parsing indicator up while dropped JSON cases materialize.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`bd043f1`](https://github.com/eigenergy/tellegen/commit/bd043f15de3be6d3b78c236285bca570d0f3e8e6) Thanks [@samtalki](https://github.com/samtalki)! - Include lowered three-winding rows in rendered transmission topology and mark
  display-only rows as non-editable. Report canonical and rendered analysis row
  counts separately.
- Updated dependencies [[`75021aa`](https://github.com/eigenergy/tellegen/commit/75021aa821ec5a93f35ff63ca87506aea52b42cf), [`670958b`](https://github.com/eigenergy/tellegen/commit/670958b95cc05c7f24fe04694c51dfc9dd907e9a), [`4d5c2a4`](https://github.com/eigenergy/tellegen/commit/4d5c2a44a7bef2e2d0bfd0c16ab6a57a8c3daebb), [`bd043f1`](https://github.com/eigenergy/tellegen/commit/bd043f15de3be6d3b78c236285bca570d0f3e8e6), [`bf35bb3`](https://github.com/eigenergy/tellegen/commit/bf35bb3988a03efc43c1201ae960859c152d26b4)]:
  - @tellegen/engine@0.2.0
