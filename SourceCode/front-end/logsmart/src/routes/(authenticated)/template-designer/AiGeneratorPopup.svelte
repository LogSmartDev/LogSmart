<!-- src/routes/(authenticated)/template-designer/AiGeneratorPopup.svelte -->

<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import type { GeneratorState, GenerationNode, CanvasItem } from './AiGeneratorPopup.types';
	import { getChatHistory, findNodeById, isLeafNode } from './aiGeneratorStore';
	import TimelineView from './TimelineView.svelte';

	interface Props {
		generatorState: GeneratorState;
		onGenerate?: (prompt: string) => void;
		onBranch?: (nodeId: string) => void;
		onRevert?: (nodeId: string) => void;
		onMinimize?: () => void;
		onClose?: () => void;
		onPositionChange?: (position: { x: number; y: number }) => void;
	}

	let {
		generatorState = $bindable(),
		onGenerate,
		onBranch,
		onRevert,
		onMinimize,
		onClose,
		onPositionChange
	}: Props = $props();

	let prompt = $state('');
	let isDragging = $state(false);
	let dragStart = $state({ x: 0, y: 0 });
	let chatContainer: HTMLElement | null = $state(null);

	const dispatch = createEventDispatcher();

	function handleMouseDown(e: MouseEvent) {
		if ((e.target as HTMLElement).closest('[data-no-drag]')) return;
		isDragging = true;
		dragStart = {
			x: e.clientX - generatorState.position.x,
			y: e.clientY - generatorState.position.y
		};
	}

	function handleMouseMove(e: MouseEvent) {
		if (!isDragging) return;
		const newPos = {
			x: e.clientX - dragStart.x,
			y: e.clientY - dragStart.y
		};
		generatorState.position = newPos;
		onPositionChange?.(newPos);
	}

	function handleMouseUp() {
		isDragging = false;
	}

	function handleGenerate() {
		if (prompt.trim()) {
			onGenerate?.(prompt);
			prompt = '';
		}
	}

	function handleBranch() {
		if (generatorState.currentNodeId) {
			onBranch?.(generatorState.currentNodeId);
		}
	}

	function handleRevert() {
		if (generatorState.currentNodeId) {
			onRevert?.(generatorState.currentNodeId);
		}
	}

	function scrollToBottom() {
		if (chatContainer) {
			setTimeout(() => {
				chatContainer!.scrollTop = chatContainer!.scrollHeight;
			}, 0);
		}
	}

	$effect(() => {
		if (generatorState.isOpen && !generatorState.isMinimized) {
			scrollToBottom();
		}
	});

	const chatHistory = $derived(
		getChatHistory(generatorState.tree, generatorState.currentNodeId ?? '')
	);
	const currentNode = $derived(
		generatorState.tree && generatorState.currentNodeId
			? findNodeById(generatorState.tree, generatorState.currentNodeId)
			: null
	);
	const canBranch = $derived(
		currentNode ? !isLeafNode(generatorState.tree, generatorState.currentNodeId!) : false
	);
	const isNotCurrent = $derived(
		generatorState.tree && generatorState.currentNodeId
			? chatHistory[chatHistory.length - 1]?.id !== generatorState.currentNodeId
			: false
	);
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} />

<div
	class="popup-container"
	style="left: {generatorState.position.x}px; top: {generatorState.position
		.y}px; display: {generatorState.isOpen && !generatorState.isMinimized ? 'flex' : 'none'};"
