import { mount } from 'svelte';
import App from './App.svelte';
import '@tellegen/svelte/styles.css';
import './style.css';

mount(App, {
	target: document.getElementById('app')!
});
