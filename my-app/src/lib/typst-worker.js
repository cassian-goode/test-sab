import init, { CompilerSession } from './typst-bridge/typst_bridge.js';

let session = null;
let initPromise = null;

async function ensureSession() {
	// Change: Initialize Wasm and the persistent Rust session once inside the worker.
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

// Kick off initialization immediately so the worker is warm as soon as possible.
ensureSession();

self.onmessage = async (event) => {
	const message = event.data ?? {};

	if (message.type !== 'compile') return;

	try {
		await ensureSession();

		// Change: Compile inside the worker, not on the main thread.
		const result = session.compile(message.source);

		self.postMessage({
			type: 'compile-result',
			requestId: message.requestId,
			pages: result.pages
		});
	} catch (error) {
		const messageText = error instanceof Error ? error.message : String(error);

		self.postMessage({
			type: 'compile-error',
			requestId: message.requestId,
			error: messageText
		});
	}
};