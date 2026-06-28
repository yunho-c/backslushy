export type AliasKind = "shell" | "text" | "url" | "template";

export type AliasRecord = {
	id: string;
	key: string;
	expansion: string;
	description: string;
	kind: AliasKind;
	tags: string[];
	usageCount: number;
	updatedAt: string;
};

const STORAGE_KEY = "bkslash:aliases:v1";

const nowIso = () => new Date().toISOString();

function createId() {
	return globalThis.crypto?.randomUUID?.() ?? `alias-${Date.now()}-${Math.random()}`;
}

export const DEFAULT_ALIASES: AliasRecord[] = [
	{
		id: "default-gsuir",
		key: "gsuir",
		expansion: "git submodule update --init --recursive",
		description: "Initialize and recursively update git submodules",
		kind: "shell",
		tags: ["git", "submodule"],
		usageCount: 0,
		updatedAt: nowIso(),
	},
	{
		id: "default-gcam",
		key: "gcam",
		expansion: 'git commit -am ""',
		description: "Commit all tracked changes with a message",
		kind: "shell",
		tags: ["git", "commit"],
		usageCount: 0,
		updatedAt: nowIso(),
	},
	{
		id: "default-gpf",
		key: "gpf",
		expansion: "git push --force-with-lease",
		description: "Force push with lease protection",
		kind: "shell",
		tags: ["git", "push"],
		usageCount: 0,
		updatedAt: nowIso(),
	},
];

export function makeAlias(
	key: string,
	expansion: string,
	description = "",
	tags: string[] = [],
	kind: AliasKind = "shell"
): AliasRecord {
	return {
		id: createId(),
		key: normalizeAliasKey(key),
		expansion: expansion.trim(),
		description: description.trim(),
		kind,
		tags: normalizeTags(tags),
		usageCount: 0,
		updatedAt: nowIso(),
	};
}

export function normalizeAliasKey(key: string) {
	return key.trim().replace(/\s+/g, "").toLowerCase();
}

export function normalizeTags(tags: string[]) {
	return Array.from(
		new Set(tags.map((tag) => tag.trim().toLowerCase()).filter((tag) => tag.length > 0))
	);
}

export function parseTags(value: string) {
	return normalizeTags(value.split(","));
}

export function suggestAliasKey(expansion: string) {
	const words = expansion
		.trim()
		.split(/\s+/)
		.map((word) => word.replace(/^-+/, ""))
		.filter(Boolean);

	return words
		.slice(0, 6)
		.map((word) => word[0]?.toLowerCase())
		.join("");
}

export function loadAliases() {
	if (typeof localStorage === "undefined") {
		return DEFAULT_ALIASES;
	}

	const stored = localStorage.getItem(STORAGE_KEY);

	if (!stored) {
		return DEFAULT_ALIASES;
	}

	try {
		const parsed = JSON.parse(stored) as AliasRecord[];
		const aliases = parsed.filter(isAliasRecord);
		const keys = new Set(aliases.map((alias) => alias.key));
		const missingDefaults = DEFAULT_ALIASES.filter((alias) => !keys.has(alias.key));
		return [...aliases, ...missingDefaults];
	} catch {
		return DEFAULT_ALIASES;
	}
}

export function saveAliases(aliases: AliasRecord[]) {
	if (typeof localStorage === "undefined") {
		return;
	}

	localStorage.setItem(STORAGE_KEY, JSON.stringify(aliases));
}

function isAliasRecord(value: AliasRecord) {
	return (
		typeof value?.id === "string" &&
		typeof value.key === "string" &&
		typeof value.expansion === "string" &&
		Array.isArray(value.tags)
	);
}
