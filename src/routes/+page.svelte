<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { Badge } from "$lib/components/ui/badge/index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import { Separator } from "$lib/components/ui/separator/index.js";
	import { ArrowRight, Command, FileText, Search, Settings, Terminal, X } from "@lucide/svelte";

	type LauncherItem = {
		title: string;
		subtitle: string;
		shortcut: string;
		icon: typeof Terminal;
	};

	const items: LauncherItem[] = [
		{
			title: "Open terminal here",
			subtitle: "Run a command in the current workspace",
			shortcut: "return",
			icon: Terminal,
		},
		{
			title: "Find project file",
			subtitle: "Search by path, symbol, or recent edit",
			shortcut: "F",
			icon: FileText,
		},
		{
			title: "Launcher settings",
			subtitle: "Tune summon key and indexing behavior",
			shortcut: ",",
			icon: Settings,
		},
	];

	let query = $state("");
	let inputRef: HTMLInputElement | null = $state(null);

	function filteredItems() {
		const normalizedQuery = query.trim().toLowerCase();

		if (!normalizedQuery) {
			return items;
		}

		return items.filter((item) =>
			`${item.title} ${item.subtitle}`.toLowerCase().includes(normalizedQuery)
		);
	}

	async function hideLauncher() {
		await invoke("hide_launcher_command");
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			event.preventDefault();
			void hideLauncher();
		}
	}

	$effect(() => {
		inputRef?.focus();
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<main class="min-h-screen bg-transparent p-3 text-foreground antialiased">
	<section
		class="launcher-shell overflow-hidden rounded-lg border border-white/12 bg-zinc-950/92 text-zinc-50 shadow-2xl shadow-black/45 backdrop-blur-2xl"
	>
		<div class="flex h-14 items-center gap-3 px-4">
			<div class="flex size-8 shrink-0 items-center justify-center rounded-md bg-cyan-300 text-zinc-950">
				<Command class="size-4" aria-hidden="true" />
			</div>
			<div class="relative min-w-0 flex-1">
				<Search
					class="pointer-events-none absolute left-0 top-1/2 size-4 -translate-y-1/2 text-zinc-500"
					aria-hidden="true"
				/>
				<Input
					bind:ref={inputRef}
					bind:value={query}
					class="h-10 border-0 bg-transparent pl-7 pr-2 text-lg text-zinc-50 shadow-none placeholder:text-zinc-500 focus-visible:ring-0 md:text-lg"
					placeholder="Search apps, files, commands..."
					spellcheck="false"
				/>
			</div>
			<Button
				variant="ghost"
				size="icon"
				class="text-zinc-400 hover:bg-white/8 hover:text-zinc-50"
				aria-label="Hide launcher"
				onclick={hideLauncher}
			>
				<X aria-hidden="true" />
			</Button>
		</div>

		<Separator class="bg-white/10" />

		<div class="grid gap-1 px-2 py-2">
			{#each filteredItems() as item, index}
				{@const Icon = item.icon}
				<button
					type="button"
					class="group grid h-[72px] grid-cols-[40px_1fr_auto] items-center gap-3 rounded-md px-3 text-left transition-colors hover:bg-white/8 focus-visible:bg-white/10 focus-visible:outline-none"
				>
					<div
						class="flex size-9 items-center justify-center rounded-md border border-white/10 bg-white/[0.06] text-cyan-200"
					>
						<Icon class="size-4" aria-hidden="true" />
					</div>
					<div class="min-w-0">
						<div class="truncate text-sm font-medium text-zinc-100">{item.title}</div>
						<div class="truncate text-xs text-zinc-500">{item.subtitle}</div>
					</div>
					<div class="flex items-center gap-2">
						<Badge variant="outline" class="border-white/10 bg-white/[0.04] text-zinc-400">
							{item.shortcut}
						</Badge>
						<ArrowRight
							class="size-4 text-zinc-600 transition-colors group-hover:text-cyan-200"
							aria-hidden="true"
						/>
					</div>
				</button>
			{:else}
				<div class="flex h-[216px] items-center justify-center px-6 text-center text-sm text-zinc-500">
					No local action matches "{query}".
				</div>
			{/each}
		</div>

		<Separator class="bg-white/10" />

		<footer class="flex h-10 items-center justify-between px-4 text-[11px] text-zinc-500">
			<span>Press \ anywhere to toggle</span>
			<span>Esc hides</span>
		</footer>
	</section>
</main>
