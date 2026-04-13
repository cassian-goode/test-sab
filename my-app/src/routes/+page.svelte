<script>
	import { onMount } from 'svelte';
	import CodeMirror from '$lib/CodeMirror.svelte';

	let source = $state(`#set page(width: 300pt, height: 180pt, margin: 16pt)
#set text(size: 14pt)

Hello, Typst!

This came from the left pane.
`);

	let svg = $state('');
	let error = $state('');
	let ready = $state(false);
	let compiling = $state(false);
	let status = $state('Starting...');
	let initError = $state('');

	let compileToSvg = null;
	const extensions = [];

	let svgUrl = $derived(svg ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}` : '');

	onMount(async () => {
		status = 'Loading Wasm module...';

		try {
			const mod = await import('$lib/typst-bridge/typst_bridge.js');
			status = 'Initializing Wasm...';

			await mod.default();

			compileToSvg = mod.compile_to_svg_wasm;
			ready = true;
			status = 'Wasm ready';
		} catch (err) {
			initError = err instanceof Error ? err.message : String(err);
			status = 'Wasm failed to load';
			console.error('Wasm init failed:', err);
		}
	});

	function compileNow(sourceToCompile) {
		if (!ready || !compileToSvg) return;

		compiling = true;
		error = '';
		status = 'Compiling...';

		try {
			svg = compileToSvg(sourceToCompile);
			status = 'Compile succeeded';
		} catch (err) {
			svg = '';
			error = err instanceof Error ? err.message : String(err);
			status = 'Compile failed';
			console.error('Compile failed:', err);
		} finally {
			compiling = false;
		}
	}

	$effect(() => {
		if (!ready || !compileToSvg) return;

		const pendingSource = source;

		const timeout = setTimeout(() => {
			compileNow(pendingSource);
		}, 50);

		return () => clearTimeout(timeout);
	});
</script>

<div class="toolbar">
	<span class="status">{status}</span>
</div>

{#if initError}
	<pre class="error">Init error: {initError}</pre>
{/if}

<div class="layout">
	<div class="pane editor-pane">
		<CodeMirror bind:value={source} {extensions} />
	</div>

	<div class="pane preview-pane">
		{#if error}
			<pre class="error">{error}</pre>
		{:else if svgUrl}
			<img class="preview-image" src={svgUrl} alt="Typst preview" />
		{:else}
			<p>No preview yet.</p>
		{/if}
	</div>
</div>

<style>
	:global(html, body) {
		margin: 0;
		height: 100%;
	}

	:global(body) {
		font-family: Arial, Helvetica, sans-serif;
		background: #f7f7f7;
	}

	:global(#svelte) {
		height: 100%;
	}

	.toolbar {
		padding: 1rem 1rem 0 1rem;
		display: flex;
		gap: 1rem;
		align-items: center;
	}

	.status {
		font-size: 0.9rem;
		color: #444;
	}

	.layout {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
		height: calc(100vh - 40px);
		padding: 1rem;
		box-sizing: border-box;
	}

	.pane {
		min-width: 0;
		min-height: 0;
	}

	.editor-pane {
		height: 100%;
	}

	.preview-pane {
		border: 1px solid #ddd;
		border-radius: 8px;
		background: white;
		overflow: auto;
		padding: 1rem;
		box-sizing: border-box;
	}

	.preview-image {
		display: block;
		max-width: 100%;
		height: auto;
	}

	.error {
		margin: 0 1rem 1rem 1rem;
		white-space: pre-wrap;
		font-family: monospace;
		color: #b00020;
	}
</style>