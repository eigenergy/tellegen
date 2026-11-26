import type { Page } from '@playwright/test';

/** Fraction of the map canvas within a few units of `rgb`. */
export async function paintedFraction(page: Page, rgb: [number, number, number]) {
	const canvas = page.locator('.maplibregl-canvas');
	const png = await canvas.screenshot();
	return page.evaluate(
		async ({ base64, rgb }) => {
			const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
			const image = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
			const pixels = new OffscreenCanvas(image.width, image.height);
			const context = pixels.getContext('2d')!;
			context.drawImage(image, 0, 0);
			const { data } = context.getImageData(0, 0, image.width, image.height);
			let painted = 0;
			for (let i = 0; i < data.length; i += 4) {
				if (
					Math.abs(data[i] - rgb[0]) < 3 &&
					Math.abs(data[i + 1] - rgb[1]) < 3 &&
					Math.abs(data[i + 2] - rgb[2]) < 3
				)
					painted++;
			}
			image.close();
			return painted / (data.length / 4);
		},
		{ base64: png.toString('base64'), rgb }
	);
}
