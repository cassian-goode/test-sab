<script>
	import { onMount } from 'svelte';
	import { EditorState, Compartment } from '@codemirror/state';
	import { EditorView } from '@codemirror/view';
	import { basicSetup } from 'codemirror';

	let {
		value = $bindable(''),
		extensions = [],
		onChange = undefined
	} = $props();

	let host = $state();
	let view = $state(null);

	const extensionsCompartment = new Compartment();

	onMount(() => {
		view = new EditorView({
			state: EditorState.create({
				doc: value,
				extensions: [
					basicSetup,
					extensionsCompartment.of(extensions),
					EditorView.updateListener.of((update) => {
						if (!update.docChanged) return;

						const nextValue = update.state.doc.toString();

						if (nextValue !== value) {
							value = nextValue;
							if (onChange) onChange(nextValue, update.view);
						}
					})
				]
			}),
			parent: host
		});

		return () => {
			if (view) view.destroy();
		};
	});

	$effect(() => {
		if (!view) return;

		view.dispatch({
			effects: extensionsCompartment.reconfigure(extensions)
		});
	});

	$effect(() => {
		if (!view) return;

		const current = view.state.doc.toString();

		if (value !== current) {
			view.dispatch({
				changes: {
					from: 0,
					to: current.length,
					insert: value
				}
			});
		}
	});
</script>

<div bind:this={host} class="cm-host"></div>

<style>
	.cm-host {
		height: 100%;
	}

	.cm-host :global(.cm-editor) {
		height: 100%;
		border: 1px solid #ddd;
		border-radius: 8px;
		background: white;
	}

	.cm-host :global(.cm-scroller) {
		overflow: auto;
		min-height: 100%;
		font-family: monospace;
	}

	.cm-host :global(.cm-content),
	.cm-host :global(.cm-gutter) {
		padding-top: 0.75rem;
		padding-bottom: 0.75rem;
	}
</style>