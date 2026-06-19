// src/routes/(authenticated)/template-designer/aiGeneratorStore.ts

import { v4 as uuidv4 } from 'uuid';
import type {
	GenerationNode,
	GeneratorState,
	CanvasItem
} from './AiGeneratorPopup.types';

const STORAGE_KEY = 'logsmart_ai_generator_state';

/**
 * Initialize empty generator state
 */
export function createEmptyGeneratorState(): GeneratorState {
	return {
		tree: null,
		currentNodeId: null,
		latestNodeId: null,
		isOpen: false,
		position: { x: 100, y: 100 },
		isMinimized: false
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
		children: []
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
			children: [...tree.children, newNode]
		};
	}

	return {
		...tree,
		children: tree.children.map((child) => addGenerationToTree(child, parentNodeId, newNode))
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
export function getChatHistory(
	tree: GenerationNode | null,
	currentNodeId: string
): GenerationNode[] {
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
