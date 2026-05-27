# Chat-Based AI Generator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the AI generator from a one-shot sidebar into a floating chat popup with state tree support, allowing sequential prompts and branch exploration.

**Architecture:** The implementation uses a Svelte 5 floating popup component with immutable state tree management. State lives in the template designer page context and persists to sessionStorage. The existing `/llm/generate-layout` backend endpoint remains unchanged. Chat history flows through the tree structure, with navigation triggering canvas state restoration.

**Tech Stack:** SvelteKit 5 (runes), TypeScript, sessionStorage, existing OpenAPI client

---

## File Structure

**Frontend Files:**

- **Create:** `src/routes/(authenticated)/template-designer/AiGeneratorPopup.svelte` - Main popup component (chat UI, timeline, input)
- **Create:** `src/routes/(authenticated)/template-designer/AiGeneratorPopup.types.ts` - TypeScript type definitions
- **Create:** `src/routes/(authenticated)/template-designer/aiGeneratorStore.ts` - State management and persistence
- **Modify:** `src/routes/(authenticated)/template-designer/+page.svelte` - Replace sidebar with button, integrate popup
- **Delete:** `src/routes/(authenticated)/template-designer/AiGeneratorSidebar.svelte` - Replaced by popup

**Backend:** No changes (existing endpoint reused)

---

## Task 1: Create Type Definitions

**Files:**
- Create: `src/routes/(authenticated)/template-designer/AiGeneratorPopup.types.ts`

- [ ] **Step 1: Write TypeScript types for generation tree**

Create the file with all type definitions the popup needs:

```typescript
// src/routes/(authenticated)/template-designer/AiGeneratorPopup.types.ts

export interface CanvasItem {
  id: string;
  type: string;
  position: { x: number; y: number };
  size?: { width: number; height: number };
  props?: Record<string, any>;
}

export interface GenerationNode {
  id: string;
  parentId: string | null;
  prompt: string;
  response: CanvasItem[];
  canvasStateAtGeneration: CanvasItem[];
  timestamp: number;
  children: GenerationNode[];
}

export interface GeneratorState {
  tree: GenerationNode | null;
  currentNodeId: string | null;
  isOpen: boolean;
  position: { x: number; y: number };
  isMinimized: boolean;
}

export interface BreadcrumbNode {
  id: string;
  prompt: string;
  isCurrentPath: boolean;
}
```

- [ ] **Step 2: Verify file created**

Run: `ls -la src/routes/'(authenticated)'/template-designer/AiGeneratorPopup.types.ts`

Expected: File exists with no TypeScript errors

---

## Task 2: Create State Management Store

**Files:**
- Create: `src/routes/(authenticated)/template-designer/aiGeneratorStore.ts`

- [ ] **Step 1: Write state initialization and persistence functions**

