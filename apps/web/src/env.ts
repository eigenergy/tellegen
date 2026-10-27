import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	PUBLIC_TELLEGEN_ACOPF_WASM_URL: {
		public: true,
		static: true,
		schema: (value) => value || undefined
	},
	PUBLIC_CARTO_BASEMAP_KEY: {
		public: true,
		static: true,
		description:
			"Optional CARTO basemap key (carto.com/basemaps/apikey). With it, the map leads with CARTO's style and falls back to OpenFreeMap; empty or unset reverses the order. The browser sends it with every tile request, so restrict it to the hosts that serve this app.",
		// A value saved with a stray newline or space is no key.
		schema: (value) => value?.trim() ?? ''
	}
});
