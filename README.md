# bkslash

A desktop-only Tauri 2 lookup-table alias launcher using SvelteKit, Tailwind CSS v4, and shadcn-svelte.

The app starts with its main window hidden. Press the backslash key (`\`) to toggle the launcher, and press `Esc` inside the launcher to hide it again.

## Alias Workflow

Type an alias such as `gsuir` to reveal its expansion, then press `Enter` to paste it into the previously focused app. Unknown aliases can be created in place with `Cmd+N` or the `+` button. Select an existing alias and press `Tab` to edit it.

On macOS, paste injection uses the system pasteboard plus a synthetic `Cmd+V` event. Clipboard contents are restored by default after the paste dispatch. macOS Accessibility permission is required for the paste event; the inline copy button still works without it.

Aliases are stored locally in browser storage for now. The default set includes:

```text
gsuir -> git submodule update --init --recursive
```

## Development

```sh
bun install
bun run desktop:dev
```

To capture macOS focus diagnostics while debugging launcher activation and paste
restore behavior:

```sh
bun run desktop:dev:focus
```

Diagnostic events are written to the dev terminal with the `[bkslash:focus]`
prefix from both the native AppKit layer and the frontend.

## Checks

```sh
bun run check
bun run build
env RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo check --manifest-path src-tauri/Cargo.toml
env RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= bun run tauri build --no-bundle
```

The explicit empty `RUSTC_WRAPPER` variables avoid local `sccache` permission failures on this machine.
