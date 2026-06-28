# bkslash

A desktop-only Tauri 2 launcher scaffold using SvelteKit, Tailwind CSS v4, and shadcn-svelte.

The app starts with its main window hidden. Press the backslash key (`\`) to toggle the launcher, and press `Esc` inside the launcher to hide it again.

## Development

```sh
bun install
bun run desktop:dev
```

## Checks

```sh
bun run check
bun run build
env RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo check --manifest-path src-tauri/Cargo.toml
env RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= bun run tauri build --no-bundle
```

The explicit empty `RUSTC_WRAPPER` variables avoid local `sccache` permission failures on this machine.
