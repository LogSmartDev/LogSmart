<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import { api } from '$lib/api';

	function validatePassword(pwd: string): { valid: boolean; errors: string[] } {
		const errors: string[] = [];
		if (pwd.length < 8) errors.push('at least 8 characters');
		if (!/[A-Z]/.test(pwd)) errors.push('an uppercase letter');
		if (!/[a-z]/.test(pwd)) errors.push('a lowercase letter');
		if (!/\d/.test(pwd)) errors.push('a digit');
		if (!/[!@#$%^&*()_+\-={};':"\\|,.<>/?]/.test(pwd)) errors.push('a special character');
		return { valid: errors.length === 0, errors };
	}

	let step = $state(1);
	let companyName = $state('');
	let companyAddress = $state('');
	let addressSearchQuery = $state('');
	let addressSearchResults: Array<{ display_name: string; lat: string; lon: string }> = $state([]);
	let showAddressResults = $state(false);
	let addressSearchTimeout: ReturnType<typeof setTimeout> | null = null;
	let isSearchingAddress = $state(false);
	let firstName = $state('');
	let lastName = $state('');
	let email = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let loading = $state(false);
	let error = $state('');
	let fieldErrors = $state<Record<string, string[]>>({});
	let passwordErrors = $state<string[]>([]);
	let showPassword = $state(false);
	let showConfirmPassword = $state(false);
	let touched = $state({
		companyName: false,
		companyAddress: false,
		firstName: false,
		lastName: false,
		email: false,
		password: false,
		confirmPassword: false
	});

	async function searchAddresses(query: string) {
		if (!query || query.length < 3) {
			addressSearchResults = [];
			showAddressResults = false;
			return;
		}

		isSearchingAddress = true;
		try {
			const response = await fetch(
				`https://nominatim.openstreetmap.org/search?q=${encodeURIComponent(query)}&format=json&limit=5&addressdetails=1`,
				{
					headers: {
						'Accept-Language': 'en'
					}
				}
			);

			if (response.ok) {
				const results = await response.json();
				addressSearchResults = results;
				showAddressResults = results.length > 0;
			}
		} catch (err) {
			console.error('Error searching addresses:', err);
		} finally {
			isSearchingAddress = false;
		}
	}

	function handleAddressInput(event: Event) {
		const target = event.target as HTMLInputElement;
		companyAddress = target.value;
		addressSearchQuery = target.value;

		if (addressSearchTimeout) {
			clearTimeout(addressSearchTimeout);
		}

		addressSearchTimeout = setTimeout(() => {
			searchAddresses(addressSearchQuery);
		}, 500);
	}

	function selectAddress(result: { display_name: string }) {
		companyAddress = result.display_name;
		addressSearchQuery = result.display_name;
		showAddressResults = false;
		addressSearchResults = [];
	}

	function handleClickOutside(event: MouseEvent) {
		const target = event.target as HTMLElement;
		if (!target.closest('.address-search-container')) {
			showAddressResults = false;
		}
	}

	const companyNameValid = $derived(companyName.trim().length > 0);
	const companyAddressValid = $derived(companyAddress.trim().length > 0);
	const firstNameValid = $derived(firstName.trim().length > 0);
	const lastNameValid = $derived(lastName.trim().length > 0);
	const emailValid = $derived(/^\S+@\S+\.\S+$/.test(email));
	const passwordValid = $derived(validatePassword(password).valid);
	const passwordsMatch = $derived(password === confirmPassword && confirmPassword.length > 0);

	const step1Valid = $derived(companyNameValid && companyAddressValid);
	const step2Valid = $derived(
		firstNameValid && lastNameValid && emailValid && passwordValid && passwordsMatch
	);

	$effect(() => {
		if (password) {
			const validation = validatePassword(password);
			passwordErrors = validation.errors;
		} else {
			passwordErrors = [];
		}
	});

	function nextStep(e: Event) {
		e.preventDefault();
		touched.companyName = true;
		touched.companyAddress = true;
		if (step1Valid) {
			step = 2;
		}
	}

	function prevStep() {
		step = 1;
		error = '';
	}

	async function submit(e: Event) {
		e.preventDefault();
		error = '';
		fieldErrors = {};
		touched = {
			companyName: true,
			companyAddress: true,
			firstName: true,
			lastName: true,
			email: true,
			password: true,
			confirmPassword: true
		};

		if (!step2Valid) return;

		loading = true;
		try {
			const { error: apiError } = await api.POST('/auth/register', {
				body: {
					company_name: companyName,
					company_address: companyAddress,
					first_name: firstName,
					last_name: lastName,
					email,
					password
				}
			});

			if (apiError) {
				error = apiError.error || 'Registration failed';
				if (apiError.fields) {
					fieldErrors = apiError.fields;
				}
			} else {
				await invalidateAll();
				await goto('/dashboard');
			}
		} catch {
			error = 'Network error';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:window on:click={handleClickOutside} />

<svelte:head>
	<title>Register Company - LogSmart</title>
</svelte:head>

<div class="flex h-full w-full items-center justify-center bg-bg-primary p-8">
	<div class="w-full max-w-150">
		<div class="mb-8 flex items-center justify-center gap-4">
			<div class="flex flex-col items-center gap-2">
				<div
					class="flex h-10 w-10 items-center justify-center rounded-full font-semibold transition-all duration-300 {step ===
					1
						? 'bg-button-primary text-button-text'
						: step > 1
							? 'bg-step-comp-bg text-button-text'
							: 'bg-grey-lite text-grey-dark dark:bg-grey-dark dark:text-grey-dark'}"
				>
					1
				</div>
				<div
					class="text-sm {step === 1 ? 'font-semibold text-text-primary' : 'text-text-secondary'}"
				>
					Company Details
				</div>
			</div>
			<div
				class="h-0.5 w-20 transition-all duration-300 {step > 1
					? 'bg-step-comp-bg'
					: 'bg-grey-lite'}"
			></div>
			<div class="flex flex-col items-center gap-2">
				<div
					class="flex h-10 w-10 items-center justify-center rounded-full font-semibold transition-all duration-300 {step ===
					2
						? 'bg-button-primary text-button-text'
						: 'bg-grey-lite text-grey-dark'}"
				>
					2
				</div>
				<div
					class="text-sm {step === 2 ? 'font-semibold text-text-primary' : 'text-text-secondary'}"
				>
					Your Details
				</div>
			</div>
		</div>

		{#if step === 1}
			<form onsubmit={nextStep} class="w-full rounded-lg bg-bg-primary p-8 shadow-lg">
				<h1 class="mb-2 text-2xl font-bold text-text-primary">Company Information</h1>
				<p class="mb-6 text-text-secondary">Tell us about your company</p>

				{#if error}
					<div
						class="mb-4 rounded-md bg-error-bg p-3 text-sm text-error dark:bg-error-bg dark:text-error"
						role="alert"
					>
						{error}
					</div>
				{/if}

				<label class="mb-4 flex flex-col">
					<span
						id="company-name-label"
						aria-label="Company Name"
						class="mb-1 text-sm font-medium text-text-primary">Company Name</span
					>
					<input
						type="text"
						aria-labelledby="company-name-label"
						id="company-name-input"
						bind:value={companyName}
						onblur={() => (touched.companyName = true)}
						aria-invalid={!companyNameValid}
						placeholder="LogSmart Ltd"
						class="w-full rounded-md border border-border-secondary bg-bg-primary px-3 py-2 text-base text-text-primary outline-none focus:border-input-focus focus:ring-2 focus:ring-input-focus/20"
						required
					/>
					{#if touched.companyName && !companyNameValid}
						<div class="mt-2 text-sm text-field-error dark:text-field-error">
							Company name is required.
						</div>
					{/if}
				</label>

				<label class="address-search-container relative mb-4 flex flex-col">
					<span class="mb-1 text-sm font-medium text-text-primary">Company Address</span>
					<input
						type="text"
						id="company-address-input"
						bind:value={companyAddress}
						oninput={handleAddressInput}
						onblur={() => (touched.companyAddress = true)}
						autocomplete="off"
						aria-invalid={!companyAddressValid}
						placeholder="Search for address..."
						class="w-full rounded-md border border-border-secondary bg-bg-primary px-3 py-2 text-base text-text-primary outline-none focus:border-input-focus focus:ring-2 focus:ring-input-focus/20"
						required
					/>
					{#if isSearchingAddress}
						<div
							class="absolute top-17.5 z-10 w-full rounded-md border border-border-secondary bg-bg-primary p-2 text-center"
						>
							<span class="text-sm text-text-secondary">Searching...</span>
						</div>
					{:else if showAddressResults && addressSearchResults.length > 0}
						<ul
							class="absolute top-17.5 z-10 max-h-60 w-full overflow-auto rounded-md border border-border-secondary bg-bg-primary shadow-lg"
						>
							{#each addressSearchResults as result (result.lat + '-' + result.lon)}
								<li>
									<button
										type="button"
										class="w-full cursor-pointer px-3 py-2 text-left text-sm text-text-primary hover:bg-bg-secondary"
										onmousedown={() => selectAddress(result)}
									>
										{result.display_name}
									</button>
								</li>
							{/each}
						</ul>
					{/if}
					<p class="mt-1 text-xs text-text-secondary">
						&gt;(search for locations, POIs, or addresses)
					</p>
					{#if touched.companyAddress && !companyAddressValid}
						<div class="mt-2 text-sm text-field-error dark:text-field-error">
							Company address is required.
						</div>
					{/if}
				</label>

				<button
					type="submit"
					class="w-full rounded-md bg-button-primary px-6 py-3 text-base font-medium text-button-text transition-all duration-200 hover:bg-button-primary-hover disabled:cursor-not-allowed disabled:opacity-60"
					disabled={!step1Valid}
				>
					Next Step
				</button>
			</form>
		{:else}
			<form onsubmit={submit} class="w-full rounded-lg bg-bg-primary p-8 shadow-lg">
				<h1 class="mb-2 text-2xl font-bold text-text-primary">Your Details</h1>
				<p class="mb-6 text-text-secondary">Create your admin account</p>

				{#if error}
					<div
						class="mb-4 rounded-md bg-error-bg p-3 text-sm text-error dark:bg-error-bg dark:text-error"
						role="alert"
					>
						{error}
					</div>
				{/if}

				<div class="grid grid-cols-2 gap-4">
					<label class="mb-4 flex flex-col">
						<span class="mb-1 text-sm font-medium text-text-primary">First Name</span>
						<input
							type="text"
							bind:value={firstName}
							onblur={() => (touched.firstName = true)}
							aria-invalid={!firstNameValid || !!fieldErrors.first_name?.length}
							placeholder="John"
							class="w-full rounded-md border border-border-secondary bg-bg-primary px-3 py-2 text-base text-text-primary outline-none focus:border-input-focus focus:ring-2 focus:ring-input-focus/20"
							required
						/>
						{#if touched.firstName && !firstNameValid}
							<div class="mt-2 text-sm text-field-error dark:text-field-error">
								First name is required.
							</div>
						{/if}
					</label>

					<label class="mb-4 flex flex-col">
						<span class="mb-1 text-sm font-medium text-text-primary">Last Name</span>
						<input
							type="text"
							bind:value={lastName}
							onblur={() => (touched.lastName = true)}
							aria-invalid={!lastNameValid || !!fieldErrors.last_name?.length}
							placeholder="Doe"
							class="w-full rounded-md border border-border-secondary bg-bg-primary px-3 py-2 text-base text-text-primary outline-none focus:border-input-focus focus:ring-2 focus:ring-input-focus/20"
							required
						/>
						{#if touched.lastName && !lastNameValid}
							<div class="mt-2 text-sm text-field-error dark:text-field-error">
								Last name is required.
							</div>
						{/if}
					</label>
				</div>

				<label class="mb-4 flex flex-col">
					<span class="mb-1 text-sm font-medium text-text-primary">Email</span>
					<input
						type="email"
						bind:value={email}
						onblur={() => (touched.email = true)}
						aria-invalid={!emailValid || !!fieldErrors.email?.length}
						placeholder="john@company.com"
						class="w-full rounded-md border border-border-secondary bg-bg-primary px-3 py-2 text-base text-text-primary outline-none focus:border-input-focus focus:ring-2 focus:ring-input-focus/20"
						required
					/>
					{#if touched.email && !emailValid}
						<div class="mt-2 text-sm text-field-error dark:text-field-error">
							Enter a valid email address.
						</div>
					{/if}
					{#if fieldErrors.email}
						{#each fieldErrors.email as msg}
							<div class="mt-2 text-sm text-field-error dark:text-field-error">{msg}</div>
						{/each}
					{/if}
				</label>

				<label class="mb-4 flex flex-col">
					<span class="mb-1 text-sm font-medium text-text-primary">Password</span>
					<div class="relative block">
						<input
							data-testid="password-input"
							type={showPassword ? 'text' : 'password'}
							bind:value={password}
							onblur={() => (touched.password = true)}
							aria-invalid={!passwordValid || !!fieldErrors.password?.length}
							placeholder="Min 8 chars, uppercase, lowercase, digit, special char"
							class="w-full rounded-md border border-border-secondary bg-bg-primary px-3 py-2 pr-10 text-base text-text-primary outline-none focus:border-input-focus focus:ring-2 focus:ring-input-focus/20"
							required
						/>
						<button
							type="button"
							class="absolute top-1/2 right-2 flex -translate-y-1/2 cursor-pointer items-center justify-center border-none bg-transparent p-1 text-pass-toggle hover:text-text-primary"
							onclick={() => (showPassword = !showPassword)}
							aria-label={showPassword ? 'Hide password' : 'Show password'}
						>
							{#if showPassword}
								<svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21"
									/>
								</svg>
							{:else}
								<svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
									/>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
									/>
								</svg>
							{/if}
						</button>
					</div>
					{#if password}
						<div class="mt-2 space-y-1 text-xs">
							<div
								class={passwordErrors.length === 0 ? 'text-step-comp-bg' : 'text-text-secondary'}
							>
								✓ Password requirements
							</div>
							<div class={!/[A-Z]/.test(password) ? 'text-field-error' : 'text-step-comp-bg'}>
								{!/[A-Z]/.test(password) ? '✗' : '✓'} Uppercase letter (A-Z)
							</div>
							<div class={!/[a-z]/.test(password) ? 'text-field-error' : 'text-step-comp-bg'}>
								{!/[a-z]/.test(password) ? '✗' : '✓'} Lowercase letter (a-z)
							</div>
							<div class={!/\d/.test(password) ? 'text-field-error' : 'text-step-comp-bg'}>
								{!/\d/.test(password) ? '✗' : '✓'} Digit (0-9)
							</div>
							<div
								class={!/[!@#$%^&*()_+\-={};':"\\|,.<>/?]/.test(password)
									? 'text-field-error'
									: 'text-step-comp-bg'}
							>
								{!/[!@#$%^&*()_+\-={};':"\\|,.<>/?]/.test(password) ? '✗' : '✓'} Special character (!@#$%^&*
								etc)
							</div>
							<div class={password.length < 8 ? 'text-field-error' : 'text-step-comp-bg'}>
								{password.length < 8 ? '✗' : '✓'} At least 8 characters
							</div>
						</div>
					{/if}
					{#if touched.password && !passwordValid}
						<div class="mt-2 text-sm text-field-error dark:text-field-error">
							Password must meet all requirements.
						</div>
					{/if}
					{#if fieldErrors.password}
						{#each fieldErrors.password as msg}
							<div class="mt-2 text-sm text-field-error dark:text-field-error">{msg}</div>
						{/each}
					{/if}
				</label>

				<label class="mb-4 flex flex-col">
					<span class="mb-1 text-sm font-medium text-text-primary">Confirm Password</span>
					<div class="relative block">
						<input
							data-testid="confirm-password-input"
							type={showConfirmPassword ? 'text' : 'password'}
							bind:value={confirmPassword}
							onblur={() => (touched.confirmPassword = true)}
							aria-invalid={!passwordsMatch}
							placeholder="Re-enter password"
							class="w-full rounded-md border border-border-secondary bg-bg-primary px-3 py-2 pr-10 text-base text-text-primary outline-none focus:border-input-focus focus:ring-2 focus:ring-input-focus/20"
							required
						/>
						<button
							type="button"
							class="absolute top-1/2 right-2 flex -translate-y-1/2 cursor-pointer items-center justify-center border-none bg-transparent p-1 text-pass-toggle hover:text-text-primary"
							onclick={() => (showConfirmPassword = !showConfirmPassword)}
							aria-label={showConfirmPassword ? 'Hide password' : 'Show password'}
						>
							{#if showConfirmPassword}
								<svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21"
									/>
								</svg>
							{:else}
								<svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
									/>
									<path
										stroke-linecap="round"
										stroke-linejoin="round"
										stroke-width="2"
										d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
									/>
								</svg>
							{/if}
						</button>
					</div>
					{#if touched.confirmPassword && !passwordsMatch}
						<div class="mt-2 text-sm text-field-error dark:text-field-error">
							Passwords do not match.
						</div>
					{/if}
				</label>

				<div class="mt-6 flex gap-4">
					<button
						type="button"
						class="rounded-md bg-bg-secondary px-6 py-3 text-base font-medium text-text-primary transition-all duration-200 hover:bg-bg-secondary/80"
						onclick={prevStep}
					>
						Back
					</button>
					<button
						type="submit"
						class="w-full flex-1 rounded-md bg-button-primary px-6 py-3 text-base font-medium text-button-text transition-all duration-200 hover:bg-button-primary-hover disabled:cursor-not-allowed disabled:opacity-60"
						disabled={!step2Valid || loading}
					>
						{#if loading}
							Creating Account...
						{:else}
							Create Account
						{/if}
					</button>
				</div>
			</form>
		{/if}

		<p class="mt-6 text-center text-text-secondary">
			Already have an account? <a href="/login" class="text-input-focus hover:underline">Sign in</a>
		</p>
	</div>
</div>
