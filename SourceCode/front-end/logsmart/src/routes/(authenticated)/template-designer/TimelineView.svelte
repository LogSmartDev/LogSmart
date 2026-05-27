<!-- src/routes/(authenticated)/template-designer/TimelineView.svelte -->

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

<div class="timeline-container">
	{#each nodes as node (node.id)}
		<div class="timeline-node" style="margin-left: {node.depth * 12}px;">
			<button
				class="node-dot"
				class:current={node.isCurrentPath}
				onclick={() => onSelectNode?.(node.id)}
				title="Generation {node.depth}"
			></button>
			{#if node.hasChildren}
				<div class="node-connector"></div>
			{/if}
		</div>
	{/each}
</div>

<style>
	.timeline-container {
		display: flex;
		flex-direction: column;
		gap: 8px;
		align-items: flex-start;
	}

	.timeline-node {
		position: relative;
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.node-dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: #ddd;
		border: 2px solid #999;
		cursor: pointer;
		transition: all 0.2s;
		padding: 0;
	}

	.node-dot:hover {
		background: #bbb;
		transform: scale(1.2);
	}

	.node-dot.current {
		background: #1976d2;
		border-color: #1565c0;
		box-shadow: 0 0 4px rgba(25, 118, 210, 0.5);
	}

	.node-connector {
		width: 2px;
		height: 16px;
		background: #ddd;
		position: absolute;
		left: 5px;
		top: 12px;
	}
</style>
