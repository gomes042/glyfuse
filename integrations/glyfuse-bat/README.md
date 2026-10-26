# Glyfuse × bat — optional source patch

Upstream: [sharkdp/bat](https://github.com/sharkdp/bat), commit
`a9aa73d215455a479f072da75dd39f19826acfea` (v0.26.1),
licensed **MIT OR Apache-2.0**. The submodule is kept at its unmodified
upstream commit. The patch adds an independent Glyfuse format-1 JSON reader;
no dependency on the Glyfuse crate or executable is added.

The patch affects **only `StyleComponent::HeaderFilename`**. With
`BAT_GLYFUSE=1`, a Unicode icon plus a separating space precedes the header
filename, and only the icon is colored (24-bit RGB when supported, ANSI 256-color
approximation otherwise). The
filename remains uncolored; line numbers, syntax highlighting, and other
header components are unchanged. With `BAT_GLYFUSE` unset, or a missing or
invalid theme snapshot, original upstream behavior is preserved.

## Build patched bat

From the Glyfuse repository root:

```sh
git submodule update --init integrations/glyfuse-bat/upstream
cd integrations/glyfuse-bat/upstream
git apply --check ../patches/0001-glyfuse-bat-header-icon.patch
git apply ../patches/0001-glyfuse-bat-header-icon.patch
cargo test --lib
cargo build --release
```

In PowerShell, activate a snapshot with `glyfuse theme use material`, then:

```powershell
$env:BAT_GLYFUSE = '1'
./target/release/bat.exe --style=header --color=always --terminal-width=100 ./Cargo.toml
```

On Unix, export `BAT_GLYFUSE=1` and use
`./target/release/bat --style=header --color=always Cargo.toml`.
If bat otherwise omits the header (for example, its default style for a
single file), explicitly set `--style=header`.

`BAT_GLYFUSE_THEME` overrides the JSON file location. Otherwise the reader
tries `GLYFUSE_THEME` (file path), `GLYFUSE_HOME/active.json`, and finally
Glyfuse's platform-standard active snapshot location. `GLYFUSE_THEME=material`
is understood by Glyfuse CLI but **not** by this standalone reader; activate
via `glyfuse theme use material`. The snapshot must have `format: 1`,
flattened inheritance (`inherits` absent), and valid icon colors.
Themes load once per process. The patched binary is separate from stock bat.

## Reset and refresh patches

To return the submodule to the pinned, clean upstream state **after applying**
the patch (assuming no additional personal edits inside the submodule):

```sh
cd integrations/glyfuse-bat/upstream
git apply -R ../patches/0001-glyfuse-bat-header-icon.patch
```

For a new upstream bat version, choose and review an upstream commit in the
submodule, then try `git apply --check` with the current patch. If it does not
apply cleanly, port changes to the new APIs and rerun build/tests and manual
header output checks. To regenerate the patch from a *clean upstream checkout*
with only Glyfuse modifications in the working tree:

```sh
git add -N src/glyfuse.rs
git diff --binary --output=../patches/0001-glyfuse-bat-header-icon.patch -- \
  Cargo.toml Cargo.lock src/lib.rs src/printer.rs src/glyfuse.rs
```

The `git add -N` command marks the new file intent-to-add so the diff includes
it. Check the new patch with `git apply --reverse --check` in the modified tree,
then reverse it and check forward application again. Commit or tag changes
only as part of your own reviewed release process; no fork is published here.
