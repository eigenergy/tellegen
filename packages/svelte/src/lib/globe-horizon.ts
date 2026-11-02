import type { Layer, LayerExtension } from '@deck.gl/core';

// On the globe, geometry past the horizon is on the far side of the earth and
// must not show through it. A depth test against maplibre's globe cannot do
// this: deck's globe and maplibre's write different depth near the surface,
// so the test also cut away parts of a network facing the camera. Instead
// each vertex measures itself against the horizon in deck's own common space,
// where the globe is a sphere at the origin and `project.cameraPosition` is
// the eye: a surface point p faces the eye when (eye − p)·p > 0. Fragments
// past it are discarded; on the flat map nothing changes.
const INJECT = {
	'vs:#decl': 'out float tgHorizon;',
	'vs:DECKGL_FILTER_GL_POSITION': `
		vec3 tgPoint = geometry.position.xyz;
		tgHorizon = project.projectionMode == PROJECTION_MODE_GLOBE
			? dot(tgPoint, project.cameraPosition - tgPoint) / max(dot(tgPoint, tgPoint), 1.0) + 1e-4
			: 1.0;
	`,
	'fs:#decl': 'in float tgHorizon;',
	'fs:DECKGL_FILTER_COLOR': 'if (tgHorizon < 0.0) discard;'
};

/** A deck.gl extension that hides geometry on the far side of the globe. It
 * takes deck's `LayerExtension` class, which the map loads lazily. */
export function createGlobeHorizon(Base: typeof LayerExtension): LayerExtension {
	class GlobeHorizonExtension extends Base {
		static extensionName = 'GlobeHorizonExtension';
		getShaders() {
			return { inject: INJECT };
		}
	}
	return new GlobeHorizonExtension();
}

/** The layers with the extension added, once each. */
export function withGlobeHorizon(layers: Layer[], horizon: LayerExtension | null): Layer[] {
	if (!horizon) return layers;
	return layers.map((layer) => {
		const extensions = layer.props.extensions ?? [];
		return extensions.includes(horizon)
			? layer
			: layer.clone({ extensions: [...extensions, horizon] });
	});
}
