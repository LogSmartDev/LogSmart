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
	latestNodeId: string | null;
	isOpen: boolean;
	position: { x: number; y: number };
	isMinimized: boolean;
}

export interface BreadcrumbNode {
	id: string;
	prompt: string;
	isCurrentPath: boolean;
}