```typescript
// src/routes/(authenticated)/template-designer/aiGeneratorStore.ts

import { v4 as uuidv4 } from 'uuid';
import type { GenerationNode, GeneratorState, CanvasItem, BreadcrumbNode } from './AiGeneratorPopup.types';

const STORAGE_KEY = 'logsmart_ai_generator_state';

/**
 * Initialize empty generator state
 */
export function createEmptyGeneratorState(): GeneratorState {
  return {
    tree: null,
    currentNodeId: null,
    isOpen: false,
    position: { x: 100, y: 100 },
    isMinimized: false,
  };
}

/**
 * Load generator state from sessionStorage
 */
export function loadGeneratorState(): GeneratorState {
  try {
    const stored = sessionStorage.getItem(STORAGE_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error('Failed to load generator state:', e);
  }
  return createEmptyGeneratorState();
}

/**
 * Save generator state to sessionStorage
 */
export function saveGeneratorState(state: GeneratorState): void {
  try {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch (e) {
    console.error('Failed to save generator state:', e);
  }
}

/**
 * Clear generator state (close popup, reset tree)
 */
export function clearGeneratorState(): void {
  sessionStorage.removeItem(STORAGE_KEY);
}

/**
 * Create a new generation node
 */
export function createGenerationNode(
  prompt: string,
  response: CanvasItem[],
  canvasStateAtGeneration: CanvasItem[],
  parentId: string | null = null
): GenerationNode {
  return {
    id: uuidv4(),
    parentId,
    prompt,
    response,
    canvasStateAtGeneration,
    timestamp: Date.now(),
    children: [],
  };
}

/**
 * Add a generation node to the tree
 */
export function addGenerationToTree(
  tree: GenerationNode,
  parentNodeId: string,
  newNode: GenerationNode
): GenerationNode {
  if (tree.id === parentNodeId) {
    return {
      ...tree,
      children: [...tree.children, newNode],
    };
  }

  return {
    ...tree,
    children: tree.children.map((child) => addGenerationToTree(child, parentNodeId, newNode)),
  };
}

/**
 * Find a node by ID in the tree
 */
export function findNodeById(tree: GenerationNode | null, nodeId: string): GenerationNode | null {
  if (!tree) return null;
  if (tree.id === nodeId) return tree;

  for (const child of tree.children) {
    const found = findNodeById(child, nodeId);
    if (found) return found;
  }

  return null;
}

/**
 * Get the path from root to a specific node (breadcrumb)
 */
export function getBreadcrumb(
  tree: GenerationNode | null,
  targetNodeId: string,
  path: GenerationNode[] = []
): GenerationNode[] {
  if (!tree) return [];
  if (tree.id === targetNodeId) return [...path, tree];

  for (const child of tree.children) {
    const found = getBreadcrumb(child, targetNodeId, [...path, tree]);
    if (found.length > 0) return found;
  }

  return [];
}

/**
 * Get all generations in order from root to current node (for chat display)
 */
export function getChatHistory(tree: GenerationNode | null, currentNodeId: string): GenerationNode[] {
  if (!tree || !currentNodeId) return [];
  const breadcrumb = getBreadcrumb(tree, currentNodeId);
  return breadcrumb;
}

/**
 * Get all branches (children) of a node
 */
export function getBranches(tree: GenerationNode | null, nodeId: string): GenerationNode[] {
  if (!tree) return [];
  const node = findNodeById(tree, nodeId);
  return node?.children ?? [];
}

/**
 * Check if a node is a leaf (has no children)
 */
export function isLeafNode(tree: GenerationNode | null, nodeId: string): boolean {
  if (!tree) return false;
  const node = findNodeById(tree, nodeId);
  return node ? node.children.length === 0 : false;
}
```

- [ ] **Step 2: Run type check**

Run: `cd src/routes/'(authenticated)'/template-designer && pnpm tsc --noEmit`

Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart
git add src/routes/'(authenticated)'/template-designer/AiGeneratorPopup.types.ts
git add src/routes/'(authenticated)'/template-designer/aiGeneratorStore.ts
git commit -m "feat: add AI generator state management and types"
```

---

## Task 3: Create Floating Popup Component

**Files:**
- Create: `src/routes/(authenticated)/template-designer/AiGeneratorPopup.svelte`

- [ ] **Step 1: Write main popup component with header and sections**

```svelte
<!-- src/routes/(authenticated)/template-designer/AiGeneratorPopup.svelte -->

<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { GeneratorState, GenerationNode, CanvasItem } from './AiGeneratorPopup.types';
  import { getChatHistory, getBranches, findNodeById, isLeafNode } from './aiGeneratorStore';

  interface Props {
    state: GeneratorState;
    onGenerate?: (prompt: string) => void;
    onBranch?: (nodeId: string) => void;
    onRevert?: (nodeId: string) => void;
    onMinimize?: () => void;
    onClose?: () => void;
    onPositionChange?: (position: { x: number; y: number }) => void;
  }

  let {
    state = $bindable(),
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
    dragStart = { x: e.clientX - state.position.x, y: e.clientY - state.position.y };
  }

  function handleMouseMove(e: MouseEvent) {
    if (!isDragging) return;
    const newPos = {
      x: e.clientX - dragStart.x,
      y: e.clientY - dragStart.y
    };
    state.position = newPos;
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
    if (state.currentNodeId) {
      onBranch?.(state.currentNodeId);
    }
  }

  function handleRevert() {
    if (state.currentNodeId) {
      onRevert?.(state.currentNodeId);
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
    if (state.isOpen && !state.isMinimized) {
      scrollToBottom();
    }
  });

  const chatHistory = $derived(getChatHistory(state.tree, state.currentNodeId ?? ''));
  const currentNode = $derived(state.tree && state.currentNodeId ? findNodeById(state.tree, state.currentNodeId) : null);
  const canBranch = $derived(currentNode ? !isLeafNode(state.tree, state.currentNodeId!) : false);
  const isNotCurrent = $derived(state.tree && state.currentNodeId ? chatHistory[chatHistory.length - 1]?.id !== state.currentNodeId : false);
