# Chat-Based AI Generator with State Tree

**Date:** 2026-05-27  
**Status:** Approved  
**Scope:** Transform one-shot AI generator into an interactive chat interface with generation history and branching support

---

## Overview

Currently, the AI generator is a one-shot solution: users enter a prompt, get one generation, and can only undo back to the original state. This design introduces a **chat-based generator with an immutable state tree**, allowing users to:

- Have sequential conversations with the AI, building on previous prompts
- Explore alternative branches from any generation point
- Navigate the generation history visually
- Persist state during editing sessions
- Revert to any previous generation state

---

## Architecture

### State Tree Structure

Each generation is a **node in an immutable tree**:

```typescript
interface GenerationNode {
  id: string;                           // Unique identifier (uuid)
  parentId: string | null;              // Parent generation (null for root)
  prompt: string;                       // User's prompt text
  response: CanvasItem[];               // Generated components from this prompt
  canvasStateAtGeneration: CanvasItem[]; // Full canvas state before this generation
  timestamp: number;                    // When generation occurred
  children: GenerationNode[];           // Branches from this node
}

interface GeneratorState {
  tree: GenerationNode;                 // Root of the generation tree
  currentNodeId: string;                // ID of currently active node
  isOpen: boolean;                      // Popup visibility
  position: { x: number; y: number };   // Window position (for dragging)
  isMinimized: boolean;                 // Minimized state
}
```

### Navigation & Branching

- **Sequential generation:** Each new prompt adds a child to the current node
- **Branching:** Clicking on an earlier generation switches the current node; generating from there creates a sibling
- **Current state:** The canvas always reflects the state at `currentNodeId`
- **Non-destructive:** Previous branches remain in the tree; users can return to them anytime

---

## UI Components

### Popup Window Structure

```
┌─────────────────────────────┐
│ AI Generator  [−] [×]       │  ← Header (draggable, minimizable)
├─────────────────────────────┤
│ Timeline │ Chat Area        │
│ ────────┼──────────────────│
│ ○       │ You: "Add a..."  │
│ │       │ AI: [response]   │
│ ├─○     │ You: "Make it..." │
│ │ ├─ ○  │ AI: [response]   │  ← Scrollable chat
│ │ └─ ○  │ (branch indicator)│
│ │       │                  │
│ └─○     │                  │
│         │                  │
├─────────────────────────────┤
│ [Type prompt...] [Generate] │  ← Input area
│ [Branch Here] [Revert]      │
└─────────────────────────────┘
```

**Components:**

1. **Header**
   - Title: "AI Generator"
   - Minimize button → collapses to button in template designer
   - Close button → closes popup, preserves state in sessionStorage

2. **Timeline (Left Panel)**
   - Vertical timeline showing generation tree structure
   - Each node is a clickable circle
   - Branches visualized with vertical/horizontal lines
   - Current node highlighted
   - Shows branch points clearly

3. **Chat Area (Main Panel)**
   - Scrollable message history
   - User prompts styled as "user" messages
   - AI responses styled as "assistant" messages
   - Branch indicator when switching nodes (e.g., "← Switched to generation #2")
   - Timestamps or generation count

4. **Input Section (Bottom)**
   - Text input for new prompt (placeholder: "Describe what you want...")
   - "Generate" button (disabled while generating)
   - "Branch from here" button (only when not on leaf node)
   - "Revert to this state" button (contextual, appears when viewing non-current node)
   - Loading indicator during generation

---

## State Management (Svelte)

All state lives in the template designer page's `<script context="module">`:

```typescript
let generatorState = $state<GeneratorState>({
  tree: null,
  currentNodeId: null,
  isOpen: false,
  position: { x: 100, y: 100 },
  isMinimized: false,
});

// Derived state
const currentNode = $derived.by(() => {
  return findNodeById(generatorState.tree, generatorState.currentNodeId);
});

const canBranch = $derived(currentNode?.children?.length > 0 ?? false);

const breadcrumb = $derived.by(() => {
  return getPathToNode(generatorState.tree, generatorState.currentNodeId);
});
```

### Key Functions

- **`addGeneration(prompt: string, response: CanvasItem[])`**
  - Creates new GenerationNode as child of currentNode
  - Updates currentNodeId
  - Saves full canvas state for potential revert

- **`switchGeneration(nodeId: string)`**
  - Updates currentNodeId
  - Optionally reverts canvas to that generation's state (based on user action)

- **`branchFromNode(nodeId: string)`**
  - Sets currentNodeId to nodeId
  - On next generation, creates sibling instead of overwriting

