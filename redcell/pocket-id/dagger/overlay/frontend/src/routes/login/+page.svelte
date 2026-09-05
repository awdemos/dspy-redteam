<script lang="ts">
	import { page } from '$app/state';
	import SignInWrapper from '$lib/components/login-wrapper.svelte';
	import { Button } from '$lib/components/ui/button';
	import Input from '$lib/components/ui/input/input.svelte';
	import { m } from '$lib/paraglide/messages';
	import UserService from '$lib/services/user-service';
	import { preventDefault } from '$lib/utils/event-util';
	import { fade } from 'svelte/transition';
	import LoginLogoErrorSuccessIndicator from './components/login-logo-error-success-indicator.svelte';

	const { data } = $props();

	const userService = new UserService();

	let email = $state('');
	let isLoading = $state(false);
	let error: string | undefined = $state(undefined);
	let success = $state(false);

	async function requestEmail() {
		isLoading = true;
		await userService
			.requestOneTimeAccessEmailAsUnauthenticatedUser(email, data.redirect)
			.then(() => (success = true))
			.catch((e) => (error = e.response?.data.error || m.an_unknown_error_occurred()));

		isLoading = false;
	}
</script>

<svelte:head>
	<title>Sign in - Redcell</title>
</svelte:head>

<div class="login-theme">
	<SignInWrapper>
		{#snippet rightPanel()}
			<div class="max-w-md text-center">
				<p class="text-[28px] font-medium leading-snug tracking-tight text-white">
					Internal AI red-teaming platform.
				</p>
				<p class="mt-4 text-base text-white/60">
					Probe, evaluate, and harden target models before production.
				</p>
			</div>
		{/snippet}
		<div class="flex justify-center">
			<LoginLogoErrorSuccessIndicator {success} error={!!error} />
		</div>
		<h1 class="mt-5 text-3xl font-semibold tracking-tight sm:text-4xl">
			Sign in to Redcell
		</h1>
		{#if error}
			<p class="text-muted-foreground mt-2" in:fade>
				{error}. {m.please_try_again()}
			</p>
			<div class="mt-10 flex justify-between gap-2 w-full max-w-[450px]">
				<Button variant="secondary" class="flex-1" href={'/' + page.url.search}>
					{m.go_back()}
				</Button>
				<Button class="flex-1" onclick={() => (error = undefined)}>{m.try_again()}</Button>
			</div>
		{:else if success}
			<p class="text-muted-foreground mt-2" in:fade>
				{m.an_email_has_been_sent_to_the_provided_email_if_it_exists_in_the_system()}
			</p>
			<div class="mt-8 flex justify-between gap-2 w-full max-w-[450px]">
				<Button variant="secondary" class="flex-1" href={'/' + page.url.search}>
					{m.go_back()}
				</Button>
				<Button class="flex-1" href={'/login/alternative/code' + page.url.search}>
					{m.enter_code()}
				</Button>
			</div>
		{:else}
			<form onsubmit={preventDefault(requestEmail)} class="w-full max-w-[450px]">
				<p class="text-muted-foreground mt-2" in:fade>
					{m.enter_your_email_address_to_receive_an_email_with_a_login_code()}
				</p>
				<Input
					id="Email"
					class="mt-7"
					placeholder={m.your_email()}
					aria-label={m.email()}
					bind:value={email}
					type="email"
				/>
				<div class="mt-8 flex justify-between gap-2">
					<Button variant="secondary" class="flex-1" href={'/' + page.url.search}>
						{m.go_back()}
					</Button>
					<Button class="flex-1" type="submit" {isLoading}>{m.submit()}</Button>
				</div>
			</form>
		{/if}
		<p class="text-muted-foreground mt-10 text-xs">&copy; 2026 Redcell. All rights reserved.</p>
	</SignInWrapper>
</div>

<style>
	:global(.login-theme) {
		--background: #0a0a0a;
		--foreground: #f4f4f5;
		--card: #111111;
		--card-foreground: #f4f4f5;
		--popover: #111111;
		--popover-foreground: #f4f4f5;
		--primary: #f59e0b;
		--primary-foreground: #0a0a0a;
		--secondary: #171717;
		--secondary-foreground: #f4f4f5;
		--muted: #171717;
		--muted-foreground: #a1a1aa;
		--accent: #171717;
		--accent-foreground: #f4f4f5;
		--destructive: #ef4444;
		--destructive-foreground: #fafafa;
		--border: #232323;
		--input: #232323;
		--ring: #f59e0b;
		--radius: 1rem;
	}
	:global(.login-theme) {
		font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
			'Helvetica Neue', Arial, sans-serif;
	}
	:global(.login-theme h1) {
		font-family: 'Space Grotesk', ui-sans-serif, system-ui, sans-serif;
		letter-spacing: -0.02em;
	}
</style>
