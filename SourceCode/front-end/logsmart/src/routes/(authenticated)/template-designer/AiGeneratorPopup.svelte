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
		isLoading?: boolean;
	}

	let {
		generatorState = $bindable(),
		onGenerate,
		onBranch,
		onRevert,
		onMinimize,
		onClose,
		onPositionChange,
		isLoading = false
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
	<div
		class="header"
		role="button"
		tabindex="0"
		onmousedown={handleMouseDown}
		onkeydown={(e) => e.key === 'Enter' && handleMouseDown(e as any)}
	>
		<h2>AI Generator</h2>
		<div class="header-buttons" data-no-drag>
			<button class="header-btn" title="Minimize" onclick={onMinimize}>─</button>
			<button class="header-btn close" title="Close" onclick={onClose}>×</button>
		</div>
	</div>

	<div class="content">
		<div class="timeline-panel">
			<div class="timeline-header">History</div>
			<TimelineView
				tree={generatorState.tree}
				currentNodeId={generatorState.currentNodeId}
				onSelectNode={(nodeId) => dispatch('selectNode', nodeId)}
			/>
		</div>

		<div class="chat-area">
			<div class="chat-messages" bind:this={chatContainer}>
				{#each chatHistory as node (node.id)}
					<div class="message-group">
						<div class="message user-message">
							<div class="msg-label">You</div>
							<div class="msg-bubble msg-bubble-user">{node.prompt}</div>
							<div class="msg-time">{new Date(node.timestamp).toLocaleTimeString()}</div>
						</div>
						<div class="message ai-message">
							<div class="msg-label">AI Generator</div>
							<div class="msg-bubble msg-bubble-ai">
								{#if node.response.length === 0}
									<span class="generating-indicator">
										<span class="dot-pulse"></span>
										Generating...
									</span>
								{:else}
									Generated {node.response.length} component{node.response.length !== 1 ? 's' : ''}
								{/if}
							</div>
							<div class="msg-time">{new Date(node.timestamp).toLocaleTimeString()}</div>
						</div>
					</div>
				{/each}
			</div>

			<div class="input-area" data-no-drag>
				<textarea
					class="prompt-input"
					placeholder="Describe what you want to generate..."
					bind:value={prompt}
					onkeydown={(e) => e.key === 'Enter' && e.ctrlKey && handleGenerate()}
					disabled={isLoading}
				></textarea>
				<div class="button-row">
					<button
						class="btn btn-primary"
						onclick={handleGenerate}
						disabled={!prompt.trim() || isLoading}
					>
						{isLoading ? 'Generating...' : 'Generate'}
					</button>
					{#if canBranch}
						<button class="btn btn-outline" onclick={handleBranch} disabled={isLoading}>
							Branch
						</button>
					{/if}
					{#if isNotCurrent}
						<button class="btn btn-outline" onclick={handleRevert} disabled={isLoading}>
							Revert
						</button>
					{/if}
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.popup-container {
		position: fixed;
		width: 480px;
		height: 560px;
		background: var(--bg-primary);
		border: 2px solid var(--border-primary);
		border-radius: 8px;
		box-shadow: 0 6px 20px rgba(0, 0, 0, 0.12);
		display: flex;
		flex-direction: column;
		z-index: 1000;
		overflow: hidden;
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 12px 16px;
		border-bottom: 2px solid var(--border-secondary);
		background: var(--bg-primary);
		cursor: move;
		user-select: none;
		flex-shrink: 0;
	}

	.header h2 {
		margin: 0;
		font-size: 14px;
		font-weight: 700;
		color: var(--text-primary);
		letter-spacing: 0.01em;
	}

	.header-buttons {
		display: flex;
		gap: 4px;
	}

	.header-btn {
		width: 28px;
		height: 28px;
		border: 2px solid transparent;
		background: transparent;
		cursor: pointer;
		font-size: 16px;
		line-height: 1;
		color: var(--text-secondary);
		border-radius: 4px;
		transition: all 0.15s ease;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.header-btn:hover {
		background: var(--bg-secondary);
		border-color: var(--border-secondary);
		color: var(--text-primary);
	}

	.header-btn.close:hover {
		background: var(--error-bg);
		border-color: var(--error);
		color: var(--error);
	}

	.content {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	.timeline-panel {
		width: 72px;
		border-right: 2px solid var(--border-secondary);
		overflow-y: auto;
		padding: 12px 8px;
		background: var(--bg-secondary);
		flex-shrink: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.timeline-header {
		font-size: 10px;
		font-weight: 700;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.08em;
		text-align: center;
	}

	.chat-area {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-width: 0;
	}

	.chat-messages {
		flex: 1;
		overflow-y: auto;
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.message-group {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.message {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}

	.msg-label {
		font-size: 11px;
		font-weight: 600;
		color: var(--text-secondary);
	}

	.msg-bubble {
		padding: 8px 12px;
		border-radius: 6px;
		font-size: 13px;
		line-height: 1.45;
		word-wrap: break-word;
	}

	.msg-bubble-user {
		background: var(--button-primary);
		color: var(--button-text);
		align-self: flex-start;
	}

	.msg-bubble-ai {
		background: var(--bg-secondary);
		border: 2px solid var(--border-secondary);
		color: var(--text-primary);
	}

	.msg-time {
		font-size: 10px;
		color: var(--text-secondary);
	}

	.generating-indicator {
		display: inline-flex;
		align-items: center;
		gap: 8px;
	}

	.dot-pulse {
		display: inline-block;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--button-primary);
		animation: pulse 1.2s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
			transform: scale(1);
		}
		50% {
			opacity: 0.4;
			transform: scale(0.7);
		}
	}

	.input-area {
		padding: 12px;
		border-top: 2px solid var(--border-secondary);
		background: var(--bg-secondary);
		display: flex;
		flex-direction: column;
		gap: 8px;
		flex-shrink: 0;
	}

	.prompt-input {
		width: 100%;
		min-height: 56px;
		padding: 8px 10px;
		border: 2px solid var(--border-primary);
		border-radius: 4px;
		background: var(--bg-primary);
		color: var(--text-primary);
		font-family: inherit;
		font-size: 13px;
		line-height: 1.4;
		resize: none;
		box-sizing: border-box;
	}

	.prompt-input:focus {
		outline: none;
		border-color: var(--input-focus);
		box-shadow: 0 0 0 2px color-mix(in srgb, var(--input-focus) 20%, transparent);
	}

	.prompt-input:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.prompt-input::placeholder {
		color: var(--text-secondary);
		opacity: 0.7;
	}

	.button-row {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
	}

	.btn {
		padding: 6px 12px;
		border: 2px solid transparent;
		border-radius: 4px;
		font-size: 13px;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.12s ease;
		flex: 1;
		min-width: 80px;
		text-align: center;
	}

	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-primary {
		background: var(--button-primary);
		color: var(--button-text);
		border-color: var(--button-primary);
	}

	.btn-primary:hover:not(:disabled) {
		background: var(--button-primary-hover);
		border-color: var(--button-primary-hover);
	}

	.btn-primary:active:not(:disabled) {
		background: var(--button-primary-active);
		border-color: var(--button-primary-active);
	}

	.btn-outline {
		background: var(--bg-primary);
		color: var(--text-primary);
		border-color: var(--border-primary);
	}

	.btn-outline:hover:not(:disabled) {
		background: var(--bg-secondary);
		opacity: 0.8;
	}
</style>
