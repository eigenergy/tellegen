import { mount } from 'svelte';
import '../../../../../packages/svelte/src/lib/styles.css';
import Fixture from './SynchronousAreasFixture.svelte';
mount(Fixture, { target: document.getElementById('app')! });
