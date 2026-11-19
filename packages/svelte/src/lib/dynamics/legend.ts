/** Legend helpers shared by the map legend and the layers.
 * @block Legend helpers */
export { dynVoltageGradient } from '../colors.js';
import { haloRadius } from './layers.js';

/** A legend ring for `dfHz`, scaled to fit a 30 px swatch. */
export const haloLegendRadius = (dfHz: number): number => Math.min(14, haloRadius(dfHz) / 2);