- **`revertToGeneration(nodeId: string)`**
  - Restores canvas to state before that generation
  - Sets currentNodeId to nodeId
  - Marks as explicit user action (not auto-revert on AI generation)

- **`minimizeGenerator()`**
  - Sets isMinimized = true
  - Popup collapses to small button or disappears
  - State persists in sessionStorage

- **`openGenerator()`**
  - Sets isOpen = true, isMinimized = false
  - Pops window back up

---

## Persistence

- **Storage:** Browser `sessionStorage` (clears on tab close, persists during work session)
- **Key:** `logsmart_ai_generator_state`
- **On load:** Check sessionStorage for existing state; restore if available
- **Canvas sync:** Generator state is separate from canvas; canvas auto-saves as usual
- **Recovery:** If state is corrupted, gracefully reset to empty tree

---

## Integration with Template Designer

1. **Replace AiGeneratorSidebar component** with a button that launches the popup
2. **Move generation logic** into the popup component
3. **Keep canvas as the source of truth** for what's displayed
4. **Handle edge cases:**
   - If user manually edits canvas while generator is open → mark as "diverged" state
   - If user generates from a non-leaf node with existing children → create new sibling, don't overwrite

---

## Interaction Flow

### New Session
1. User clicks "AI Generator" button
2. Popup launches (floating, draggable)
3. User enters prompt, clicks "Generate"
4. Generation 0 created, canvas updates

### Sequential Changes
1. User enters new prompt (e.g., "Make it blue")
2. Clicks "Generate"
3. Generation 1 created as child of Generation 0
4. Canvas updates with both prompts applied (through AI context)
5. Chat shows both messages in sequence

### Exploring Branches
1. User clicks on Generation 0 in timeline
2. Canvas reverts to state at Generation 0
3. User generates new prompt
4. Generation 1b created as second child of Generation 0
5. Timeline now shows fork visually

### Returning to Branch
1. User clicks on Generation 1b in timeline
2. Canvas restores to Generation 1b's state
3. User can continue generating from there

---

## Error Handling

- **Generation failure:** Show error message in chat, don't create node
- **Invalid prompt:** Show validation error (e.g., max 1000 chars)
- **Canvas divergence:** If user manually edits while generator is open, log it; don't break state
- **Storage quota:** Gracefully degrade if sessionStorage is full; offer to clear old branches
- **State corruption:** Reset to empty tree on load; warn user

---

## Future Enhancements (Out of Scope)

- Export/import generation trees
- Collaborate on templates in real-time with shared generator state
- Persistent history across sessions (localStorage instead of sessionStorage)
- Generation tree search/filter
- Template suggestions based on generation history
- A/B comparison view of branches

---

## Files to Modify/Create

### Frontend

- **Create:** `src/routes/(authenticated)/template-designer/AiGeneratorPopup.svelte`
  - Main popup component with chat UI, timeline, and input

- **Create:** `src/routes/(authenticated)/template-designer/AiGeneratorPopup.types.ts`
  - TypeScript types for GenerationNode, GeneratorState

- **Create:** `src/routes/(authenticated)/template-designer/aiGeneratorStore.ts`
  - State management functions and persistence helpers

- **Modify:** `src/routes/(authenticated)/template-designer/+page.svelte`
  - Replace AiGeneratorSidebar with button
  - Integrate AiGeneratorPopup component
  - Move generation logic to popup

- **Delete:** `src/routes/(authenticated)/template-designer/AiGeneratorSidebar.svelte`
  - Replace with new popup model

### Backend

- No changes required (existing `/llm/generate-layout` endpoint remains the same)

---

## Testing Strategy

### Unit Tests (Frontend)

- Tree insertion (adding nodes as children/siblings)
- Tree navigation (finding nodes, building breadcrumbs)
- State mutations (switching generation, branching)
- SessionStorage serialization/deserialization

### Integration Tests

- Generate → verify node created
- Generate → switch node → verify canvas state
- Generate → branch → verify sibling creation
- Minimize/restore popup
- Reload page → verify state restored

### Manual Testing

- Generate multiple times in sequence
- Branch from middle node
- Navigate between branches
- Drag popup around
- Minimize/restore
- Manual canvas edits while generator open
- Close popup with state, reopen

---

## Acceptance Criteria

✅ Popup window launches from button in template designer  
✅ Users can enter sequential prompts that build on each other  
✅ Generation history visible in chat format  
✅ Timeline shows tree structure with branches  
✅ Clicking any generation switches canvas to that state  
✅ Generating from non-leaf node creates sibling, not overwrite  
✅ Minimize/restore functionality works  
✅ State persists in sessionStorage during session  
✅ No console errors or broken canvas state  
✅ Draggable window behaves smoothly  

