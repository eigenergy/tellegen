import { mount, unmount } from 'svelte';
import Lifecycle from './AcOpfLifecycle.svelte';

const component = mount(Lifecycle, { target: document.getElementById('app')! });
window.unmountAcOpfProvider = () => unmount(component);

declare global {
	interface Window {
		unmountAcOpfProvider(): Promise<void>;
		acOpfLifecycle: {
			available(): boolean;
			start(text: string): Promise<void>;
			solved(): boolean;
			/** Terminate the idle kept worker so the next solve starts a new one. */
			release(): boolean;
		};
	}
}
