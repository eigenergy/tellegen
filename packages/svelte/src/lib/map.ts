import type { ComponentProps } from 'svelte';
import TellegenMap from './TellegenMap.svelte';

export { default as TellegenMap } from './TellegenMap.svelte';
export {
	CARTO_STYLES,
	OPENFREEMAP_STYLES,
	attachBasemapFallback,
	cartoBasemap,
	cartoStyleUrl,
	defaultBasemap,
	isOpenFreeMapStyle,
	openFreeMapEdits,
	resolveBasemap
} from './basemap.js';
export type {
	BasemapFallbackReason,
	BasemapMapLike,
	BasemapProvider,
	BasemapSpec,
	BasemapStyle,
	BasemapSwitch,
	BasemapTheme,
	CartoBasemap,
	DefaultBasemap,
	MapProjection,
	MapTheme,
	ResolvedBasemap,
	StyleEdit
} from './basemap.js';
export {
	LOADING_AMBER,
	LOADING_AT_LIMIT,
	LOADING_ATTENTION,
	LOADING_BASE,
	LOADING_RED,
	MAP_PALETTE,
	branchColor,
	branchLoadingGradient,
	branchWidth,
	busNeutral,
	busRadius,
	busMarkerRadius,
	busRadiusFor,
	demandColor,
	demandDomain,
	demandGradient,
	priceColor,
	priceDomain,
	priceGradient,
	scalarDomain,
	scalarGradient,
	scalarStops,
	sensColor,
	sensFlatColor,
	sensGradient,
	sensNeutral,
	sensitivityDomain,
	UNIFORM_BUS_GROUND_RADIUS,
	UNIFORM_BUS_MIN_RADIUS,
	UNIFORM_BUS_RADIUS
} from './colors.js';
export { caseDeltas, displayMetaFor, displaySeriesFor } from './display.js';
export { busColorScale, displayRange, lineLoadingKey } from './map-legend.js';
export { ALL_MAP_LAYERS, busesForLayers, resolveMapLayers } from './map-layers.js';
export type { ColorStop, MapPalette, RGBA, SensitivityDomain } from './colors.js';
export type { BusColorScale, LineLoadingStep, ScaleEnd } from './map-legend.js';
export type { MapLayers, ResolvedMapLayers } from './map-layers.js';
export type { DisplayOption } from './display.js';

export type TellegenMapProps = ComponentProps<typeof TellegenMap>;
