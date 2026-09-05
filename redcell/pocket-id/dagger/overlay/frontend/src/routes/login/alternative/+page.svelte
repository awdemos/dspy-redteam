<script lang="ts">
	import { page } from '$app/state';
	import SignInWrapper from '$lib/components/login-wrapper.svelte';
	import Logo from '$lib/components/logo.svelte';
	import * as Item from '$lib/components/ui/item/index.js';
	import { m } from '$lib/paraglide/messages';
	import { LucideChevronRight, LucideMail, LucideRectangleEllipsis } from '@lucide/svelte';

	const methods = [
		{
			icon: LucideMail,
			title: m.email_login(),
			description: 'Request a login code via email.',
			href: '/login/alternative/email'
		},
		{
			icon: LucideRectangleEllipsis,
			title: m.login_code(),
			description: 'Enter a login code you already received.',
			href: '/login/alternative/code'
		}
	];
</script>

<svelte:head>
	<title>{m.sign_in()}</title>
</svelte:head>

<SignInWrapper>
	<div class="flex h-full flex-col justify-center">
		<div class="bg-muted mx-auto rounded-2xl p-3">
			<Logo class="size-10" />
		</div>
		<h1 class="font-gloock mt-5 text-3xl font-bold sm:text-4xl">{m.alternative_sign_in()}</h1>
		<p class="text-muted-foreground mt-3">
			Choose one of the options below to sign in to Redcell.
		</p>
		<Item.Group class="mt-5 gap-3">
			{#each methods as method}
				<Item.Root variant="outline" class="gap-5">
					{#snippet child({ props })}
						<a href={method.href + page.url.search} {...props}>
							<Item.Media class="text-primary !self-center !translate-y-0">
								<method.icon class="size-7" />
							</Item.Media>
							<Item.Content class="text-start">
								<Item.Title class="text-lg font-semibold">{method.title}</Item.Title>
								<Item.Description>{method.description}</Item.Description>
							</Item.Content>
							<Item.Actions>
								<LucideChevronRight class="size-5" />
							</Item.Actions>
						</a>
					{/snippet}
				</Item.Root>
			{/each}
		</Item.Group>

		<a class="text-muted-foreground mt-5 text-xs" href={'/login' + page.url.search}>
			Sign in with email instead
		</a>
	</div>
</SignInWrapper>
