/** Grid exchange formats the committed state of a case can export, by PowerIO token. */
export const CASE_EXPORT_FORMATS: readonly { token: string; label: string }[] = [
	{ token: 'matpower', label: 'MATPOWER (.m)' },
	{ token: 'psse', label: 'PSS/E (.raw)' },
	{ token: 'psse35', label: 'PSS/E v35 (.raw)' },
	{ token: 'powerworld', label: 'PowerWorld (.aux)' },
	{ token: 'pslf', label: 'PSLF (.epc)' },
	{ token: 'powermodels-json', label: 'PowerModels (.json)' },
	{ token: 'pandapower-json', label: 'pandapower (.json)' }
];

/** The materialized case as a retained PowerIO module, the **Save** action's file. */
export const PIO_MODULE_EXPORT = { token: 'pio-module', label: 'PowerIO module (.pio.json)' };
