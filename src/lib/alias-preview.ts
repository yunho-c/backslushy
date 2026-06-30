import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import python from "highlight.js/lib/languages/python";
import typescript from "highlight.js/lib/languages/typescript";
import yaml from "highlight.js/lib/languages/yaml";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("shell", bash);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("python", python);
hljs.registerLanguage("yaml", yaml);

const HIGHLIGHT_LANGUAGES = ["bash", "shell", "javascript", "typescript", "json", "python", "yaml"];

const COMMAND_PREFIXES = new Set([
	"awk",
	"bun",
	"cargo",
	"cat",
	"cd",
	"chmod",
	"chown",
	"code",
	"cp",
	"curl",
	"docker",
	"find",
	"git",
	"grep",
	"jq",
	"kubectl",
	"ln",
	"ls",
	"make",
	"mkdir",
	"mv",
	"npm",
	"npx",
	"pnpm",
	"python",
	"python3",
	"rg",
	"rm",
	"rsync",
	"sed",
	"ssh",
	"sudo",
	"tar",
	"touch",
	"vim",
	"yarn",
]);

const NATURAL_LANGUAGE_WORDS = new Set([
	"a",
	"an",
	"and",
	"are",
	"as",
	"for",
	"from",
	"in",
	"is",
	"it",
	"of",
	"on",
	"or",
	"that",
	"the",
	"this",
	"to",
	"with",
]);

export type AliasExpansionPreview = {
	codeLike: boolean;
	html: string;
	language?: string;
	relevance: number;
};

export function formatAliasExpansion(value: string): AliasExpansionPreview {
	const trimmed = value.trim();
	const highlight = hljs.highlightAuto(trimmed, HIGHLIGHT_LANGUAGES);
	const shellScore = scoreShellLikeText(trimmed);
	const language = highlight.language;
	const shellLike = shellScore >= 3;
	const codeLike = shellLike || highlight.relevance >= 5;

	if (!codeLike) {
		return {
			codeLike: false,
			html: "",
			language,
			relevance: highlight.relevance,
		};
	}

	const highlighted =
		shellLike || language === "shell"
			? hljs.highlight(trimmed, { language: "bash", ignoreIllegals: true })
			: highlight;

	return {
		codeLike: true,
		html: highlighted.value,
		language: highlighted.language,
		relevance: highlighted.relevance,
	};
}

function scoreShellLikeText(value: string) {
	if (!value) return 0;

	const tokens = value.split(/\s+/);
	const firstToken = firstCommandToken(tokens);
	let score = 0;

	if (COMMAND_PREFIXES.has(firstToken)) score += 3;
	if (/(^|\s)--?[A-Za-z0-9][\w-]*/.test(value)) score += 2;
	if (/(^|\s)(\.{1,2}\/|~\/|\/[\w.-])/.test(value)) score += 2;
	if (/(^|\s)[A-Za-z_][A-Za-z0-9_]*=/.test(value)) score += 2;
	if (/[|;&<>`$(){}[\]\\]/.test(value)) score += 2;
	if (/["']/.test(value)) score += 1;

	const lowerWords = value.toLowerCase().match(/[a-z]+/g) ?? [];
	const naturalWordCount = lowerWords.filter((word) => NATURAL_LANGUAGE_WORDS.has(word)).length;
	if (naturalWordCount >= 2) score -= 2;
	if (/^[A-Z][^.!?]*[.!?]$/.test(value)) score -= 2;

	return score;
}

function firstCommandToken(tokens: string[]) {
	const [first, second] = tokens;
	if ((first === "env" || first === "sudo") && second) return second.toLowerCase();
	return first?.toLowerCase() ?? "";
}
