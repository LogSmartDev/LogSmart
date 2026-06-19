<script lang="ts">
	import type { GenerationNode } from './AiGeneratorPopup.types';

	interface Props {
		tree: GenerationNode | null;
		currentNodeId: string | null;
		onSelectNode?: (nodeId: string) => void;
	}

	let { tree, currentNodeId, onSelectNode }: Props = $props();

	function renderNode(node: GenerationNode | null, depth: number = 0): any[] {
		if (!node) return [];

		const isCurrentPath = node.id === currentNodeId;
		const items = [
			{
				id: node.id,
				depth,
				isCurrentPath,
				hasChildren: node.children.length > 0
			}
		];

		for (const child of node.children) {
			items.push(...renderNode(child, depth + 1));
		}

		return items;
	}

	const nodes = $derived(renderNode(tree));
</script>

<div class="timeline">
	{#each nodes as node (node.id)}
		<div class="tl-node" style="margin-left: {node.depth * 10}px;">
			{#if node.hasChildren}
				<div class="tl-connector"></div>
			{/if}
			<button
				class="tl-dot"
				class:tl-dot-current={node.isCurrentPath}
				onclick={() => onSelectNode?.(node.id)}
				title="Generation {node.depth}"
			></button>
		</div>
	{/each}
</div>

<style>
	.timeline {
		display: flex;
		flex-direction: column;
		gap: 4px;
		align-items: center;
	}

	.tl-node {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
	}

	.tl-dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--border-secondary);
		border: 2px solid var(--text-secondary);
		cursor: pointer;
		transition: all 0.15s ease;
		padding: 0;
		position: relative;
		z-index: 1;
	}

	.tl-dot:hover {
		transform: scale(1.3);
		border-color: var(--button-primary);
	}

	.tl-dot-current {
		background: var(--button-primary);
		border-color: var(--button-primary);
		box-shadow: 0 0 0 2px color-mix(in srgb, var(--button-primary) 30%, transparent);
	}

	.tl-connector {
		width: 2px;
		height: 14px;
		background: var(--border-secondary);
		margin-bottom: 2px;
	}
</style>
