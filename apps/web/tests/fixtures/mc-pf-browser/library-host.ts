import { mount } from 'svelte';
import LibraryHost from './LibraryHost.svelte';
import '@tellegen/svelte/styles.css';
mount(LibraryHost, { target: document.getElementById('app')! });
