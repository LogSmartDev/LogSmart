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
	getBreadcrumb
} from '../aiGeneratorStore';
import type { GeneratorState } from '../AiGeneratorPopup.types';

// Mock sessionStorage
const mockSessionStorage = {
	getItem: vi.fn(),
	setItem: vi.fn(),
	removeItem: vi.fn(),
	clear: vi.fn(),
	length: 0,
	key: vi.fn()
};

describe('aiGeneratorStore', () => {
	beforeEach(() => {
		// Reset mocks before each test
		vi.clearAllMocks();
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
