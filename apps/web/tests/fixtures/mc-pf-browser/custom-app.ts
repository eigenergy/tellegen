import { mount } from 'svelte';
import CustomApp from '../../../../../examples/svelte-minimal/src/CustomApp.svelte';
import '@tellegen/svelte/styles.css';
mount(CustomApp, { target: document.getElementById('app')! });
