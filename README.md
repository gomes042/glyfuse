# Glyfuse

**Shared Unicode glyph + RGB color themes for terminal tools.** Glyfuse is a Rust resolver library and CLI with configurable JSON themes, a live inspector, and integrations that can read a common theme snapshot without depending on its executable.

Glyfuse does not render SVGs, install fonts, or detect which glyphs your terminal font supports. The bundled Material-inspired glyphs are approximate Nerd Font symbols; use a [Nerd Font](https://www.nerdfonts.com/) or `--ascii` in CLI/inspector.

## Feature status

- **Implemented:** Rust library, CLI `theme list/use/show`, `resolve`, text and interactive TUI `inspect`; versioned themes with inheritance, overrides, path globs, and fallbacks.
- **Implemented:** bundled 3,550 Material Icon Theme-derived associations (963 folders, 1,148 filenames, 1,439 extensions). Associations and sampled SVG colors come from upstream; Unicode glyphs are approximations.
- **Implemented separately:** LazySC's optional built-in JSON reader and a source patch for bat's filename header icon. Stock bat is unchanged.
- **Planned only:** Neovim tree/explorer integrations, SVG display, font probing, automatic synchronization, package release.

## Build and CLI

Requires a recent stable Rust/Cargo toolchain, and an ANSI/Unicode terminal for visual output.

```sh
cargo test --all-targets
cargo build --release
cargo install --path .
glyfuse --help
glyfuse theme list
glyfuse theme show
glyfuse theme use material
glyfuse theme use ./my-theme.json
glyfuse resolve src --folder --json
glyfuse resolve README.md --file
glyfuse resolve archive.tar.gz --json
glyfuse resolve unknown.xyz --ascii --no-color
glyfuse inspect --ascii
glyfuse inspect --tui
glyfuse --theme material resolve src --folder --json
```

Built-in Material data is embedded in the executable. An existing directory or trailing slash implies folder matching; `--folder` / `--file` overrides detection. `--expanded` selects expanded folder icons; `--json` emits only an icon object. Normal interactive-terminal output colors the glyph and leaves path text uncolored.

In `inspect --tui`, type a path for a live matching preview: `src/` is a folder; `src/a.exe` is a file. A trailing `/` or `\` selects folder matching automatically. `Tab` switches keyboard focus between the editable path and the complete association list. With the list focused, arrows/Page Up/Down/Home/End navigate; the mouse wheel and draggable scrollbar scroll through all rules. `Enter` or clicking an association copies its sample path to the input. In the path input, Left/Right/Home/End, Backspace, and Delete edit the text. `Ctrl+E` toggles expanded folder icons; `Esc` or `Ctrl+C` exits. `--ascii` replaces glyph display with generic ASCII markers.

## Theme JSON format (v1)

UTF-8 JSON. Glyphs contain printable Unicode; colors are `#RRGGBB`. Custom override example:

```json
{
  "format": 1,
  "name": "My theme",
  "inherits": "material",
  "rules": [
    {"kind":"folder", "pattern":"assets", "icon":{"glyph":"★","color":"#55CC99"}, "expanded":{"glyph":"☆","color":"#55CC99"}},
    {"kind":"file", "pattern":"Cargo.toml", "icon":{"glyph":"R","color":"#FF7043"}},
    {"kind":"extension", "pattern":"test.ts", "icon":{"glyph":"T","color":"#3178C6"}},
    {"kind":"path", "pattern":"**/generated/*.rs", "icon":{"glyph":"G","color":"#AAAAAA"}}
  ]
}
```

A base theme without `inherits` must add a `fallbacks` object like the following **field value**:

```json
{
  "file": {"glyph":"?", "color":"#B0BEC5"},
  "folder": {"glyph":"+", "color":"#90A4AE"},
  "folder_expanded": {"glyph":"-", "color":"#90A4AE"}
}
```

`folder_expanded` is optional. Rules may provide `source_icon` as descriptive attribution metadata. Resolution priority: **path glob > exact filename/folder > longest compound extension > fallback**; later duplicates take precedence. Matching is case-insensitive; backslashes are normalized; extension patterns omit the leading dot (`tar.gz`). Path globs use `globset` syntax and are matched against the supplied path. `inherits` accepts `material` or a file path; relative parent paths resolve against the child JSON file. Cycles, malformed colors, invalid glob expressions, and control characters in glyphs are rejected.

`theme use` flattens inheritance and saves a fully resolved shared `active.json`:

| OS | Location |
| --- | --- |
| Windows | `%APPDATA%\glyfuse\active.json` |
| Linux/Unix | `${XDG_CONFIG_HOME:-$HOME/.config}/glyfuse/active.json` |

Override the parent directory with `GLYFUSE_HOME`. `GLYFUSE_THEME=/absolute/path/to/flattened.json` selects a specific snapshot; the CLI also accepts `GLYFUSE_THEME=material`, but standalone integrations expect an actual JSON path. Prefer `glyfuse theme use material` to activate a shared preset. Restart integrations after changes to the snapshot.

## Integrations

| Application | Status | Enablement | Glyfuse runtime dependency? |
| --- | --- | --- | --- |
| Glyfuse CLI/library | Implemented | Built in | Native component |
| **LazySC** | **Built-in, opt-in** | `[source_control] glyfuse_icons = true` in LazySC config; activate snapshot, restart | **No**; reads JSON independently |
| **bat** | **Patched source only** | Apply `glyfuse-bat/patches/0001-glyfuse-bat-header-icon.patch`, rebuild, `BAT_GLYFUSE=1` | **No**; patched bat reads JSON independently |
| Neovim file trees and other terminal tools | Future | No integration shipped | TBD |

LazySC colors folder and file icons in its source-control sidebar without changing its Git status display or navigation. Missing/invalid snapshots leave its original icon-free UI. See the [LazySC README](../lazysc/README.md) for configuration.

The modified bat colors **only the icon prefix before the header filename**. The filename itself is plain/uncolored; body syntax highlighting stays upstream. If disabled or no valid snapshot is found, the usual bat header remains. **Unmodified bat does not support Glyfuse.** See the [bat patch guide](glyfuse-bat/README.md) for build, patch, verification and update steps.

## Material provenance and license

The [bundled Material preset](themes/material.json) and [import script](scripts/import_material.mjs) adapt definitions and color values from [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme), copyright © 2025 Material Extensions, licensed MIT, pinned at commit `1a9d4b3d478d7fd1c49a86ea371ec6a8b505b097`. SVG files are read for color sampling during import, **never shipped or rendered**; many associations use generic approximated symbols. Some sampled colors can differ from upstream's composited SVG appearance. See [third-party license](THIRD_PARTY_LICENSES.md).

Regenerate deliberately by checking out Material Icon Theme separately and running `node scripts/import_material.mjs PATH_TO_CHECKOUT`. Review the changes, then run tests. Original Glyfuse code is [MIT](LICENSE). The separate bat submodule is licensed **MIT OR Apache-2.0** and retains upstream license files. No binaries have been published.
