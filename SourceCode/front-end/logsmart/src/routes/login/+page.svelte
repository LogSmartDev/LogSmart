<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import { startAuthentication } from '@simplewebauthn/browser';
	import { api } from '$lib/api';

	let email = $state('');
	let password = $state('');
	let loading = $state(false);
	let error = $state('');
	let fieldErrors = $state<Record<string, string[]>>({});
	let touched = $state({ email: false, password: false });
	const emailValid = $derived(/^\S+@\S+\.\S+$/.test(email));
	const passwordValid = $derived(password.length >= 6);
	const formValid = $derived(emailValid && passwordValid);

	async function submit(e: Event) {
		e.preventDefault();
		error = '';
		fieldErrors = {};
		touched = { email: true, password: true };
		if (!formValid) return;
		loading = true;
		try {
			const { data, error: apiError } = await api.POST('/auth/login', {
				body: { email, password }
			});

			if (apiError) {
				error = apiError.error || 'Login failed';
				if (apiError.fields) {
					fieldErrors = apiError.fields;
				}
			} else {
				await invalidateAll();
				// Redirect based on user role
				if (data?.user?.role === 'logsmart_admin') {
					await goto('/admin-dashboard');
				} else {
					await goto('/dashboard');
				}
			}
		} catch (err) {
			console.error('Login error:', err);
			error = 'Network error';
		} finally {
			loading = false;
		}
	}

	async function handlePasskeyLogin() {
		loading = true;
		error = '';

		try {
			let startResp;
			let isDiscoverable = !emailValid;

			if (isDiscoverable) {
				// One-click discoverable flow - no email required
				startResp = await fetch('/api/auth/passkey/login/discoverable/start', {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({})
				});
			} else {
				// Email-first flow (backward compatibility)
				startResp = await fetch('/api/auth/passkey/login/start', {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({ email })
				});
			}

			if (!startResp.ok) {
				const err = await startResp.json();
				throw new Error(err.error || 'Failed to start passkey login');
			}
			const startData = await startResp.json();

			const authResp = await startAuthentication(startData.options);

			const finishEndpoint = isDiscoverable
				? '/api/auth/passkey/login/discoverable/finish'
				: '/api/auth/passkey/login/finish';

			const finishResp = await fetch(finishEndpoint, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					credential: authResp,
					auth_id: startData.auth_id
				})
			});

			if (!finishResp.ok) {
				const err = await finishResp.json();
				throw new Error(err.error || 'Authentication failed');
			}

			const userData = await finishResp.json();
			await invalidateAll();
			// Redirect based on user role
			if (userData?.user?.role === 'logsmart_admin') {
				await goto('/admin-dashboard');
			} else {
				await goto('/dashboard');
			}
		} catch (e: unknown) {
			console.error(e);
			const message = e instanceof Error ? e.message : 'Passkey login failed';
			error = message || 'Passkey login failed';
		} finally {
			loading = false;
		}
	}

	async function handleGoogleLogin() {
		window.location.href = '/api/auth/google/initiate';
	}
</script>

<svelte:head>
	<title>Log In</title>
</svelte:head>
<div class="flex h-full w-full items-center justify-center bg-bg-primary p-6">
	<form onsubmit={submit} class="form bg-bg-secondary">
		<h1 class="font-semibold text-text-primary">Log in</h1>
		{#if error}
			<div class="error" role="alert">{error}</div>
		{/if}
		<label class="field">
			<span class="label-text text-text-primary">Email</span>
			<input
				type="email"
				class="text-text-primary"
				bind:value={email}
				onblur={() => (touched.email = true)}
				aria-invalid={!emailValid || !!fieldErrors.email?.length}
				aria-describedby="email-help"
				autocomplete="username webauthn"
				required
			/>
			{#if touched.email && !emailValid}
				<div id="email-help" class="field-error">Enter a valid email address.</div>
			{/if}
			{#if fieldErrors.email}
				{#each fieldErrors.email as msg (msg)}
					<div class="field-error">{msg}</div>
				{/each}
			{/if}
		</label>

		<label class="field">
			<span class="label-text text-text-primary">Password</span>
			<input
				type="password"
				class="text-text-primary"
				bind:value={password}
				onblur={() => (touched.password = true)}
				aria-invalid={!passwordValid || !!fieldErrors.password?.length}
				aria-describedby="password-help"
				required
			/>
			{#if touched.password && !passwordValid}
				<div id="password-help" class="field-error">Password must be at least 6 characters.</div>
			{/if}
			{#if fieldErrors.password}
				{#each fieldErrors.password as msg (msg)}
					<div class="field-error">{msg}</div>
				{/each}
			{/if}
		</label>

		<button type="submit" class="btn" disabled={!formValid || loading}>
			{#if loading}
				Signing in...
			{:else}
				Sign in
			{/if}
		</button>

		<div class="mt-4 flex items-center justify-between">
			<hr class="w-full border-gray-300" />
			<span class="px-2 text-sm text-gray-500">OR</span>
			<hr class="w-full border-gray-300" />
		</div>

		<button
			type="button"
			class="btn mt-4 bg-button-primary"
			onclick={handlePasskeyLogin}
			disabled={loading}
		>
			{#if loading}
				Signing in...
			{:else}
				Sign in with Passkey
			{/if}
		</button>

		<button
			type="button"
			class="btn mt-4 flex items-center justify-center gap-2 border border-text-secondary bg-bg-primary text-text-primary"
			onclick={handleGoogleLogin}
			disabled={loading}
		>
			{#if loading}
				Signing in...
			{:else}
				<svg width="18" height="18" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">
					<path
						fill="#EA4335"
						d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"
					></path>
					<path
						fill="#4285F4"
						d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"
					></path>
					<path
						fill="#FBBC05"
						d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"
					></path>
					<path
						fill="#34A853"
						d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"
					></path>
					<path fill="none" d="M0 0h48v48H0z"></path>
				</svg>
				Sign in with Google
			{/if}
		</button>

		<a href="/reset-password" class="mt-4 inline-block text-sm text-indigo-600 hover:underline"
			>Forgot password?</a
		>
	</form>
</div>

<style>
	:global(body) {
		font-family:
			system-ui,
			-apple-system,
			'Segoe UI',
			Roboto,
			'Helvetica Neue',
			Arial;
		margin: 0;
		padding: 0;
	}
	.form {
		width: 100%;
		max-width: 400px;
		padding: 2rem;
		border-radius: 8px;
		box-shadow: 0 6px 20px rgba(20, 20, 50, 0.08);
	}
	h1 {
		margin: 0 0 1rem 0;
		font-size: 1.25rem;
	}
	.field {
		display: flex;
		flex-direction: column;
		margin-bottom: 1rem;
	}
	.label-text {
		font-size: 0.9rem;
		margin-bottom: 0.25rem;
	}
	input {
		padding: 0.5rem 0.75rem;
		border: 1px solid #d1d7e0;
		border-radius: 6px;
		font-size: 1rem;
		outline: none;
	}
	input:focus {
		border-color: var(--input-focus);
		box-shadow: 0 0 0 3px rgba(107, 140, 255, 0.12);
	}
	.field-error {
		color: var(--error);
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
		width: 100%;
		padding: 0.75rem 1rem;
		background: var(--button-primary);
		color: var(--button-text);
		border: none;
		border-radius: 6px;
		font-size: 1rem;
		cursor: pointer;
	}
	.btn[disabled] {
		opacity: 0.6;
		cursor: not-allowed;
	}
</style>
