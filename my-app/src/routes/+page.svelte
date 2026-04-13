<script>
	import { onMount } from 'svelte';
	import CodeMirror from '$lib/CodeMirror.svelte';

	let source = $state(`#set page(width: 300pt, height: 180pt, margin: 16pt)
#set text(size: 14pt)

Hello, Typst!

This came from the left pane.
`);

	// Change: Keep stable page entries so unchanged pages can be reused.
	// Each entry stores the original SVG plus an object URL for display.
	let pageEntries = $state([]);

	let error = $state('');
	let status = $state('Starting...');
	let workerReady = $state(false);
	let compiling = $state(false);

	let worker = null;
	let latestRequestId = 0;

	const extensions = [];

	function makePageEntry(index, svg) {
		const blob = new Blob([svg], { type: 'image/svg+xml' });
		const url = URL.createObjectURL(blob);

		return { index, svg, url };
	}

	function revokePageEntry(entry) {
		if (entry?.url) {
			URL.revokeObjectURL(entry.url);
		}
	}

	function replacePages(nextSvgs) {
		const nextEntries = [];

		// Change: Only create new page URLs for pages whose SVG actually changed.
		// Unchanged pages keep the same object identity and URL.
		for (let index = 0; index < nextSvgs.length; index += 1) {
			const nextSvg = nextSvgs[index];
			const previous = pageEntries[index];

			if (previous && previous.svg === nextSvg) {
				nextEntries.push(previous);
			} else {
				if (previous) {
					revokePageEntry(previous);
				}

				nextEntries.push(makePageEntry(index, nextSvg));
			}
		}

		// Change: Revoke URLs for pages that disappeared (for example if the
		// document became shorter).
		for (let index = nextEntries.length; index < pageEntries.length; index += 1) {
			revokePageEntry(pageEntries[index]);
		}

		pageEntries = nextEntries;
	}

	function clearPages() {
		for (const entry of pageEntries) {
			revokePageEntry(entry);
		}

		pageEntries = [];
	}

	function requestCompile(sourceToCompile) {
		if (!worker || !workerReady) return;

		latestRequestId += 1;
		compiling = true;
		error = '';
		status = 'Compiling…';

		worker.postMessage({
			type: 'compile',
			requestId: latestRequestId,
			source: sourceToCompile
		});
	}

	onMount(() => {
		worker = new Worker(new URL('../lib/typst-worker.js', import.meta.url), {
			type: 'module'
		});

		worker.onmessage = (event) => {
			const message = event.data ?? {};

			if (message.type === 'ready') {
				workerReady = true;
				status = 'Worker ready';
				return;
			}

			if (message.type === 'init-error') {
				error = message.error;
				status = 'Worker failed to initialize';
				return;
			}

			// Ignore stale compile results so only the newest request updates the preview.
			if (message.requestId !== latestRequestId) return;

			if (message.type === 'compile-result') {
				replacePages(message.pages ?? []);
				compiling = false;
				error = '';
				status = `Compile succeeded (${pageEntries.length} page${pageEntries.length === 1 ? '' : 's'})`;
				return;
			}

			if (message.type === 'compile-error') {
				compiling = false;
				error = message.error;
				status = 'Compile failed';
			}
		};

		return () => {
			worker?.terminate();
			clearPages();
		};
	});

	$effect(() => {
		if (!workerReady) return;

		const pendingSource = source;

		const timeout = setTimeout(() => {
			requestCompile(pendingSource);
		}, 300);

		return () => clearTimeout(timeout);
	});
</script>

<div class="toolbar">
	<span class="status">{status}</span>
</div>

<div class="layout">
	<div class="pane editor-pane">
		<CodeMirror bind:value={source} {extensions} />
	</div>

	<div class="pane preview-pane">
		{#if error}
			<pre class="error">{error}</pre>
		{:else if pageEntries.length > 0}
			<div class="page-stack">
				{#each pageEntries as page (page.index)}
					<div class="page-shell">
						<img class="page-image" src={page.url} alt={`Typst page ${page.index + 1}`} />
					</div>
				{/each}
			</div>
		{:else if compiling}
			<p class="placeholder">Compiling preview…</p>
		{:else}
			<p class="placeholder">No preview yet.</p>
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
		align-items: center;
	}

	.status {
		font-size: 0.9rem;
		color: #444;
	}

	.layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
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
		background: #eef0f3;
		overflow: auto;
		padding: 1.5rem;
		box-sizing: border-box;
	}

	.page-stack {
		display: flex;
		flex-direction: column;
		gap: 24px;
		align-items: center;
	}

	.page-shell {
		max-width: 100%;
	}

	.page-image {
		display: block;
		max-width: 100%;
		height: auto;
		background: white;
		box-shadow:
			0 1px 3px rgba(0, 0, 0, 0.12),
			0 8px 24px rgba(0, 0, 0, 0.08);
	}

	.placeholder {
		margin: 0;
		color: #666;
	}

	.error {
		margin: 0;
		white-space: pre-wrap;
		font-family: monospace;
		color: #b00020;
	}
</style>