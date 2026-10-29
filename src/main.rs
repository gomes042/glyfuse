mod eza;
mod tui;

use anyhow::{Context, Result, bail};
use glyfuse::{Kind, Resolver, Theme, use_theme};
use std::env;
use std::io::Write;
use std::io::{self, IsTerminal};

const HELP: &str = r#"glyfuse — shared terminal icon themes (glyph + RGB color)

Usage:
  glyfuse theme list
  glyfuse theme use material|PATH.json
  glyfuse theme show
  glyfuse resolve PATH [--folder|--file] [--expanded] [--json] [--ascii] [--no-color]
  glyfuse inspect [--tui] [--ascii]
  glyfuse export eza [--output PATH] [--force]

Global:
  --theme material|PATH.json   Select theme for this invocation
  --ascii                      Display portable ASCII markers; preserve RGB metadata

Default: active theme in GLYFUSE_HOME/active.json (or platform config dir),
then built-in material. GLYFUSE_THEME overrides the active selection.
`theme use` resolves inheritance and writes active.json for other applications.
No font is detected; install a compatible Nerd Font or use --ascii.
"#;

fn main() -> Result<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(());
    }
    let explicit = take_option(&mut args, "--theme")?;
    let ascii = take_flag(&mut args, "--ascii");
    let selected = if let Some(choice) = explicit {
        Theme::load(&choice)?
    } else {
        Theme::active()?
    };
    let cmd = args.remove(0);
    match cmd.as_str() {
        "theme" => {
            if args.is_empty() {
                bail!("expected theme subcommand: list, use, show");
            }
            match args.remove(0).as_str() {
                "list" if args.is_empty() => {
                    println!("material  (built-in)\ncustom    (PATH.json / GLYFUSE_THEME)")
                }
                "use" if args.len() == 1 => {
                    let theme = Theme::load(&args[0])?;
                    let path = use_theme(&theme)?;
                    println!("Activated {} at {}", theme.name, path.display());
                }
                "show" if args.is_empty() => {
                    println!("{}", serde_json::to_string_pretty(&selected)?);
                }
                _ => bail!("expected theme list, theme use PATH, or theme show"),
            }
        }
        "resolve" => {
            let is_dir = take_flag(&mut args, "--folder");
            let file = take_flag(&mut args, "--file");
            let expanded = take_flag(&mut args, "--expanded");
            let json = take_flag(&mut args, "--json");
            let no_color = take_flag(&mut args, "--no-color");
            if is_dir && file {
                bail!("choose --folder or --file");
            }
            if args.len() != 1 {
                bail!("resolve requires one file or folder path");
            }
            let path = &args[0];
            let folder = if is_dir {
                true
            } else if file {
                false
            } else {
                std::path::Path::new(path).is_dir() || path.ends_with(['/', '\\'])
            };
            let resolver = Resolver::new(selected)?;
            let icon = resolver.resolve(path, folder, expanded);
            if json {
                println!("{}", serde_json::to_string(icon)?);
            } else {
                let glyph = if ascii {
                    if folder { "[D]" } else { "[F]" }
                } else {
                    &icon.glyph
                };
                let (r, g, b) = icon.rgb();
                if io::stdout().is_terminal() && !no_color {
                    println!("\x1b[38;2;{r};{g};{b}m{glyph}\x1b[0m {path}");
                } else {
                    println!("{glyph} {path}");
                }
            }
        }
        "inspect" => {
            let tui = take_flag(&mut args, "--tui");
            if !args.is_empty() {
                bail!("unexpected inspect arguments: {args:?}");
            }
            let resolver = Resolver::new(selected)?;
            if tui {
                tui::run(resolver, ascii).context("running icon theme inspector")?;
            } else {
                println!(
                    "{} — {} rules",
                    resolver.theme.name,
                    resolver.theme.rules.len()
                );
                for rule in &resolver.theme.rules {
                    let kind = match rule.kind {
                        Kind::File => "file",
                        Kind::Folder => "folder",
                        Kind::Extension => "extension",
                        Kind::Path => "path",
                    };
                    let glyph = if ascii {
                        if rule.kind == Kind::Folder {
                            "[D]"
                        } else {
                            "[F]"
                        }
                    } else {
                        &rule.icon.glyph
                    };
                    println!("{kind:10} {:30} {glyph} {}", rule.pattern, rule.icon.color);
                }
            }
        }
        "export" => {
            let output = take_option(&mut args, "--output")?;
            let force = take_flag(&mut args, "--force");
            if args != ["eza"] || (force && output.is_none()) {
                bail!("expected export eza [--output PATH] [--force]");
            }
            let yaml = eza::render(&selected);
            if let Some(path) = output {
                let mut opts = std::fs::OpenOptions::new();
                opts.write(true);
                if force {
                    opts.create(true).truncate(true);
                } else {
                    opts.create_new(true);
                }
                let mut file = opts
                    .open(&path)
                    .with_context(|| format!("opening {path} (existing files require --force)"))?;
                file.write_all(yaml.as_bytes())?;
                println!("Exported eza theme to {path}");
            } else {
                print!("{yaml}");
            }
        }
        _ => bail!("unknown command {cmd:?}; see glyfuse --help"),
    }
    Ok(())
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|a| a == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn take_option(args: &mut Vec<String>, flag: &str) -> Result<Option<String>> {
    if let Some(index) = args.iter().position(|a| a == flag) {
        args.remove(index);
        if index >= args.len() || args[index].starts_with("--") {
            bail!("{flag} requires a value");
        }
        Ok(Some(args.remove(index)))
    } else {
        Ok(None)
    }
}