>
	<!-- Header -->
	<div
		class="header"
		role="button"
		tabindex="0"
		onmousedown={handleMouseDown}
		onkeydown={(e) => e.key === 'Enter' && handleMouseDown(e as any)}
	>
		<h2>AI Generator</h2>
		<div class="header-buttons" data-no-drag>
			<button class="header-btn" title="Minimize" onclick={onMinimize}> − </button>
			<button class="header-btn close" title="Close" onclick={onClose}> × </button>
		</div>
	</div>

	<div class="content">
		<!-- Timeline (left) -->
		<div class="timeline">
			<TimelineView
				tree={generatorState.tree}
				currentNodeId={generatorState.currentNodeId}
				onSelectNode={(nodeId) => dispatch('selectNode', nodeId)}
			/>
		</div>

		<!-- Chat (right) -->
		<div class="chat-area">
			<div class="chat-messages" bind:this={chatContainer}>
				{#each chatHistory as node (node.id)}
					<div class="message user-message">
						<div class="message-label">You</div>
						<div class="message-content">{node.prompt}</div>
						<div class="message-time">{new Date(node.timestamp).toLocaleTimeString()}</div>
					</div>
					<div class="message ai-message">
						<div class="message-label">AI Generator</div>
						<div class="message-content">
							Generated {node.response.length} component{node.response.length !== 1 ? 's' : ''}
						</div>
						<div class="message-time">{new Date(node.timestamp).toLocaleTimeString()}</div>
					</div>
				{/each}
			</div>

			<!-- Input -->
			<div class="input-area" data-no-drag>
				<textarea
					class="prompt-input"
					placeholder="Describe what you want to generate..."
					bind:value={prompt}
					onkeydown={(e) => e.key === 'Enter' && e.ctrlKey && handleGenerate()}
				></textarea>
				<div class="button-group">
					<button class="btn-primary" onclick={handleGenerate} disabled={!prompt.trim()}>
						Generate
					</button>
					{#if canBranch}
						<button class="btn-secondary" onclick={handleBranch}> Branch from here </button>
					{/if}
					{#if isNotCurrent}
						<button class="btn-secondary" onclick={handleRevert}> Revert to this state </button>
					{/if}
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.popup-container {
		position: fixed;
		width: 500px;
		height: 600px;
		background: white;
		border: 1px solid #ccc;
		border-radius: 8px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
		display: flex;
		flex-direction: column;
		z-index: 1000;
		font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
	}

	.header {
		padding: 12px 16px;
		border-bottom: 1px solid #eee;
		display: flex;
		justify-content: space-between;
		align-items: center;
		cursor: move;
		user-select: none;
		background: #f9f9f9;
	}

	.header h2 {
		margin: 0;
		font-size: 14px;
		font-weight: 600;
		color: #333;
	}

	.header-buttons {
		display: flex;
		gap: 4px;
	}

	.header-btn {
		width: 28px;
		height: 28px;
		border: none;
		background: transparent;
		cursor: pointer;
		font-size: 18px;
		color: #666;
		border-radius: 4px;
		transition: background 0.2s;
	}

	.header-btn:hover {
		background: #e0e0e0;
	}

	.header-btn.close:hover {
		background: #ff4444;
		color: white;
	}

	.content {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	.timeline {
		width: 80px;
		border-right: 1px solid #eee;
		overflow-y: auto;
		padding: 12px;
		background: #fafafa;
	}

	.chat-area {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.chat-messages {
		flex: 1;
		overflow-y: auto;
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.message {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.user-message .message-content {
		background: #e3f2fd;
		color: #1976d2;
		padding: 8px 12px;
		border-radius: 6px;
		word-wrap: break-word;
	}

	.ai-message .message-content {
		background: #f5f5f5;
		color: #333;
		padding: 8px 12px;
		border-radius: 6px;
	}

	.message-label {
		font-size: 12px;
		font-weight: 600;
		color: #666;
	}

	.message-time {
		font-size: 11px;
		color: #999;
	}

	.input-area {
		padding: 12px;
		border-top: 1px solid #eee;
		background: #fafafa;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.prompt-input {
		width: 100%;
		min-height: 60px;
		padding: 8px;
		border: 1px solid #ddd;
		border-radius: 4px;
		font-family: inherit;
		font-size: 14px;
		resize: none;
	}

	.prompt-input:focus {
		outline: none;
		border-color: #1976d2;
		box-shadow: 0 0 0 2px rgba(25, 118, 210, 0.1);
	}

	.button-group {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}

	.btn-primary,
	.btn-secondary {
		padding: 8px 12px;
		border: none;
		border-radius: 4px;
		font-size: 13px;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.2s;
	}

	.btn-primary {
		background: #1976d2;
		color: white;
		flex: 1;
		min-width: 100px;
	}

	.btn-primary:hover:not(:disabled) {
		background: #1565c0;
	}

	.btn-primary:disabled {
		background: #ccc;
		cursor: not-allowed;
	}

	.btn-secondary {
		background: #e0e0e0;
		color: #333;
		flex: 1;
		min-width: 100px;
	}

	.btn-secondary:hover {
		background: #d0d0d0;
	}
</style>