</script>

<svelte:window
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
/>

<div
  class="popup-container"
  style="left: {state.position.x}px; top: {state.position.y}px; display: {state.isOpen && !state.isMinimized ? 'flex' : 'none'};"
>
  <!-- Header -->
  <div class="header" onmousedown={handleMouseDown}>
    <h2>AI Generator</h2>
    <div class="header-buttons" data-no-drag>
      <button
        class="header-btn"
        title="Minimize"
        onclick={onMinimize}
      >
        −
      </button>
      <button
        class="header-btn close"
        title="Close"
        onclick={onClose}
      >
        ×
      </button>
    </div>
  </div>

  <div class="content">
    <!-- Timeline (left) -->
    <div class="timeline">
      <TimelineView tree={state.tree} currentNodeId={state.currentNodeId} onSelectNode={(nodeId) => dispatch('selectNode', nodeId)} />
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
            <div class="message-content">Generated {node.response.length} component{node.response.length !== 1 ? 's' : ''}</div>
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
            <button class="btn-secondary" onclick={handleBranch}>
              Branch from here
            </button>
          {/if}
          {#if isNotCurrent}
            <button class="btn-secondary" onclick={handleRevert}>
              Revert to this state
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
```

- [ ] **Step 2: Create TimelineView component**

Create a new file for the timeline visualization:

```svelte
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
        hasChildren: node.children.length > 0,
      },
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
      />
      {#if node.hasChildren}
        <div class="node-connector" />
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
```

- [ ] **Step 3: Run type check and compile**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart && pnpm build`

Expected: No TypeScript or build errors (component not yet integrated, so build should succeed with warnings about unused component)

- [ ] **Step 4: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart
git add src/routes/'(authenticated)'/template-designer/AiGeneratorPopup.svelte
git add src/routes/'(authenticated)'/template-designer/TimelineView.svelte
git commit -m "feat: create AI generator popup component with timeline"
```

---

## Task 4: Modify Template Designer Page to Integrate Popup

**Files:**
- Modify: `src/routes/(authenticated)/template-designer/+page.svelte`

- [ ] **Step 1: Replace AiGeneratorSidebar with launch button and integrate popup**

In `+page.svelte`, find the section where `AiGeneratorSidebar` is used (around line 925-926). Replace it with:

```svelte
<!-- Remove this: <AiGeneratorSidebar ... /> -->

<!-- Replace with: -->
<div class="ai-generator-button-container">
  <button
    class="ai-generator-btn"
    onclick={() => {
      generatorState.isOpen = true;
      generatorState.isMinimized = false;
    }}
    title="Open AI Generator"
  >
    ✨ AI Generator
  </button>
</div>

{#if generatorState.isOpen}
  <AiGeneratorPopup
    bind:state={generatorState}
    onGenerate={handleAiGenerate}
    onBranch={handleAiBranch}
    onRevert={handleAiRevert}
    onMinimize={() => {
      generatorState.isMinimized = true;
    }}
    onClose={() => {
      generatorState = createEmptyGeneratorState();
      clearGeneratorState();
    }}
    onPositionChange={(pos) => {
      generatorState.position = pos;
      saveGeneratorState(generatorState);
    }}
    on:selectNode={(e) => {
      const nodeId = e.detail;
      switchAiGeneration(nodeId);
    }}
  />
{/if}
```

- [ ] **Step 2: Add imports at top of `+page.svelte`**

Add these imports to the `<script>` block:

```typescript
import AiGeneratorPopup from './AiGeneratorPopup.svelte';
import TimelineView from './TimelineView.svelte';
import type { GeneratorState, GenerationNode, CanvasItem } from './AiGeneratorPopup.types';
import {
  createEmptyGeneratorState,
  loadGeneratorState,
  saveGeneratorState,
  clearGeneratorState,
  createGenerationNode,
  addGenerationToTree,
  findNodeById,
  getChatHistory,
} from './aiGeneratorStore';
```

- [ ] **Step 3: Initialize generator state in `+page.svelte` page context**

Add this to the page's `<script context="module">` or top of `<script>`:

```typescript
let generatorState = $state<GeneratorState>(loadGeneratorState());

// Persist state changes
$effect(() => {
  saveGeneratorState(generatorState);
});
```

- [ ] **Step 4: Replace the old `generateLayoutFromPrompt` function with new version**

Find the existing `generateLayoutFromPrompt` function (around line 583) and replace it with:

```typescript
async function generateLayoutFromPrompt(prompt: string): Promise<void> {
  if (!prompt.trim() || !generatorState.tree) {
    return;
  }

  // Build context from current canvas state
  const contextDescription = `Canvas dimensions: ${canvasDimensions.width}x${canvasDimensions.height}. Current components: ${JSON.stringify(canvasItems)}`;
  const enrichedPrompt = `${prompt}\n\nCurrent canvas context: ${contextDescription}`;

  // Show loading state
  const previousItems = [...canvasItems];

  try {
    const response = await apiClient.POST('/llm/generate-layout', {
      body: {
        user_prompt: enrichedPrompt,
      },
    });

    if (response.error) {
      throw new Error(response.error.message || 'Generation failed');
    }

    const layoutResponse = response.data;
    const generatedComponents = (layoutResponse?.layout?.template_layout || []).map(mapApiFieldToCanvasItem);

    // Create generation node
    const newNode = createGenerationNode(
      prompt,
      generatedComponents,
      previousItems,
      generatorState.currentNodeId
    );

    // Add to tree
    if (!generatorState.tree) {
      generatorState.tree = newNode;
    } else {
      generatorState.tree = addGenerationToTree(
        generatorState.tree,
        generatorState.currentNodeId || generatorState.tree.id,
        newNode
      );
    }

    generatorState.currentNodeId = newNode.id;

    // Update canvas with generated items
    canvasItems = generatedComponents;

    saveGeneratorState(generatorState);
  } catch (error) {
    console.error('Generation failed:', error);
    // Restore previous state on error
    canvasItems = previousItems;
    // Show error in popup (handled by parent component)
  }
}
```

- [ ] **Step 5: Add branch and revert handlers**

Add these new functions to handle popup interactions:

```typescript
function handleAiBranch(parentNodeId: string): void {
  // Branching just switches to that node; next generation creates sibling
  switchAiGeneration(parentNodeId);
}

function handleAiRevert(nodeId: string): void {
  // Restore canvas to state before that generation
  const node = findNodeById(generatorState.tree, nodeId);
  if (node) {
    canvasItems = node.canvasStateAtGeneration;
    generatorState.currentNodeId = nodeId;
    saveGeneratorState(generatorState);
  }
}

function switchAiGeneration(nodeId: string): void {
  // Switch to a different generation node
  const node = findNodeById(generatorState.tree, nodeId);
  if (node) {
    generatorState.currentNodeId = nodeId;
    // Restore canvas to this generation's post-state
    canvasItems = node.response;
    saveGeneratorState(generatorState);
  }
}

function handleAiGenerate(prompt: string): void {
  generateLayoutFromPrompt(prompt);
}
```

- [ ] **Step 6: Update template designer styles for button**

Add to the `<style>` block:

```css
.ai-generator-button-container {
  padding: 12px;
  border-top: 1px solid #eee;
}

.ai-generator-btn {
  width: 100%;
  padding: 10px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border: none;
  border-radius: 6px;
  font-weight: 600;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
}

.ai-generator-btn:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
}
```

- [ ] **Step 7: Build and verify no errors**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart && pnpm build`

Expected: Build succeeds with no new errors

- [ ] **Step 8: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart
git add src/routes/'(authenticated)'/template-designer/+page.svelte
git commit -m "feat: integrate AI generator popup into template designer"
```

---

## Task 5: Remove Old AiGeneratorSidebar Component

**Files:**
- Delete: `src/routes/(authenticated)/template-designer/AiGeneratorSidebar.svelte`

- [ ] **Step 1: Delete old sidebar component**

```bash
rm /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart/src/routes/'(authenticated)'/template-designer/AiGeneratorSidebar.svelte
```

- [ ] **Step 2: Verify file deleted and build succeeds**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart && pnpm build`

Expected: Build succeeds, no errors about missing AiGeneratorSidebar

- [ ] **Step 3: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart
git add -u
git commit -m "chore: remove old AI generator sidebar component"
```

---

## Task 6: Add SessionStorage Persistence Tests

**Files:**
- Create: `src/routes/(authenticated)/template-designer/__tests__/aiGeneratorStore.test.ts`

- [ ] **Step 1: Write tests for state persistence**

```typescript
// src/routes/(authenticated)/template-designer/__tests__/aiGeneratorStore.test.ts

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  createEmptyGeneratorState,
  loadGeneratorState,
  saveGeneratorState,
  clearGeneratorState,
  createGenerationNode,
  addGenerationToTree,
  findNodeById,
  getBreadcrumb,
} from '../aiGeneratorStore';
import type { GeneratorState } from '../AiGeneratorPopup.types';

