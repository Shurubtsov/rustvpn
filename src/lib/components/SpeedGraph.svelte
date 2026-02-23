<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { formatSpeed } from '$lib/utils/format';

	interface Props {
		uploadHistory: number[];
		downloadHistory: number[];
		uploadSpeed: number;
		downloadSpeed: number;
	}

	const { uploadHistory, downloadHistory, uploadSpeed, downloadSpeed }: Props = $props();

	let canvas: HTMLCanvasElement;
	let container: HTMLDivElement;
	let resizeObserver: ResizeObserver | null = null;

	const GRAPH_POINTS = 60;
	const UPLOAD_COLOR = '#3b82f6';
	const DOWNLOAD_COLOR = '#22c55e';

	$effect(() => {
		// Re-draw whenever data props change
		// Access reactive props to track them
		void uploadHistory;
		void downloadHistory;
		void uploadSpeed;
		void downloadSpeed;
		if (canvas) drawGraph();
	});

	onMount(() => {
		resizeObserver = new ResizeObserver(() => {
			if (canvas) drawGraph();
		});
		if (container) resizeObserver.observe(container);
	});

	onDestroy(() => {
		if (resizeObserver) {
			resizeObserver.disconnect();
			resizeObserver = null;
		}
	});

	function drawGraph() {
		const ctx = canvas.getContext('2d');
		if (!ctx) return;

		const dpr = window.devicePixelRatio || 1;
		const rect = canvas.getBoundingClientRect();

		// Only draw if canvas has a valid size
		if (rect.width === 0 || rect.height === 0) return;

		canvas.width = rect.width * dpr;
		canvas.height = rect.height * dpr;
		ctx.scale(dpr, dpr);

		const w = rect.width;
		const h = rect.height;
		const padding = { top: 8, right: 8, bottom: 20, left: 50 };
		const graphW = w - padding.left - padding.right;
		const graphH = h - padding.top - padding.bottom;

		// Clear
		ctx.clearRect(0, 0, w, h);

		// Find max value for Y axis
		const allValues = [...uploadHistory, ...downloadHistory];
		let maxVal = Math.max(...allValues, 1024); // At least 1 KB/s
		// Round up to nice number
		maxVal = niceMax(maxVal);

		// Draw grid lines
		ctx.strokeStyle = 'rgba(128, 128, 128, 0.15)';
		ctx.lineWidth = 1;
		const gridLines = 4;
		for (let i = 0; i <= gridLines; i++) {
			const y = padding.top + (graphH * i) / gridLines;
			ctx.beginPath();
			ctx.moveTo(padding.left, y);
			ctx.lineTo(w - padding.right, y);
			ctx.stroke();

			// Y labels
			const val = maxVal * (1 - i / gridLines);
			ctx.fillStyle = 'rgba(128, 128, 128, 0.6)';
			ctx.font = '10px monospace';
			ctx.textAlign = 'right';
			ctx.fillText(formatSpeedShort(val), padding.left - 6, y + 3);
		}

		// Draw lines
		drawLine(ctx, downloadHistory, DOWNLOAD_COLOR, padding, graphW, graphH, maxVal);
		drawLine(ctx, uploadHistory, UPLOAD_COLOR, padding, graphW, graphH, maxVal);

		// X axis label
		ctx.fillStyle = 'rgba(128, 128, 128, 0.5)';
		ctx.font = '9px monospace';
		ctx.textAlign = 'center';
		ctx.fillText('60s ago', padding.left, h - 2);
		ctx.fillText('now', w - padding.right, h - 2);
	}

	function drawLine(
		ctx: CanvasRenderingContext2D,
		data: number[],
		color: string,
		padding: { top: number; right: number; bottom: number; left: number },
		graphW: number,
		graphH: number,
		maxVal: number
	) {
		if (data.length < 2) return;

		const step = graphW / (GRAPH_POINTS - 1);
		const startIdx = GRAPH_POINTS - data.length;

		// Fill area
		ctx.beginPath();
		ctx.moveTo(padding.left + startIdx * step, padding.top + graphH);
		for (let i = 0; i < data.length; i++) {
			const x = padding.left + (startIdx + i) * step;
			const y = padding.top + graphH - (data[i] / maxVal) * graphH;
			if (i === 0) ctx.lineTo(x, y);
			else ctx.lineTo(x, y);
		}
		ctx.lineTo(padding.left + (startIdx + data.length - 1) * step, padding.top + graphH);
		ctx.closePath();
		ctx.fillStyle = color + '15';
		ctx.fill();

		// Draw line
		ctx.beginPath();
		for (let i = 0; i < data.length; i++) {
			const x = padding.left + (startIdx + i) * step;
			const y = padding.top + graphH - (data[i] / maxVal) * graphH;
			if (i === 0) ctx.moveTo(x, y);
			else ctx.lineTo(x, y);
		}
		ctx.strokeStyle = color;
		ctx.lineWidth = 1.5;
		ctx.stroke();
	}

	function niceMax(val: number): number {
		if (val <= 0) return 1024;
		const mag = Math.pow(10, Math.floor(Math.log10(val)));
		const norm = val / mag;
		if (norm <= 1) return mag;
		if (norm <= 2) return 2 * mag;
		if (norm <= 5) return 5 * mag;
		return 10 * mag;
	}

	function formatSpeedShort(bytesPerSec: number): string {
		if (bytesPerSec < 1024) return `${Math.round(bytesPerSec)} B`;
		if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(0)} K`;
		if (bytesPerSec < 1024 * 1024 * 1024) return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} M`;
		return `${(bytesPerSec / (1024 * 1024 * 1024)).toFixed(1)} G`;
	}
</script>

<div class="w-full" bind:this={container}>
	<!-- Speed legend -->
	<div class="flex justify-between items-center mb-2 px-1">
		<div class="flex items-center gap-4">
			<div class="flex items-center gap-1.5">
				<span class="w-2.5 h-0.5 rounded-full" style="background-color: {UPLOAD_COLOR}"></span>
				<span class="text-xs text-muted-foreground">Upload</span>
				<span class="text-xs font-mono font-medium text-foreground/80">{formatSpeed(uploadSpeed)}</span>
			</div>
			<div class="flex items-center gap-1.5">
				<span class="w-2.5 h-0.5 rounded-full" style="background-color: {DOWNLOAD_COLOR}"></span>
				<span class="text-xs text-muted-foreground">Download</span>
				<span class="text-xs font-mono font-medium text-foreground/80">{formatSpeed(downloadSpeed)}</span>
			</div>
		</div>
	</div>
	<!-- Canvas graph — fills container width, fixed height -->
	<canvas
		bind:this={canvas}
		class="w-full rounded-lg bg-card border border-border"
		style="height: 160px"
	></canvas>
</div>
