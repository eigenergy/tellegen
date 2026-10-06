import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	PUBLIC_TELLEGEN_ACOPF_WASM_URL: {
		public: true,
		static: true,
		schema: (value) => value || undefined
	}
});
