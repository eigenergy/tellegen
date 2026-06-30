<script lang="ts">
	const SITE_ORIGIN = 'https://tellegen.dev';

	let {
		path,
		title,
		description,
		socialDescription = description
	}: {
		path: string;
		title: string;
		description: string;
		socialDescription?: string;
	} = $props();

	const canonical = $derived(`${SITE_ORIGIN}${path}`);
</script>

<svelte:head>
	<title>{title}</title>
	<meta name="description" content={description} />
	<link rel="canonical" href={canonical} />
	<meta property="og:title" content={title} />
	<meta property="og:description" content={socialDescription} />
	<meta property="og:url" content={canonical} />
	<meta name="twitter:title" content={title} />
	<meta name="twitter:description" content={socialDescription} />
</svelte:head>
