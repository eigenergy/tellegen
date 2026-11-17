/**
 * Every error code a Tellegen tool can return, with when it happens and what
 * the caller should do next. A failed call returns
 * `{ ok: false, error: { code, message } }`; the message says the same thing
 * for the specific call.
 *
 * This file has no imports, so the docs script can load it directly.
 */
export const TELLEGEN_ERROR_CODES = {
  // The call itself
  INVALID_INPUT: {
    when: "An argument is missing, has the wrong type or is out of range.",
    next: "Fix the argument the message names and call again.",
  },
  INVALID_OUTPUT: {
    when: "The tool produced a result that is not valid JSON for its schema.",
    next: "Report it; this is a bug in the page, not in the call.",
  },
  OUTPUT_TOO_LARGE: {
    when: "The result is longer than the output limit.",
    next: "Ask for less: lower `limit` or name fewer elements.",
  },
  TIMEOUT: {
    when: "The tool did not finish within its time limit.",
    next: "Call again, or use a smaller case or a shorter simulation.",
  },
  CANCELLED: {
    when: "The agent or the person stopped the call.",
    next: "Call again only if the result is still wanted.",
  },
  BUSY: {
    when: "Another tool call that changes the case is still running.",
    next: "Wait for it to finish, then call again.",
  },
  TOOL_FAILED: {
    when: "The tool failed for a reason no other code covers.",
    next: "Read the message; call `inspect_case` to see the current state.",
  },
  INVALID_CONFIGURATION: {
    when: "The page set up the tools with an invalid option.",
    next: "Report it; an agent cannot fix this.",
  },

  // Which case is shown
  NO_ACTIVE_CASE: {
    when: "No case that can be solved is shown.",
    next: "Call `list_cases`, then `select_case`.",
  },
  CASE_NOT_FOUND: {
    when: "The case ID is not one `list_cases` returned.",
    next: "Call `list_cases` and use one of its IDs.",
  },
  CASE_UNAVAILABLE: {
    when: "The case exists but cannot be opened, for example its data failed to load.",
    next: "Choose another case from `list_cases`.",
  },
  CASE_NOT_READY: {
    when: "The case is still loading.",
    next: "Wait a moment, then call `inspect_case` again.",
  },
  CASE_SOLVING: {
    when: "A solve is still running.",
    next: "Wait for it to finish, then call again.",
  },
  CASE_DATA_UNAVAILABLE: {
    when: "The page does not hold the case data the tool needs.",
    next: "Ask the person to reload the page.",
  },
  STALE_CASE: {
    when: "The case shown changed since `case_id` was read.",
    next: "Call `inspect_case` and use the new `case_id`.",
  },
  STALE_REVISION: {
    when: "The case, its settings or the display changed since the revision was read.",
    next: "Read the state again with the tool the message names and use the new revision.",
  },
  SAVED_STATE_READ_ONLY: {
    when: "A saved Study state is shown, which cannot be edited.",
    next: "Use the Study tools, or call `select_case` to return to the live case.",
  },

  // What the case supports
  ELEMENT_NOT_FOUND: {
    when: "The bus or branch ID is not in the case.",
    next: "Find a valid ID with `query_network`.",
  },
  EDIT_NOT_SUPPORTED: {
    when: "The element cannot be edited, for example a bus that is shown but not modelled.",
    next: "Choose an element that `query_network` reports as editable.",
  },
  EDIT_OUT_OF_RANGE: {
    when: "The change is outside the limits for that element.",
    next: "Choose a value inside the range the message gives.",
  },
  FORMULATION_UNAVAILABLE: {
    when: "The calculation is not available for this case.",
    next: "Call `inspect_settings` to list the calculations that are.",
  },
  METRIC_UNAVAILABLE: {
    when: "The calculation shown does not produce the requested quantity, such as a price from a power flow.",
    next: "Ask for a quantity the message lists, or run a calculation that produces it.",
  },
  INVALID_SORT: {
    when: "`sort_by` does not apply to that kind of element.",
    next: "Choose another `sort_by`.",
  },
  SENSITIVITY_UNAVAILABLE: {
    when: "The calculation shown has no sensitivity for the requested target.",
    next: "Run a calculation that has one (DC OPF for prices), or change the target.",
  },
  PREVIEW_UNAVAILABLE: {
    when: "The calculation shown cannot predict the effect of an edit.",
    next: "Apply the edit with `update_case`, which solves it exactly.",
  },
  DYNAMICS_OFF: {
    when: "Dynamic simulation is turned off in Settings.",
    next: "Ask the person to turn Dynamics on in Settings.",
  },
  DYNAMICS_ATTACHED: {
    when: "The case already has dynamic data and the import would replace it.",
    next: "Ask the person, then call `import_case` with `replace_dynamics: true`.",
  },
  TRACE_UNAVAILABLE: {
    when: "The browser cannot compute the checksum that identifies the case data.",
    next: "Use a browser with Web Crypto, on an https page or localhost.",
  },

  // Files
  IMPORT_FAILED: {
    when: "The file could not be read as a case.",
    next: "Check the file name's extension and the text; the message says what failed.",
  },
  EXPORT_UNAVAILABLE: {
    when: "The case cannot be written in the requested format.",
    next: "Choose a format the `export_case` schema lists for this case.",
  },

  // Capacity planning
  PLANNING_UNAVAILABLE: {
    when: "Capacity planning cannot run on this case or this branch.",
    next: "Solve the case as DC OPF, and choose branches `query_network` returns.",
  },
  UNKNOWN_PROPOSAL: {
    when: "No proposal with that ID is waiting.",
    next: "Call `propose_capacity_plan` first.",
  },
  STALE_PROPOSAL: {
    when: "The case changed after the proposal was computed.",
    next: "Call `propose_capacity_plan` again.",
  },
  APPROVAL_REQUIRED: {
    when: "The person has not approved the proposal in the Agent panel.",
    next: "Ask them to review it and press Approve, then call again.",
  },
} as const satisfies Record<string, { when: string; next: string }>;

/** An error code a Tellegen tool can return. */
export type TellegenErrorCode = keyof typeof TELLEGEN_ERROR_CODES;
