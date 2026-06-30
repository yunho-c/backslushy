<script lang="ts">
	import { onMount, tick } from "svelte";
	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import {
		loadAliases,
		makeAlias,
		normalizeAliasKey,
		parseTags,
		saveAliases,
		suggestAliasKey,
		type AliasRecord,
	} from "$lib/aliases.js";
	import { Badge } from "$lib/components/ui/badge/index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import { Separator } from "$lib/components/ui/separator/index.js";
	import { Check, Clipboard, Plus, Search, X } from "@lucide/svelte";

	type Mode = "search" | "create" | "edit";
	type LauncherStage = "hidden" | "showing" | "shown" | "hiding";
	type AliasResult = { kind: "alias"; alias: AliasRecord; exact: boolean };
	type CreateResult = { kind: "create"; key: string; expansion: string };
	type Result = AliasResult | CreateResult;

	let query = $state("");
	let inputRef: HTMLInputElement | null = $state(null);
	let expansionRef: HTMLTextAreaElement | null = $state(null);
	let panelRef: HTMLElement | null = $state(null);
	let aliases = $state<AliasRecord[]>([]);
	let mode = $state<Mode>("search");
	let selectedIndex = $state(0);
	let status = $state("Ready");
	let loaded = $state(false);
	let focusDiagnosticsEnabled = $state(false);
	let launcherStage = $state<LauncherStage>("hidden");

	let editingId = $state<string | null>(null);
	let draftKey = $state("");
	let draftExpansion = $state("");
	let draftDescription = $state("");
	let draftTags = $state("");

	function results(): Result[] {
		const normalizedQuery = query.trim().toLowerCase();

		if (!normalizedQuery) {
			return rankedAliases(aliases).slice(0, 5).map((alias) => ({
				kind: "alias",
				alias,
				exact: false,
			}));
		}

		const matched = aliases
			.map((alias) => ({
				kind: "alias" as const,
				alias,
				exact: alias.key === normalizedQuery,
				score: aliasScore(alias, normalizedQuery),
			}))
			.filter((result) => result.score < 100)
			.sort((a, b) => a.score - b.score || b.alias.usageCount - a.alias.usageCount);

		if (!matched.some((result) => result.exact)) {
			return [
				...matched.slice(0, 4),
				{
					kind: "create",
					key: normalizedQuery.includes(" ")
						? suggestAliasKey(normalizedQuery)
						: normalizeAliasKey(normalizedQuery),
					expansion: normalizedQuery.includes(" ") ? query.trim() : "",
				},
			];
		}

		return matched.slice(0, 5);
	}

	function rankedAliases(values: AliasRecord[]) {
		return [...values].sort(
			(a, b) =>
				b.usageCount - a.usageCount ||
				new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
		);
	}

	function aliasScore(alias: AliasRecord, normalizedQuery: string) {
		if (alias.key === normalizedQuery) return 0;
		if (alias.key.startsWith(normalizedQuery)) return 10;
		if (alias.key.includes(normalizedQuery)) return 20;
		if (alias.expansion.toLowerCase().includes(normalizedQuery)) return 40;
		if (alias.description.toLowerCase().includes(normalizedQuery)) return 50;
		if (alias.tags.some((tag) => tag.includes(normalizedQuery))) return 60;
		return 100;
	}

	function activeElementSummary() {
		const activeElement = document.activeElement;
		if (!(activeElement instanceof HTMLElement)) return "none";

		const parts = [activeElement.tagName.toLowerCase()];
		if (activeElement.id) parts.push(`#${activeElement.id}`);
		if (activeElement.className && typeof activeElement.className === "string") {
			parts.push(`.${activeElement.className.trim().split(/\s+/).slice(0, 3).join(".")}`);
		}

		const role = activeElement.getAttribute("role");
		if (role) parts.push(`[role=${role}]`);
		return parts.join("");
	}

	function logFocusDiagnostics(stage: string, detail: Record<string, unknown> = {}) {
		if (!focusDiagnosticsEnabled) return;

		const selected = selectedAlias();
		const payload = {
			stage,
			mode,
			queryLength: query.length,
			selectedAliasKey: selected?.key ?? null,
			documentHasFocus: document.hasFocus(),
			activeElement: activeElementSummary(),
			...detail,
		};
		console.info("[bkslash:focus]", payload);
		void invoke("log_frontend_focus_diagnostic", { stage, detail: payload });
	}

	async function hideLauncher() {
		logFocusDiagnostics("frontend-before-hide");
		launcherStage = "hiding";
		await invoke("hide_launcher_command");
		logFocusDiagnostics("frontend-after-hide");
	}

	async function copyAlias(alias: AliasRecord) {
		logFocusDiagnostics("frontend-before-copy", { aliasKey: alias.key });
		await writeClipboard(alias.expansion);
		logFocusDiagnostics("frontend-after-copy", { aliasKey: alias.key });
		recordAliasUse(alias);
		status = `Copied ${alias.key}`;
		window.setTimeout(() => void hideLauncher(), 120);
	}

	async function pasteAlias(alias: AliasRecord) {
		logFocusDiagnostics("frontend-before-paste", { aliasKey: alias.key });
		try {
			await invoke("paste_alias_command", {
				expansion: alias.expansion,
				restoreClipboard: true,
			});
			logFocusDiagnostics("frontend-after-paste", { aliasKey: alias.key });
			recordAliasUse(alias);
			status = `Pasted ${alias.key}`;
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error);
			status = message;
			logFocusDiagnostics("frontend-paste-error", { aliasKey: alias.key, message });
		}
	}

	function recordAliasUse(alias: AliasRecord) {
		aliases = aliases.map((item) =>
			item.id === alias.id
				? { ...item, usageCount: item.usageCount + 1, updatedAt: new Date().toISOString() }
				: item
		);
	}

	async function writeClipboard(value: string) {
		if (navigator.clipboard?.writeText) {
			await navigator.clipboard.writeText(value);
			return;
		}

		const textarea = document.createElement("textarea");
		textarea.value = value;
		textarea.setAttribute("readonly", "");
		textarea.style.position = "fixed";
		textarea.style.opacity = "0";
		document.body.append(textarea);
		textarea.select();
		document.execCommand("copy");
		textarea.remove();
	}

	function activate(result = results()[selectedIndex]) {
		if (!result) return;

		if (result.kind === "create") {
			beginCreate(result.key, result.expansion);
			return;
		}

		void pasteAlias(result.alias);
	}

	function beginCreate(seedKey = query, seedExpansion = "") {
		const trimmedSeed = seedKey.trim();
		const looksLikeExpansion = trimmedSeed.includes(" ");
		mode = "create";
		editingId = null;
		draftKey = normalizeAliasKey(looksLikeExpansion ? suggestAliasKey(trimmedSeed) : trimmedSeed);
		draftExpansion = seedExpansion || (looksLikeExpansion ? trimmedSeed : "");
		draftDescription = "";
		draftTags = "";
		status = "New alias";
		void tick().then(() => expansionRef?.focus());
	}

	function beginEdit(alias: AliasRecord) {
		mode = "edit";
		editingId = alias.id;
		draftKey = alias.key;
		draftExpansion = alias.expansion;
		draftDescription = alias.description;
		draftTags = alias.tags.join(", ");
		status = `Editing ${alias.key}`;
		void tick().then(() => expansionRef?.focus());
	}

	function saveDraft() {
		const key = normalizeAliasKey(draftKey);
		const expansion = draftExpansion.trim();

		if (!key || !expansion) {
			status = "Alias and expansion required";
			return;
		}

		const duplicate = aliases.find((alias) => alias.key === key && alias.id !== editingId);
		if (duplicate) {
			status = `${key} already exists`;
			return;
		}

		if (mode === "edit" && editingId) {
			aliases = aliases.map((alias) =>
				alias.id === editingId
					? {
							...alias,
							key,
							expansion,
							description: draftDescription.trim(),
							tags: parseTags(draftTags),
							updatedAt: new Date().toISOString(),
						}
					: alias
			);
			status = `Saved ${key}`;
		} else {
			const alias = makeAlias(key, expansion, draftDescription, parseTags(draftTags));
			aliases = [alias, ...aliases];
			query = key;
			selectedIndex = 0;
			status = `Created ${key}`;
		}

		mode = "search";
		void tick().then(() => inputRef?.focus());
	}

	function cancelDraft() {
		mode = "search";
		status = "Ready";
		void tick().then(() => inputRef?.focus());
	}

	function selectedAlias() {
		const result = results()[selectedIndex];
		return result?.kind === "alias" ? result.alias : null;
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			event.preventDefault();
			if (mode === "search") {
				void hideLauncher();
			} else {
				cancelDraft();
			}
			return;
		}

		if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "n") {
			event.preventDefault();
			beginCreate(query);
			return;
		}

		if (mode !== "search") {
			if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
				event.preventDefault();
				saveDraft();
			}
			return;
		}

		if (event.key === "ArrowDown") {
			event.preventDefault();
			selectedIndex = Math.min(selectedIndex + 1, results().length - 1);
		} else if (event.key === "ArrowUp") {
			event.preventDefault();
			selectedIndex = Math.max(selectedIndex - 1, 0);
		} else if (event.key === "Enter") {
			event.preventDefault();
			activate();
		} else if (event.key === "Tab") {
			const alias = selectedAlias();
			if (alias) {
				event.preventDefault();
				beginEdit(alias);
			}
		}
	}

	function handleWindowClick(event: MouseEvent) {
		if (event.target instanceof Node && !panelRef?.contains(event.target)) {
			void hideLauncher();
		}
	}

	function handleLauncherDragMouseDown(event: MouseEvent) {
		if (event.button !== 0 || event.detail !== 1) return;
		if (!(event.target instanceof Element)) return;
		if (!(event.currentTarget instanceof Element)) return;

		const blocker = event.target.closest(
			'a, button, input, label, select, textarea, [contenteditable]:not([contenteditable="false"]), [role="button"], [role="link"], [role="menuitem"], [role="tab"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"]'
		);
		if (blocker && event.currentTarget.contains(blocker)) return;

		event.preventDefault();
		event.stopPropagation();
		void invoke("start_launcher_drag_command");
	}

	function focusLauncherField() {
		void tick().then(() => {
			if (mode === "search") {
				inputRef?.focus();
			} else {
				expansionRef?.focus();
			}
			logFocusDiagnostics("frontend-after-focus-field");
		});
	}

	$effect(() => {
		if (!loaded) return;
		saveAliases(aliases);
	});

	$effect(() => {
		query;
		selectedIndex = 0;
	});

	onMount(() => {
		aliases = loadAliases();
		loaded = true;
		void invoke<boolean>("focus_diagnostics_enabled").then((enabled) => {
			focusDiagnosticsEnabled = enabled;
			logFocusDiagnostics("frontend-diagnostics-ready");
		});
		focusLauncherField();

		const unlisteners: Array<() => void> = [];
		void listen("launcher-showing", () => {
			launcherStage = "showing";
			window.requestAnimationFrame(() => {
				if (launcherStage === "showing") {
					launcherStage = "shown";
				}
			});
		}).then((cleanup) => unlisteners.push(cleanup));

		void listen("launcher-shown", () => {
			if (launcherStage !== "showing") {
				launcherStage = "shown";
			}
			logFocusDiagnostics("frontend-launcher-shown");
			focusLauncherField();
		}).then((cleanup) => unlisteners.push(cleanup));

		void listen("launcher-hiding", () => {
			launcherStage = "hiding";
		}).then((cleanup) => unlisteners.push(cleanup));

		void listen("launcher-hidden", () => {
			launcherStage = "hidden";
		}).then((cleanup) => unlisteners.push(cleanup));

		return () => {
			for (const unlisten of unlisteners) {
				unlisten();
			}
		};
	});
