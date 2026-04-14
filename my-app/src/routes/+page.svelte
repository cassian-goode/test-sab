<script>
	import { onMount } from 'svelte';
	import CodeMirror from '$lib/CodeMirror.svelte';
	import initialSourceText from '$lib/sample-typst-source.txt?raw';

	let source = $state(initialSourceText);

	let pageEntries = $state([]);
	let error = $state('');
	let status = $state('Starting...');
	let workerReady = $state(false);
	let compiling = $state(false);

	let worker = null;
	let latestRequestId = 0;

	let metrics = $state({
		pageCount: 0,
		compileMs: null,
		renderPassMs: null,
		totalTimeToFullPreviewMs: null,
		cacheHitCount: 0,
		cacheMissCount: 0,
		threadCount: null
	});

	const extensions = [];

	function formatMs(value) {
		return value == null ? '—' : `${value.toFixed(1)} ms`;
	}

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

	function clearPages() {
		for (const entry of pageEntries) {
			revokePageEntry(entry);
		}

		pageEntries = [];
	}

	function replaceChangedPages(changedPages, pageCount) {
		const entriesByIndex = new Map(pageEntries.map((entry) => [entry.index, entry]));

		for (const changedPage of changedPages) {
			const previous = entriesByIndex.get(changedPage.index);
			if (previous) {
				revokePageEntry(previous);
			}

			entriesByIndex.set(changedPage.index, makePageEntry(changedPage.index, changedPage.svg));
		}

		for (const [index, entry] of entriesByIndex) {
			if (index >= pageCount) {
				revokePageEntry(entry);
				entriesByIndex.delete(index);
			}
		}

		const nextEntries = [];
		for (let index = 0; index < pageCount; index += 1) {
			const entry = entriesByIndex.get(index);
			if (entry) {
				nextEntries.push(entry);
			}
		}

		pageEntries = nextEntries;
	}

	function requestCompile(sourceToCompile) {
		if (!worker || !workerReady) return;

		latestRequestId += 1;
		compiling = true;
		error = '';
		status = 'Compiling…';

		metrics = {
			pageCount: 0,
			compileMs: null,
			renderPassMs: null,
			totalTimeToFullPreviewMs: null,
			cacheHitCount: 0,
			cacheMissCount: 0,
			threadCount: metrics.threadCount
		};

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
				metrics = {
					...metrics,
					threadCount: message.threadCount ?? null
				};

				status = `Worker ready (${message.threadCount ?? 1} wasm threads)`;
				return;
			}

			if (message.type === 'init-error') {
				error = message.error;
				status = 'Worker failed to initialize';
				return;
			}

			if (message.requestId !== latestRequestId) return;

			if (message.type === 'compile-result') {
				replaceChangedPages(message.changedPages ?? [], message.pageCount ?? 0);

				metrics = {
					pageCount: message.metrics.pageCount,
					compileMs: message.metrics.compileMs,
					renderPassMs: message.metrics.renderPassMs,
					totalTimeToFullPreviewMs: message.metrics.totalTimeToFullPreviewMs,
					cacheHitCount: message.metrics.cacheHitCount ?? 0,
					cacheMissCount: message.metrics.cacheMissCount ?? 0,
					threadCount: message.metrics.threadCount ?? metrics.threadCount
				};

				compiling = false;
				error = '';
				status = `Compile succeeded (${message.metrics.pageCount} page${message.metrics.pageCount === 1 ? '' : 's'}, ${message.metrics.cacheMissCount} changed)`;
				return;
			}

			if (message.type === 'compile-error') {
				compiling = false;
				error = message.error;
				status = 'Compile failed';
			}
		};

		const threadCount = Math.max(1, (navigator.hardwareConcurrency ?? 4) - 1);

		worker.postMessage({
			type: 'init',
			threadCount
		});

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
		<div class="metrics">
			<div><strong>Page count:</strong> {metrics.pageCount || '—'}</div>
			<div><strong>Wasm threads:</strong> {metrics.threadCount ?? '—'}</div>
			<div><strong>Full Typst compile:</strong> {formatMs(metrics.compileMs)}</div>
			<div><strong>Hash/render pass:</strong> {formatMs(metrics.renderPassMs)}</div>
			<div><strong>Time to full preview:</strong> {formatMs(metrics.totalTimeToFullPreviewMs)}</div>
			<div><strong>Cache hits:</strong> {metrics.cacheHitCount}</div>
			<div><strong>Cache misses:</strong> {metrics.cacheMissCount}</div>
		</div>

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

	.metrics {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.35rem;
		background: white;
		border: 1px solid #ddd;
		border-radius: 8px;
		padding: 0.9rem 1rem;
		margin-bottom: 1rem;
		font-size: 0.9rem;
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