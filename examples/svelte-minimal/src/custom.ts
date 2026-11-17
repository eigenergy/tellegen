import { mount } from 'svelte';
import CustomApp from './CustomApp.svelte';
import '@tellegen/svelte/styles.css';
mount(CustomApp, { target: document.getElementById('app')! });