</script>

<svelte:window onclick={handleWindowClick} onkeydown={handleKeydown} />

<main class="grid min-h-screen place-items-center bg-transparent p-16 text-foreground antialiased">
	<section
		bind:this={panelRef}
		data-launcher-stage={launcherStage}
		class="launcher-shell w-[720px] overflow-hidden rounded-lg border border-white/12 bg-zinc-950/92 text-zinc-50 shadow-2xl shadow-black/45 backdrop-blur-2xl"
	>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			data-tauri-drag-region="deep"
			class="flex h-14 items-center gap-3 px-4"
			onmousedown={handleLauncherDragMouseDown}
		>
			<div class="relative min-w-0 flex-1">
				<Search
					class="pointer-events-none absolute left-0 top-1/2 size-4 -translate-y-1/2 text-zinc-500"
					aria-hidden="true"
				/>
				<Input
					bind:ref={inputRef}
					bind:value={query}
					class="h-10 border-0 bg-transparent pl-7 pr-2 text-lg text-zinc-50 shadow-none placeholder:text-zinc-500 focus-visible:ring-0 md:text-lg"
					placeholder="Alias or command..."
					spellcheck="false"
				/>
			</div>
			<Button
				variant="ghost"
				size="icon"
				class="text-zinc-400 hover:bg-white/8 hover:text-zinc-50"
				aria-label="Create alias"
				onclick={() => beginCreate(query)}
			>
				<Plus aria-hidden="true" />
			</Button>
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

		{#if mode === "search"}
			<div class="grid min-h-[230px] gap-1 px-2 py-2">
				{#each results() as result, index}
					{#if result.kind === "alias"}
						<div
							role="button"
							tabindex="-1"
							class="group grid h-[72px] grid-cols-[1fr_auto] items-center gap-3 rounded-md px-3 text-left transition-colors hover:bg-white/8 focus-visible:bg-white/10 focus-visible:outline-none {index ===
							selectedIndex
								? 'bg-white/10'
								: ''}"
							onmouseenter={() => (selectedIndex = index)}
							onclick={() => pasteAlias(result.alias)}
							onkeydown={(event) => {
								if (event.key === "Enter" || event.key === " ") {
									event.preventDefault();
									void pasteAlias(result.alias);
								}
							}}
						>
							<div class="min-w-0">
								<div class="flex items-center gap-2">
									<span class="font-mono text-base font-semibold tracking-normal text-zinc-100">
										{result.alias.key}
									</span>
									{#if result.exact}
										<Badge class="bg-cyan-300 text-zinc-950">exact</Badge>
									{/if}
								</div>
								<div class="truncate font-mono text-xs text-zinc-400">{result.alias.expansion}</div>
								{#if result.alias.description}
									<div class="truncate text-xs text-zinc-600">{result.alias.description}</div>
								{/if}
							</div>
							<div class="flex items-center gap-2">
								<Button
									variant="ghost"
									size="icon"
									class="size-8 text-zinc-600 hover:bg-white/8 hover:text-cyan-200"
									aria-label={`Copy ${result.alias.key}`}
									onclick={(event) => {
										event.stopPropagation();
										void copyAlias(result.alias);
									}}
								>
									<Clipboard class="size-4" aria-hidden="true" />
								</Button>
							</div>
						</div>
					{:else}
						<button
							type="button"
							class="group grid h-[72px] grid-cols-[1fr_auto] items-center gap-3 rounded-md px-3 text-left transition-colors hover:bg-white/8 focus-visible:bg-white/10 focus-visible:outline-none {index ===
							selectedIndex
								? 'bg-white/10'
								: ''}"
							onmouseenter={() => (selectedIndex = index)}
							onclick={() => beginCreate(result.key, result.expansion)}
						>
							<div class="min-w-0">
								<div class="font-mono text-base font-semibold tracking-normal text-zinc-100">
									{result.key || "new-alias"}
								</div>
								<div class="truncate text-xs text-zinc-500">
									{result.expansion || "Create a lookup-table alias"}
								</div>
							</div>
							<Badge variant="outline" class="border-cyan-300/30 bg-cyan-300/10 text-cyan-100">
								create
							</Badge>
						</button>
					{/if}
				{:else}
					<div class="flex h-[216px] items-center justify-center px-6 text-center text-sm text-zinc-500">
						No aliases yet.
					</div>
				{/each}
			</div>
		{:else}
			<form class="grid min-h-[230px] gap-3 px-4 py-4" onsubmit={(event) => event.preventDefault()}>
				<div class="grid grid-cols-[170px_1fr] gap-3">
					<label class="grid gap-1 text-xs font-medium text-zinc-500">
						Alias
						<Input
							bind:value={draftKey}
							class="h-9 border-white/10 bg-white/[0.04] font-mono text-sm text-zinc-50"
							placeholder="gsuir"
							spellcheck="false"
						/>
					</label>
					<label class="grid gap-1 text-xs font-medium text-zinc-500">
						Tags
						<Input
							bind:value={draftTags}
							class="h-9 border-white/10 bg-white/[0.04] text-sm text-zinc-50"
							placeholder="git, submodule"
							spellcheck="false"
						/>
					</label>
				</div>
				<label class="grid gap-1 text-xs font-medium text-zinc-500">
					Expansion
					<textarea
						bind:this={expansionRef}
						bind:value={draftExpansion}
						class="min-h-20 w-full resize-none rounded-md border border-white/10 bg-white/[0.04] px-3 py-2 font-mono text-sm text-zinc-50 outline-none transition-colors placeholder:text-zinc-600 focus:border-cyan-300/50 focus:ring-2 focus:ring-cyan-300/20"
						placeholder="git submodule update --init --recursive"
						spellcheck="false"
					></textarea>
				</label>
				<label class="grid gap-1 text-xs font-medium text-zinc-500">
					Description
					<Input
						bind:value={draftDescription}
						class="h-9 border-white/10 bg-white/[0.04] text-sm text-zinc-50"
						placeholder="Initialize and recursively update git submodules"
						spellcheck="false"
					/>
				</label>
				<div class="flex justify-end gap-2">
					<Button variant="ghost" class="text-zinc-400 hover:bg-white/8" onclick={cancelDraft}>
						<X aria-hidden="true" />
						Cancel
					</Button>
					<Button class="bg-cyan-300 text-zinc-950 hover:bg-cyan-200" onclick={saveDraft}>
						<Check aria-hidden="true" />
						Save
					</Button>
				</div>
			</form>
		{/if}

		<Separator class="bg-white/10" />

		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<footer
			data-tauri-drag-region="deep"
			class="flex h-10 items-center justify-between px-4 text-[11px] text-zinc-500"
			onmousedown={handleLauncherDragMouseDown}
		>
			<span>{status}</span>
			<div class="flex items-center gap-2">
				<Badge variant="outline" class="border-white/10 bg-white/[0.04] text-zinc-500">Enter paste</Badge>
				<Badge variant="outline" class="border-white/10 bg-white/[0.04] text-zinc-500">Tab edit</Badge>
				<Badge variant="outline" class="border-white/10 bg-white/[0.04] text-zinc-500">
					⌘N create
				</Badge>
			</div>
		</footer>
	</section>
</main>

<style>
	.launcher-shell {
		opacity: 0;
		transform: translateY(-4px) scale(0.985);
		transform-origin: center top;
		transition:
			opacity 60ms ease-out,
			transform 60ms cubic-bezier(0.16, 1, 0.3, 1);
		will-change: opacity, transform;
	}

	.launcher-shell[data-launcher-stage="shown"] {
		opacity: 1;
		transform: translateY(0) scale(1);
	}

	.launcher-shell[data-launcher-stage="hiding"],
	.launcher-shell[data-launcher-stage="hidden"] {
		opacity: 0;
		transform: translateY(-2px) scale(0.992);
		transition-duration: 100ms;
		transition-timing-function: cubic-bezier(0.4, 0, 1, 1);
	}

	@media (prefers-reduced-motion: reduce) {
		.launcher-shell {
			transform: none;
			transition: none;
		}
	}
</style>
