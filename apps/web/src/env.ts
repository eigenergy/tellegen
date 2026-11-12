import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	PUBLIC_TELLEGEN_ACOPF_WASM_URL: {
		public: true,
		static: true,
		description: 'Browser AC OPF solver URL. Defaults to the asset bundled with the website.',
		schema: (value) => value?.trim() || '/experimental-acopf/tellegen_acopf_wasi.wasm'
	},
	PUBLIC_CARTO_BASEMAP_KEY: {
		public: true,
		static: true,
		description:
			"Optional CARTO basemap key (carto.com/basemaps/apikey). With it, the map leads with CARTO's style and falls back to OpenFreeMap; empty or unset reverses the order. The browser sends it with every tile request, so restrict it to the hosts that serve this app.",
		// A value saved with a stray newline or space is no key.
		schema: (value) => value?.trim() ?? ''
	},
	PUBLIC_WEBMCP_ORIGIN_TRIAL_TOKEN: {
		public: true,
		static: true,
		description:
			'Optional WebMCP origin trial token for the host that serves this build (Chrome and Edge register one per origin). The prerendered pages carry it in an origin-trial meta tag, so visitors get document.modelContext without enabling a browser flag. Empty or unset adds no tag.',
		schema: (value) => value?.trim() ?? ''
	},
	PUBLIC_WEBMCP_EXPOSED_TO: {
		public: true,
		static: true,
		description:
			"Optional comma-separated origins, such as https://maptogrid.example, that may discover and call this tab's WebMCP tools when they embed it. The embedding page must also opt in. Empty or unset keeps the tools same-origin.",
		schema: (value) =>
			(value ?? '')
				.split(',')
				.map((origin) => origin.trim())
				.filter((origin) => origin.length > 0)
				.join(',')
	}
});
