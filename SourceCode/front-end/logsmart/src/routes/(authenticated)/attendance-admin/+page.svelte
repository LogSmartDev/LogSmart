<script lang="ts">
	import type { PageData } from './$types';
	import type { components } from '$lib/api-types';
	import DatePicker from '$lib/components/DatePicker.svelte';
	import { generateAttendancePdfHtml } from '$lib/utils/pdf-templates';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { SvelteMap, SvelteDate, SvelteURLSearchParams } from 'svelte/reactivity';

	type ClockEvent = components['schemas']['CompanyClockEventResponse'];

	let { data } = $props<{ data: PageData }>();

	const clockEvents = $derived(data?.clockEvents ?? []);
	const branches = $derived(data?.branches ?? []);
	const members = $derived(data?.members ?? []);
	const userRole = $derived(data?.userRole ?? '');
	const isHQStaff = $derived(data?.isHQStaff ?? false);
	const nextCursor = $derived(data?.nextCursor ?? null);
	const hasPrevPage = $derived.by(() => {
		const params = new SvelteURLSearchParams(page.url.searchParams);
		const cursorParam = params.get('cursor') || '';
		const stackParam = params.get('cursor_stack') || '';
		return !!cursorParam || stackParam.split(',').filter(Boolean).length > 0;
	});

	// Create mapping from user_id to branch_id for client-side filtering
	const userToBranchMap = $derived.by(() => {
		const map = new SvelteMap<string, string | null>();
		for (const member of members) {
			map.set(member.id, member.branch_id);
		}
		return map;
	});

	// --- Date picker state (matches reports page) ---
	const today = new Date();
	const dd = String(today.getDate()).padStart(2, '0');
	const mm = String(today.getMonth() + 1).padStart(2, '0');
	const yyyy = today.getFullYear();
	const currentDateFormatted = `${dd}/${mm}/${yyyy}`;

	const tomorrow = new SvelteDate(today);
	tomorrow.setDate(tomorrow.getDate() + 1);
	const tomorrowDD = String(tomorrow.getDate()).padStart(2, '0');
	const tomorrowMM = String(tomorrow.getMonth() + 1).padStart(2, '0');
	const tomorrowYYYY = tomorrow.getFullYear();
	const tomorrowDateFormatted = `${tomorrowDD}/${tomorrowMM}/${tomorrowYYYY}`;

	// Initialise from URL params (convert ISO → DD/MM/YYYY)
	function isoToDisplay(iso: string | null): string {
		if (!iso) return '';
		const d = new Date(iso);
		if (isNaN(d.getTime())) return '';
		return `${String(d.getUTCDate()).padStart(2, '0')}/${String(d.getUTCMonth() + 1).padStart(2, '0')}/${d.getUTCFullYear()}`;
	}
	function displayToISO(display: string): string {
		const parts = display.split('/');
		if (parts.length === 3) {
			const [day, month, year] = parts;
			return `${year}-${month.padStart(2, '0')}-${day.padStart(2, '0')}`;
		}
		return '';
	}

	let dateFrom = $derived(
		isoToDisplay(page.url.searchParams.get('from')) ||
			isoToDisplay(data?.defaultFrom) ||
			currentDateFormatted
	);
	let dateTo = $derived(
		isoToDisplay(page.url.searchParams.get('to')) ||
			isoToDisplay(data?.defaultTo) ||
			tomorrowDateFormatted
	);

	// For branch_manager, auto-select their branch
	let selectedBranchId = $state('');
	$effect(() => {
		if (userRole === 'branch_manager' && data?.user?.branch_id) {
			selectedBranchId = data.user.branch_id;
		}
	});

	// If we have server defaults but no URL params, add them to the URL so data displays
	$effect(() => {
		if (
			data?.defaultFrom &&
			data?.defaultTo &&
			!page.url.searchParams.has('from') &&
			!page.url.searchParams.has('to')
		) {
			const params = new SvelteURLSearchParams();
			params.set('from', data.defaultFrom);
			params.set('to', data.defaultTo);
			// Preserve branch_id if present
			const branchId = page.url.searchParams.get('branch_id');
			if (branchId) {
				params.set('branch_id', branchId);
			}
			goto(`/attendance-admin?${params.toString()}`, { replaceState: true });
		}
	});

	// Search / filter
	let searchQuery = $state('');

	const filteredEvents = $derived(() => {
		let events = clockEvents;

		// Filter by branch (client-side)
		if (selectedBranchId && selectedBranchId.trim() !== '') {
			events = events.filter((e: ClockEvent) => {
				const userBranchId = userToBranchMap.get(e.user_id);
				return userBranchId === selectedBranchId;
			});
		}

		// Filter by search query
		if (!searchQuery.trim()) return events;
		const q = searchQuery.toLowerCase();
		return events.filter(
			(e: ClockEvent) =>
				e.first_name.toLowerCase().includes(q) ||
				e.last_name.toLowerCase().includes(q) ||
				e.email.toLowerCase().includes(q)
		);
	});

	const hasAppliedFilter = $derived(
		page.url.searchParams.has('from') ||
			page.url.searchParams.has('to') ||
			(data?.defaultFrom && data?.defaultTo)
	);

	// --- Apply / Clear ---
	function applyDateFilter() {
		const params = new SvelteURLSearchParams();
		const fromISO = displayToISO(dateFrom);
		const toISO = displayToISO(dateTo);
		if (fromISO) params.set('from', new SvelteDate(fromISO).toISOString());
		if (toISO) {
			const end = new SvelteDate(toISO);
			end.setHours(23, 59, 59, 999);
			params.set('to', end.toISOString());
		}
		// Only add branch_id if it's not empty
		if (selectedBranchId && selectedBranchId.trim() !== '') {
			params.set('branch_id', selectedBranchId.trim());
		}
		const qs = params.toString();
		goto(`/attendance-admin${qs ? '?' + qs : ''}`, { invalidateAll: true });
	}

	function clearFilters() {
		dateFrom = '';
		dateTo = '';
		searchQuery = '';
		// Don't clear branch filter for branch_manager
		if (userRole !== 'branch_manager') {
			selectedBranchId = '';
		}
		goto('/attendance-admin', { invalidateAll: true });
	}

	function goToNextPage() {
		if (!nextCursor) return;
		const params = new SvelteURLSearchParams(page.url.searchParams);
		const currentCursor = params.get('cursor') || '';
		const stackParam = params.get('cursor_stack') || '';
		const stack = stackParam ? stackParam.split(',').filter(Boolean) : [];
		if (currentCursor) {
			stack.push(currentCursor);
		}
		params.set('cursor', nextCursor);
		if (stack.length > 0) {
			params.set('cursor_stack', stack.join(','));
		}
		goto(`/attendance-admin?${params.toString()}`, { invalidateAll: true });
	}

	function goToPreviousPage() {
		const params = new SvelteURLSearchParams(page.url.searchParams);
		const stackParam = params.get('cursor_stack') || '';
		const stack = stackParam ? stackParam.split(',').filter(Boolean) : [];
		if (stack.length > 0) {
			const prevCursor = stack.pop() ?? '';
			params.set('cursor', prevCursor);
			if (stack.length > 0) {
				params.set('cursor_stack', stack.join(','));
			} else {
				params.delete('cursor_stack');
			}
		} else {
			params.delete('cursor');
			params.delete('cursor_stack');
		}
		goto(`/attendance-admin?${params.toString()}`, { invalidateAll: true });
	}

	function formatDateTime(iso: string) {
		if (!iso) return '—';
		const d = new Date(iso);
		return d.toLocaleString('en-GB', {
			day: '2-digit',
			month: 'short',
			year: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	function formatDuration(clockIn: string, clockOut: string | null) {
		if (!clockOut) return 'In progress';
		const ms = new Date(clockOut).getTime() - new Date(clockIn).getTime();
		const hours = Math.floor(ms / 3600000);
		const mins = Math.floor((ms % 3600000) / 60000);
		return `${hours}h ${mins}m`;
	}

	function printToPDF() {
		const events = filteredEvents();
		const companyName = data?.user?.company_name ?? 'Company';
		const dateRange =
			dateFrom || dateTo ? `${dateFrom || 'Start'} to ${dateTo || 'Present'}` : 'All time';

		const html = generateAttendancePdfHtml({
			companyName,
			dateRange,
			events,
			formatDateTime,
			formatDuration
		});

		const w = window.open('', '_blank');
		if (w) {
			w.document.write(html);
			w.document.close();
			setTimeout(() => {
				w.print();
			}, 400);
		}
	}
</script>

<svelte:head>
	<title>Attendance</title>
</svelte:head>

<div class="attendance-page h-full w-full bg-bg-secondary">
	<div class="mx-auto max-w-7xl px-6 py-8">
		<!-- Header -->
		<div class="mb-6 flex flex-wrap items-center justify-between gap-4">
			<h1 class="text-3xl font-bold text-text-primary">Attendance Overview</h1>
			<button
				onclick={printToPDF}
				disabled={!hasAppliedFilter || filteredEvents().length === 0}
				class="cursor-pointer rounded border-2 border-button-primary bg-button-primary px-5 py-2 font-medium text-bg-primary transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-50"
			>
				Download PDF
			</button>
		</div>

		<!-- Filters -->
		<div
			class="mb-6 flex flex-wrap items-end gap-4 rounded border-2 border-border-primary bg-bg-primary p-4"
		>
			<!-- Branch Filter (only for company_manager or hq staff) -->
			{#if (userRole === 'company_manager' || userRole === 'logsmart_admin' || isHQStaff) && branches.length > 0}
				<div class="flex flex-col gap-1">
					<label for="filter-branch" class="text-xs font-medium text-text-secondary">
						Branch
					</label>
					<select
						id="filter-branch"
						bind:value={selectedBranchId}
						class="min-w-45 border-2 border-border-primary bg-bg-secondary px-3 py-2 text-sm text-text-primary"
					>
						<option value="">All Branches</option>
						{#each branches as branch (branch.id)}
							<option value={branch.id}>{branch.name}</option>
						{/each}
					</select>
				</div>
			{/if}

			<!-- Date From -->
			<div class="flex flex-col gap-1">
				<label for="filter-from" class="text-xs font-medium text-text-secondary">From</label>
				<DatePicker
					inputId="filter-from"
					bind:value={dateFrom}
					inputClass="border-2 px-3 py-2 text-sm"
					inputStyle="border-color: var(--border-primary); background-color: var(--bg-secondary); color: var(--text-primary); width: 140px;"
					buttonStyle="border-color: var(--border-primary); background-color: var(--bg-secondary); color: var(--text-primary);"
					openCalendarAriaLabel="Open calendar for start date"
				/>
			</div>

			<!-- Date To -->
			<div class="flex flex-col gap-1">
				<label for="filter-to" class="text-xs font-medium text-text-secondary">To</label>
				<DatePicker
					inputId="filter-to"
					bind:value={dateTo}
					inputClass="border-2 px-3 py-2 text-sm"
					inputStyle="border-color: var(--border-primary); background-color: var(--bg-secondary); color: var(--text-primary); width: 140px;"
					buttonStyle="border-color: var(--border-primary); background-color: var(--bg-secondary); color: var(--text-primary);"
					openCalendarAriaLabel="Open calendar for end date"
				/>
			</div>

			<button
				onclick={applyDateFilter}
				class="cursor-pointer rounded border-2 border-button-primary bg-button-primary px-4 py-2 text-sm font-medium text-text-primary transition-opacity hover:opacity-80"
			>
				Apply
			</button>
			<button
				onclick={clearFilters}
				class="cursor-pointer rounded border-2 border-border-primary bg-bg-primary px-4 py-2 text-sm font-medium text-text-primary transition-opacity hover:opacity-80"
			>
				Clear
			</button>

			<div class="ml-auto flex flex-col gap-1">
				<label for="filter-search" class="text-xs font-medium text-text-secondary">Search</label>
				<input
					id="filter-search"
					type="text"
					bind:value={searchQuery}
					placeholder="Filter by name or email..."
					class="min-w-55 rounded border border-border-primary bg-bg-secondary px-3 py-2 text-sm text-text-primary"
				/>
			</div>
		</div>

		<!-- Summary -->
		<div class="mb-4 text-sm text-text-secondary">
			{#if hasAppliedFilter}
				{filteredEvents().length} record{filteredEvents().length !== 1 ? 's' : ''}
				{#if searchQuery.trim()}
					&mdash; filtered by &ldquo;{searchQuery}&rdquo;
				{/if}
			{:else}
				Waiting for filter application...
			{/if}
		</div>

		<!-- Table -->
		<div class="overflow-x-auto rounded border-2 border-border-primary bg-bg-primary">
			<table class="w-full text-left text-sm">
				<thead>
					<tr class="bg-button-primary text-text-primary">
						<th class="px-4 py-3 font-medium">Employee</th>
						<th class="px-4 py-3 font-medium">Email</th>
						<th class="px-4 py-3 font-medium">Clock In</th>
						<th class="px-4 py-3 font-medium">Clock Out</th>
						<th class="px-4 py-3 font-medium">Duration</th>
						<th class="px-4 py-3 font-medium">Status</th>
					</tr>
				</thead>
				<tbody>
					{#if !hasAppliedFilter}
						<tr>
							<td colspan="6" class="px-4 py-8 text-center text-text-secondary">
								Please select a date range and apply to view records
							</td>
						</tr>
					{:else if filteredEvents().length === 0}
						<tr>
							<td colspan="6" class="px-4 py-8 text-center text-text-secondary">
								No attendance records found
							</td>
						</tr>
					{:else}
						{#each filteredEvents() as event (event.id)}
							<tr class="border-b border-border-secondary transition-colors hover:opacity-90">
								<td class="px-4 py-3 text-text-primary">
									{event.first_name}
									{event.last_name}
								</td>
								<td class="px-4 py-3 text-text-secondary">
									{event.email}
								</td>
								<td class="px-4 py-3 text-text-primary">
									{formatDateTime(event.clock_in)}
								</td>
								<td class="px-4 py-3 text-text-primary">
									{event.clock_out ? formatDateTime(event.clock_out) : '—'}
								</td>
								<td class="px-4 py-3 text-text-primary">
									{formatDuration(event.clock_in, event.clock_out)}
								</td>
								<td class="px-4 py-3">
									{#if event.status === 'in'}
										<span
											class="inline-block rounded-full bg-clock-in-bg px-2 py-0.5 text-xs font-semibold text-button-primary"
										>
											Clocked In
										</span>
									{:else}
										<span
											class="inline-block rounded-full bg-clock-out-bg px-2 py-0.5 text-xs font-semibold text-text-secondary"
										>
											Clocked Out
										</span>
									{/if}
								</td>
							</tr>
						{/each}
					{/if}
				</tbody>
			</table>
		</div>

		<!-- Pagination -->
		<div class="mt-4 flex items-center justify-between">
			<button
				onclick={goToPreviousPage}
				disabled={!hasPrevPage}
				class="rounded border-2 border-border-primary bg-bg-primary px-4 py-2 text-sm font-medium text-text-primary transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-50"
			>
				Previous
			</button>
			<span class="text-sm text-text-secondary">Showing up to 25 records per page</span>
			<button
				onclick={goToNextPage}
				disabled={!nextCursor}
				class="rounded border-2 border-border-primary bg-bg-primary px-4 py-2 text-sm font-medium text-text-primary transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-50"
			>
				Next
			</button>
		</div>
	</div>
</div>

<style>
	.attendance-page button:not(:disabled) {
		cursor: pointer;
	}
</style>
