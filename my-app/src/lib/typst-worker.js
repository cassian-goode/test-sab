let session = null;
let initPromise = null;
let runtimeThreadCount = 1;

async function ensureSession(threadCount) {
	if (initPromise) {
		return initPromise;
	}

	initPromise = (async () => {
		if (!self.crossOriginIsolated) {
			throw new Error(
				'SharedArrayBuffer / wasm threads require cross-origin isolation. Check COOP/COEP headers.'
			);
		}

		if (typeof SharedArrayBuffer === 'undefined') {
			throw new Error('SharedArrayBuffer is unavailable in this context.');
		}

		const threadedPkg = await import('./typst-bridge/typst_bridge.js');

		await threadedPkg.default();
		threadedPkg.init_panic_hook?.();

		console.log(
			'threaded wasm memory buffer is SAB:',
			threadedPkg.__wasm.memory.buffer instanceof SharedArrayBuffer
		);

		runtimeThreadCount = Math.max(1, threadCount);
		await threadedPkg.initThreadPool(runtimeThreadCount);

		session = new threadedPkg.CompilerSession();

		self.postMessage({
			type: 'ready',
			threadCount: runtimeThreadCount
		});
	})().catch((error) => {
		const message = error instanceof Error ? error.message : String(error);
		self.postMessage({ type: 'init-error', error: message });
		throw error;
	});

	return initPromise;
}

self.onmessage = async (event) => {
	const message = event.data ?? {};

	if (message.type === 'init') {
		await ensureSession(message.threadCount ?? 1);
		return;
	}

	if (message.type !== 'compile') return;

	try {
		if (!initPromise) {
			throw new Error('Worker has not been initialized yet.');
		}

		await initPromise;

		const requestStart = performance.now();

		const compileStart = performance.now();
		const compileInfo = session.compile_document(message.source);
		const compileMs = performance.now() - compileStart;

		const renderStart = performance.now();
		const changed = session.render_changed_pages();
		const renderPassMs = performance.now() - renderStart;

		console.log(
			'changed pages:',
			changed.changed_pages?.length ?? 0,
			'first svg length:',
			changed.changed_pages?.[0]?.svg?.length ?? 0,
			'first svg prefix:',
			changed.changed_pages?.[0]?.svg?.slice(0, 200) ?? null
    );
		
		const firstSvg = changed.changed_pages?.[0]?.svg ?? '';
    console.log('first svg length:', firstSvg.length);
    console.log('first svg prefix:', firstSvg.slice(0, 300));
    console.log('first svg suffix:', firstSvg.slice(-300));

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
				cacheMissCount: changed.cache_miss_count ?? 0,
				threadCount: runtimeThreadCount
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