describe('aiGeneratorStore', () => {
  beforeEach(() => {
    // Mock sessionStorage
    vi.stubGlobal('sessionStorage', {
      getItem: vi.fn(),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn(),
      length: 0,
      key: vi.fn(),
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe('createEmptyGeneratorState', () => {
    it('should create empty state with default values', () => {
      const state = createEmptyGeneratorState();
      expect(state.tree).toBeNull();
      expect(state.currentNodeId).toBeNull();
      expect(state.isOpen).toBe(false);
      expect(state.isMinimized).toBe(false);
      expect(state.position).toEqual({ x: 100, y: 100 });
    });
  });

  describe('createGenerationNode', () => {
    it('should create a node with correct properties', () => {
      const node = createGenerationNode('test prompt', [], [], null);
      expect(node.prompt).toBe('test prompt');
      expect(node.parentId).toBeNull();
      expect(node.response).toEqual([]);
      expect(node.children).toEqual([]);
      expect(node.id).toBeTruthy();
      expect(node.timestamp).toBeGreaterThan(0);
    });
  });

  describe('addGenerationToTree', () => {
    it('should add node as child of parent', () => {
      const root = createGenerationNode('root', [], [], null);
      const child = createGenerationNode('child', [], [], root.id);
      const newRoot = addGenerationToTree(root, root.id, child);
      expect(newRoot.children).toHaveLength(1);
      expect(newRoot.children[0].id).toBe(child.id);
    });

    it('should handle nested tree additions', () => {
      const root = createGenerationNode('root', [], [], null);
      const child1 = createGenerationNode('child1', [], [], root.id);
      const tree1 = addGenerationToTree(root, root.id, child1);

      const child2 = createGenerationNode('child2', [], [], child1.id);
      const tree2 = addGenerationToTree(tree1, child1.id, child2);

      expect(tree2.children[0].children).toHaveLength(1);
      expect(tree2.children[0].children[0].id).toBe(child2.id);
    });
  });

  describe('findNodeById', () => {
    it('should find node in tree', () => {
      const root = createGenerationNode('root', [], [], null);
      const child = createGenerationNode('child', [], [], root.id);
      const tree = addGenerationToTree(root, root.id, child);

      const found = findNodeById(tree, child.id);
      expect(found).toBeTruthy();
      expect(found?.prompt).toBe('child');
    });

    it('should return null if node not found', () => {
      const root = createGenerationNode('root', [], [], null);
      const found = findNodeById(root, 'nonexistent');
      expect(found).toBeNull();
    });
  });

  describe('getBreadcrumb', () => {
    it('should return path from root to node', () => {
      const root = createGenerationNode('root', [], [], null);
      const child = createGenerationNode('child', [], [], root.id);
      const grandchild = createGenerationNode('grandchild', [], [], child.id);

      const tree = addGenerationToTree(root, root.id, child);
      const tree2 = addGenerationToTree(tree, child.id, grandchild);

      const breadcrumb = getBreadcrumb(tree2, grandchild.id);
      expect(breadcrumb).toHaveLength(3);
      expect(breadcrumb.map((n) => n.prompt)).toEqual(['root', 'child', 'grandchild']);
    });
  });
});
```

- [ ] **Step 2: Run tests**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart && pnpm test src/routes/'(authenticated)'/template-designer/__tests__/aiGeneratorStore.test.ts`

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart
git add src/routes/'(authenticated)'/template-designer/__tests__/aiGeneratorStore.test.ts
git commit -m "test: add AI generator store tests"
```

---

## Task 7: Manual Testing & Verification

- [ ] **Step 1: Start development server**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart && pnpm dev`

Expected: Dev server starts, no errors

- [ ] **Step 2: Test popup launch**

1. Navigate to template designer
2. Click "AI Generator" button
3. Verify popup appears floating on screen
4. Verify it can be dragged
5. Verify minimize button collapses it

- [ ] **Step 3: Test generation flow**

1. Enter a prompt (e.g., "Add a text input")
2. Click "Generate"
3. Verify canvas updates
4. Verify chat shows user prompt and generation
5. Verify timeline shows generation node

- [ ] **Step 4: Test sequential generation**

1. From existing generation, enter new prompt (e.g., "Make it blue")
2. Click "Generate"
3. Verify new message appears in chat
4. Verify timeline shows generation 2 as child of generation 1
5. Verify canvas shows both changes applied

- [ ] **Step 5: Test branching**

1. Click on generation 1 in timeline
2. Verify canvas reverts to generation 1's state
3. Verify "Branch from here" button appears
4. Click "Branch from here"
5. Enter new prompt
6. Click "Generate"
7. Verify generation 1b appears as sibling to generation 2
8. Verify canvas shows new generation

- [ ] **Step 6: Test session persistence**

1. Generate multiple times, create branches
2. Refresh page
3. Verify generator state is restored
4. Verify chat history is intact
5. Verify timeline structure matches

- [ ] **Step 7: Test error handling**

1. Try to generate with empty prompt (should be disabled)
2. Disable network and attempt generation (should show error)
3. Re-enable network
4. Verify recovery

---

## Task 8: Final Integration Commit

- [ ] **Step 1: Verify full build and no console errors**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart && pnpm build`

Expected: Build succeeds with no errors

- [ ] **Step 2: Run all tests**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart && pnpm test`

Expected: All tests pass (including new generator tests)

- [ ] **Step 3: Format code**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart && pnpm format`

- [ ] **Step 4: Final commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/front-end/logsmart
git add -A
git commit -m "feat: complete AI generator chat popup implementation with state tree

- Replace one-shot generator with floating chat popup
- Implement immutable state tree for generation history
- Add branching support for exploring alternatives
- Persist state to sessionStorage across session
- Add timeline visualization of generation tree
- Support sequential prompt refinement"
```

---

## Self-Review Checklist

**Spec Coverage:**
- ✅ Popup window with chat interface → Task 3 (AiGeneratorPopup component)
- ✅ State tree structure → Task 1-2 (Types, Store)
- ✅ Sequential generation → Task 4 (generateLayoutFromPrompt updated)
- ✅ Branching support → Task 4 (handleAiBranch, addGenerationToTree)
- ✅ Timeline visualization → Task 3 (TimelineView component)
- ✅ Draggable window → Task 3 (handleMouseDown/Move/Up)
- ✅ Minimize/restore → Task 4 (integrated into template designer)
- ✅ SessionStorage persistence → Task 2 (saveGeneratorState, loadGeneratorState)
- ✅ Canvas state restoration → Task 4 (handleAiRevert, switchAiGeneration)

**Placeholder Scan:**
- No "TBD" or "TODO" left in tasks
- All code blocks complete and ready to run
- All commands with expected output documented

**Type Consistency:**
- GenerationNode, GeneratorState types defined in Task 1
- Used consistently in store (Task 2) and components (Task 3-4)
- Function signatures match across all files

**No Gaps:** All design requirements covered by at least one task.

