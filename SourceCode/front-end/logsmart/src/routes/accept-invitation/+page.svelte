<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import { browser } from '$app/environment';
	import { api } from '$lib/api';
	import { onMount } from 'svelte';

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
	let token = $state('');
	let error = $state('');
	let fieldErrors = $state<Record<string, string[]>>({});
	let loading = $state(true);

	let firstName = $state('');
	let lastName = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let passwordErrors = $state<string[]>([]);
	let showPassword = $state(false);
	let showConfirmPassword = $state(false);
	let touched = $state({
		firstName: false,
		lastName: false,
		password: false,
		confirmPassword: false
	});
	let company_name = $state('');

	const firstNameValid = $derived(firstName.trim().length > 0);
	const lastNameValid = $derived(lastName.trim().length > 0);
	const passwordValid = $derived(validatePassword(password).valid);
	const passwordsMatch = $derived(password === confirmPassword && confirmPassword.length > 0);
	const formValid = $derived(firstNameValid && lastNameValid && passwordValid && passwordsMatch);

	const fetchInvitationDetails = async (invitationToken: string) => {
		const { data, response } = await api.GET('/auth/invitations/details', {
			params: { query: { token: invitationToken } }
		});
		if (!response.ok) {
			error = 'Invalid invitation link.';
		} else {
			company_name = data?.company_name || '';
			token = invitationToken;
		}
		loading = false;
	};

	onMount(() => {
		if (!browser) return;
		const params = new URLSearchParams(window.location.search);
		const t = params.get('token') || '';
		if (t) {
			fetchInvitationDetails(t);
		} else {
			error = 'Invalid invitation link.';
			loading = false;
		}
	});

	$effect(() => {
		if (password) {
			const validation = validatePassword(password);
			passwordErrors = validation.errors;
		} else {
			passwordErrors = [];
		}
	});

	function nextStep() {
		step = 2;
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
			firstName: true,
			lastName: true,
			password: true,
			confirmPassword: true
		};

		if (!formValid) return;

		loading = true;
		try {
			const { error: apiError } = await api.POST('/auth/invitations/accept', {
				body: {
					token,
					first_name: firstName,
					last_name: lastName,
					password
				}
			});

			if (apiError) {
				error = apiError.error || 'Failed to accept invitation.';
				if (apiError.fields) {
					fieldErrors = apiError.fields;
				}
				loading = false;
			} else {
				await invalidateAll();
				await goto('/dashboard');
			}
		} catch {
			error = 'Network error';
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Accept Invitation - LogSmart</title>
</svelte:head>

<div class="register-wrapper">
	<div class="form-container">
		{#if loading && step === 1}
			<div class="form text-center">
				<p class="text-gray-700">Processing your invitation...</p>
			</div>
		{:else if error && step === 1}
			<div class="form text-center">
				<p class="mb-4 text-red-600">{error}</p>
				<a href="/login" class="text-blue-600 hover:underline">Return to login</a>
			</div>
		{:else}
			<div class="progress-indicator">
				<div class="step" class:active={step === 1} class:completed={step > 1}>
					<div class="step-number">1</div>
					<div class="step-label">Accept Invite</div>
				</div>
				<div class="progress-line" class:completed={step > 1}></div>
				<div class="step" class:active={step === 2}>
					<div class="step-number">2</div>
					<div class="step-label">Your Details</div>
				</div>
			</div>

			{#if step === 1}
				<div class="form text-center">
					<h1>You're Invited!</h1>
					<p class="subtitle">
						You've been invited to join {company_name} on LogSmart
					</p>
					<p class="mb-6 text-gray-600">
						Click the button below to accept your invitation and create your account.
					</p>
					<button type="button" class="btn btn-primary cursor-pointer" onclick={nextStep}>
						Accept Invitation
					</button>
				</div>
			{:else}
				<form onsubmit={submit} class="form">
					<h1>Create Your Account</h1>
					<p class="subtitle">Enter your details to complete registration</p>

					{#if error}
						<div class="error" role="alert">{error}</div>
					{/if}

					<div class="field-row">
						<label class="field">
							<span class="label-text">First Name</span>
							<input
								type="text"
								bind:value={firstName}
								onblur={() => (touched.firstName = true)}
								aria-invalid={!firstNameValid || !!fieldErrors.first_name?.length}
								placeholder="John"
								required
							/>
							{#if touched.firstName && !firstNameValid}
								<div class="field-error">First name is required.</div>
							{/if}
							{#if fieldErrors.first_name}
								{#each fieldErrors.first_name as msg}
									<div class="field-error">{msg}</div>
								{/each}
							{/if}
						</label>

						<label class="field">
							<span class="label-text">Last Name</span>
							<input
								type="text"
								bind:value={lastName}
								onblur={() => (touched.lastName = true)}
								aria-invalid={!lastNameValid || !!fieldErrors.last_name?.length}
								placeholder="Doe"
								required
							/>
							{#if touched.lastName && !lastNameValid}
								<div class="field-error">Last name is required.</div>
							{/if}
							{#if fieldErrors.last_name}
								{#each fieldErrors.last_name as msg}
									<div class="field-error">{msg}</div>
								{/each}
							{/if}
						</label>
					</div>

					<label class="field">
						<span class="label-text">Password</span>
						<div class="password-input-wrapper">
							<input
								data-testid="password-input"
								type={showPassword ? 'text' : 'password'}
								bind:value={password}
								onblur={() => (touched.password = true)}
								aria-invalid={!passwordValid || !!fieldErrors.password?.length}
								placeholder="Min 8 chars, uppercase, lowercase, digit, special char"
								class="password-input"
								required
							/>
							<button
								type="button"
								class="password-toggle"
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
								<div class={passwordErrors.length === 0 ? 'text-green-600' : 'text-gray-500'}>
									✓ Password requirements
								</div>
								<div class={!/[A-Z]/.test(password) ? 'text-red-600' : 'text-green-600'}>
									{!/[A-Z]/.test(password) ? '✗' : '✓'} Uppercase letter (A-Z)
								</div>
								<div class={!/[a-z]/.test(password) ? 'text-red-600' : 'text-green-600'}>
									{!/[a-z]/.test(password) ? '✗' : '✓'} Lowercase letter (a-z)
								</div>
								<div class={!/\d/.test(password) ? 'text-red-600' : 'text-green-600'}>
									{!/\d/.test(password) ? '✗' : '✓'} Digit (0-9)
								</div>
								<div
									class={!/[!@#$%^&*()_+\-={};':"\\|,.<>/?]/.test(password)
										? 'text-red-600'
										: 'text-green-600'}
								>
									{!/[!@#$%^&*()_+\-={};':"\\|,.<>/?]/.test(password) ? '✗' : '✓'} Special character (!@#$%^&*
									etc)
								</div>
								<div class={password.length < 8 ? 'text-red-600' : 'text-green-600'}>
									{password.length < 8 ? '✗' : '✓'} At least 8 characters
								</div>
							</div>
						{/if}
						{#if touched.password && !passwordValid}
							<div class="field-error">Password must meet all requirements.</div>
						{/if}
						{#if fieldErrors.password}
							{#each fieldErrors.password as msg}
								<div class="field-error">{msg}</div>
							{/each}
						{/if}
					</label>

					<label class="field">
						<span class="label-text">Confirm Password</span>
						<div class="password-input-wrapper">
							<input
								type={showConfirmPassword ? 'text' : 'password'}
								bind:value={confirmPassword}
								onblur={() => (touched.confirmPassword = true)}
								aria-invalid={!passwordsMatch}
								placeholder="Re-enter password"
								class="password-input"
								data-testid="confirm-password-input"
								required
							/>
							<button
								type="button"
								class="password-toggle"
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
							<div class="field-error">Passwords do not match.</div>
						{/if}
					</label>

					<div class="button-group">
						<button type="button" class="btn btn-secondary" onclick={prevStep}> Back </button>
						<button type="submit" class="btn btn-primary" disabled={!formValid || loading}>
							{#if loading}
								Creating Account...
							{:else}
								Create Account
							{/if}
						</button>
					</div>
				</form>
			{/if}

			<p class="footer-text">
				Already have an account? <a href="/login">Sign in</a>
			</p>
		{/if}
	</div>
</div>

<style>
	.register-wrapper {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2rem;
		box-sizing: border-box;
	}

	.form-container {
		width: 100%;
		max-width: 600px;
	}

	.progress-indicator {
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: 2rem;
		gap: 1rem;
	}

	.step {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.5rem;
	}

	.step-number {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		background: var(--button-text);
		color: var(--text-secondary);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		transition: all 0.3s;
	}

	.step.active .step-number {
		background: var(--step-active);
		color: var(--button-text);
	}

	.step.completed .step-number {
		background: var(--step-comp-bg);
		color: var(--button-text);
	}

	.step-label {
		font-size: 0.875rem;
		color: var(--text-secondary);
	}

	.step.active .step-label {
		color: var(--text-primary);
		font-weight: 600;
	}

	.progress-line {
		width: 80px;
		height: 2px;
		background: var(--button-text);
		transition: all 0.3s;
	}

	.progress-line.completed {
		background: var(--step-comp-bg);
	}

	.form {
		width: 100%;
		background: var(--button-text);
		padding: 2rem;
		border-radius: 8px;
		box-shadow: 0 6px 20px rgba(20, 20, 50, 0.08);
	}

	h1 {
		margin: 0 0 0.5rem 0;
		font-size: 1.5rem;
	}

	.subtitle {
		color: var(--text-secondary);
		margin: 0 0 1.5rem 0;
	}

	.field {
		display: flex;
		flex-direction: column;
		margin-bottom: 1rem;
	}

	.field-row {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
	}

	.label-text {
		font-size: 0.9rem;
		font-weight: 500;
		margin-bottom: 0.25rem;
	}

	input {
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--input-invite);
		border-radius: 6px;
		font-size: 1rem;
		outline: none;
		font-family: inherit;
		width: 100%;
		box-sizing: border-box;
	}

	input:focus {
		border-color: var(--input-focus);
		box-shadow: 0 0 0 3px rgba(107, 140, 255, 0.12);
	}

	.field-error {
		color: var(--field-error);
		font-size: 0.875rem;
		margin-top: 0.5rem;
	}

	.error {
		background: var(--error-bg);
		color: var(--button-secondary);
		padding: 0.5rem 0.75rem;
		border-radius: 6px;
		margin-bottom: 1rem;
		font-size: 0.95rem;
	}

	.btn {
		padding: 0.75rem 1.5rem;
		border: none;
		border-radius: 6px;
		font-size: 1rem;
		cursor: pointer;
		font-weight: 500;
		transition: all 0.2s;
	}

	.btn-primary {
		width: 100%;
		background: var(--step-active);
		color: var(--button-text);
	}

	.btn-primary:hover:not([disabled]) {
		background: var(--step-active);
	}

	.btn-secondary {
		background: var(--button-text);
		color: var(--pass-toggle);
	}

	.btn-secondary:hover {
		background: var(--border-secondary);
	}

	.btn[disabled] {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.button-group {
		display: flex;
		gap: 1rem;
		margin-top: 1.5rem;
	}

	.button-group .btn-primary {
		flex: 1;
	}

	.footer-text {
		text-align: center;
		margin-top: 1.5rem;
		color: var(--text-secondary);
	}

	.footer-text a {
		color: var(--step-active);
		text-decoration: none;
	}

	.footer-text a:hover {
		text-decoration: underline;
	}

	.password-input-wrapper {
		position: relative;
		display: block;
	}

	.password-input {
		padding-right: 2.5rem !important;
	}

	.password-toggle {
		position: absolute;
		right: 0.5rem;
		top: 50%;
		transform: translateY(-50%);
		background: none;
		border: none;
		cursor: pointer;
		color: var(--text-secondary);
		padding: 0.25rem;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.password-toggle:hover {
		color: var(--pass-toggle);
	}
</style>
