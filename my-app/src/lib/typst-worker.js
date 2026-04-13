import init, { CompilerSession } from './typst-bridge/typst_bridge.js';

let session = null;
let initPromise = null;

async function ensureSession() {
	// Initialize Wasm and the persistent Rust session once inside the worker.
	if (!initPromise) {
		initPromise = (async () => {
			await init();
			session = new CompilerSession();
			self.postMessage({ type: 'ready' });
		})().catch((error) => {
			const message = error instanceof Error ? error.message : String(error);
			self.postMessage({ type: 'init-error', error: message });
			throw error;
		});
	}

	return initPromise;
}

ensureSession();

self.onmessage = async (event) => {
	const message = event.data ?? {};

	if (message.type !== 'compile') return;

	try {
		await ensureSession();

		const requestStart = performance.now();

		// Measure the full Typst compile.
		const compileStart = performance.now();
		const compileInfo = session.compile_document(message.source);
		const compileMs = performance.now() - compileStart;

		// Measure the whole hash+render pass that now returns only changed pages.
		const renderStart = performance.now();
		const changed = session.render_changed_pages();
		const renderPassMs = performance.now() - renderStart;

		const totalTimeToFullPreviewMs = performance.now() - requestStart;

		self.postMessage({
			type: 'compile-result',
			requestId: message.requestId,
			pageCount: compileInfo.page_count ?? 0,
			changedPages: changed.changed_pages ?? [],
			metrics: {
				pageCount: compileInfo.page_count ?? 0,
				compileMs,
				renderPassMs,
				totalTimeToFullPreviewMs,
				cacheHitCount: changed.cache_hit_count ?? 0,
				cacheMissCount: changed.cache_miss_count ?? 0
			}
		});
	} catch (error) {
		const messageText = error instanceof Error ? error.message : String(error);
		console.error('Worker compile failed:', error);

		self.postMessage({
			type: 'compile-error',
			requestId: message.requestId,
			error: messageText
		});
	}
};