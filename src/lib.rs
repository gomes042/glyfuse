//! Unicode glyph and RGB color resolution over portable, versioned JSON themes.
//! No font probing, SVG rendering, or terminal I/O is performed by this library.

use anyhow::{Context, Result, anyhow, bail};
use globset::{GlobBuilder, GlobMatcher};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const MATERIAL: &str = include_str!("../themes/material.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Icon {
    pub glyph: String,
    pub color: String,
}

impl Icon {
    fn validate(&self) -> Result<()> {
        if self.glyph.is_empty() || self.glyph.chars().any(char::is_control) {
            bail!("glyph must contain printable Unicode text");
        }
        let color = self.color.as_bytes();
        if color.len() != 7 || color[0] != b'#' || !color[1..].iter().all(u8::is_ascii_hexdigit) {
            bail!("invalid RGB color {:?}: expected #RRGGBB", self.color);
        }
        Ok(())
    }

    pub fn rgb(&self) -> (u8, u8, u8) {
        let n = u32::from_str_radix(&self.color[1..], 16).expect("validated color");
        ((n >> 16) as u8, (n >> 8) as u8, n as u8)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fallbacks {
    pub file: Icon,
    pub folder: Icon,
    #[serde(default)]
    pub folder_expanded: Option<Icon>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    File,
    Folder,
    Extension,
    Path,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub kind: Kind,
    pub pattern: String,
    pub icon: Icon,
    #[serde(default)]
    pub expanded: Option<Icon>,
    /// For attribution and editor display; not used for matching.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub format: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallbacks: Option<Fallbacks>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl Theme {
    pub fn parse(json: &str) -> Result<Self> {
        let theme: Self = serde_json::from_str(json).context("invalid Glyfuse theme JSON")?;
        theme.validate()?;
        Ok(theme)
    }

    pub fn validate(&self) -> Result<()> {
        if self.format != 1 {
            bail!("unsupported theme format {}; expected 1", self.format);
        }
        if self.name.trim().is_empty() {
            bail!("theme name cannot be empty");
        }
        if let Some(f) = &self.fallbacks {
            f.file.validate()?;
            f.folder.validate()?;
            if let Some(icon) = &f.folder_expanded {
                icon.validate()?;
            }
        } else if self.inherits.is_none() {
            bail!("base themes must define fallbacks");
        }
        for (i, rule) in self.rules.iter().enumerate() {
            if rule.pattern.trim().is_empty() {
                bail!("rule {i}: empty pattern");
            }
            rule.icon.validate().with_context(|| format!("rule {i}"))?;
            if let Some(expanded) = &rule.expanded {
                if rule.kind != Kind::Folder {
                    bail!("rule {i}: expanded only applies to folders");
                }
                expanded
                    .validate()
                    .with_context(|| format!("rule {i} expanded"))?;
            }
            if rule.kind == Kind::Path {
                GlobBuilder::new(&rule.pattern)
                    .case_insensitive(true)
                    .build()
                    .with_context(|| format!("rule {i}: invalid path glob"))?;
            }
        }
        Ok(())
    }

    /// Inheritance is resolved and flattened before integrations consume the snapshot.
    pub fn load(selection: &str) -> Result<Self> {
        load_inner(selection, None, &mut HashSet::new())
    }

    pub fn active() -> Result<Self> {
        if let Ok(path) = env::var("GLYFUSE_THEME") {
            return Self::load(&path);
        }
        let path = active_path()?;
        if path.exists() {
            return Self::load(&path.to_string_lossy());
        }
        Self::load("material")
    }
}

fn load_inner(
    selection: &str,
    relative_to: Option<&Path>,
    seen: &mut HashSet<String>,
) -> Result<Theme> {
    let material = selection == "material";
    let file = if material {
        None
    } else {
        let p = Path::new(selection);
        Some(if p.is_absolute() {
            p.to_path_buf()
        } else if let Some(base) = relative_to {
            base.join(p)
        } else {
            p.to_path_buf()
        })
    };
    let key = if let Some(path) = &file {
        path.canonicalize()
            .with_context(|| format!("theme {}", path.display()))?
            .display()
            .to_string()
    } else {
        "builtin:material".into()
    };
    if !seen.insert(key.clone()) {
        bail!("cyclic theme inheritance at {key}");
    }
    let result = (|| -> Result<Theme> {
        let contents = if let Some(path) = &file {
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?
        } else {
            MATERIAL.to_string()
        };
        let child = Theme::parse(&contents)?;
        if let Some(parent_name) = child.inherits.as_deref() {
            let parent_dir = file.as_ref().and_then(|p| p.parent());
            let mut parent = load_inner(parent_name, parent_dir, seen)?;
            if let Some(fallbacks) = child.fallbacks {
                parent.fallbacks = Some(fallbacks);
            }
            parent.rules.extend(child.rules);
            parent.name = child.name;
            parent.validate()?;
            Ok(parent)
        } else {
            Ok(child)
        }
    })();
    seen.remove(&key);
    result.map(|mut t| {
        t.inherits = None;
        t
    })
}

pub fn config_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("GLYFUSE_HOME") {
        return Ok(PathBuf::from(path));
    }
    if cfg!(windows) {
        Ok(
            PathBuf::from(env::var_os("APPDATA").ok_or_else(|| anyhow!("APPDATA missing"))?)
                .join("glyfuse"),
        )
    } else if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        Ok(PathBuf::from(path).join("glyfuse"))
    } else {
        Ok(
            PathBuf::from(env::var_os("HOME").ok_or_else(|| anyhow!("HOME missing"))?)
                .join(".config/glyfuse"),
        )
    }
}

pub fn active_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("active.json"))
}

pub fn use_theme(theme: &Theme) -> Result<PathBuf> {
    theme.validate()?;
    let path = active_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("invalid active theme path"))?;
    fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(theme)?;
    let temp = parent.join(format!(".active-{}.tmp", std::process::id()));
    fs::write(&temp, bytes).with_context(|| format!("writing {}", temp.display()))?;
    if let Err(err) = fs::rename(&temp, &path) {
        // Windows does not replace an existing destination via rename.
        if path.exists() {
            fs::remove_file(&path)?;
            fs::rename(&temp, &path)?;
        } else {
            return Err(err.into());
        }
    }
    Ok(path)
}

pub struct Resolver {
    pub theme: Theme,
    files: HashMap<String, usize>,
    folders: HashMap<String, usize>,
    extensions: HashMap<String, usize>,
    glob_rules: Vec<(usize, GlobMatcher)>,
}

impl Resolver {
    pub fn new(theme: Theme) -> Result<Self> {
        theme.validate()?;
        let (mut files, mut folders, mut extensions, mut glob_rules) =
            (HashMap::new(), HashMap::new(), HashMap::new(), Vec::new());
        for (index, rule) in theme.rules.iter().enumerate() {
            let key = rule.pattern.to_lowercase();
            match rule.kind {
                Kind::File => {
                    files.insert(key, index);
                }
                Kind::Folder => {
                    folders.insert(key, index);
                }
                Kind::Extension => {
                    extensions.insert(key, index);
                }
                Kind::Path => {
                    glob_rules.push((
                        index,
                        GlobBuilder::new(&rule.pattern)
                            .case_insensitive(true)
                            .build()?
                            .compile_matcher(),
                    ));
                }
            }
        }
        Ok(Self {
            theme,
            files,
            folders,
            extensions,
            glob_rules,
        })
    }

    /// Paths use forward slashes logically (backslashes accepted). Path rules
    /// outrank exact filenames/folders, which outrank longest compound extension.
    /// Later rules win within equal precedence, allowing overrides after inheritance.
    pub fn resolve(&self, path: &str, is_dir: bool, expanded: bool) -> &Icon {
        let path = path.replace('\\', "/");
        let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        let name_lower = name.to_lowercase();
        let glob = self
            .glob_rules
            .iter()
            .rev()
            .find(|(_, matcher)| matcher.is_match(&path))
            .map(|(index, _)| *index);
        let exact = if is_dir {
            self.folders.get(&name_lower)
        } else {
            self.files.get(&name_lower)
        }
        .copied();
        let extension = if is_dir {
            None
        } else {
            name_lower
                .match_indices('.')
                .filter_map(|(dot, _)| {
                    self.extensions
                        .get(&name_lower[dot + 1..])
                        .map(|index| (name_lower.len() - dot, *index))
                })
                .max()
                .map(|(_, index)| index)
        };
        if let Some(index) = glob.or(exact).or(extension) {
            let rule = &self.theme.rules[index];
            return if expanded {
                rule.expanded.as_ref().unwrap_or(&rule.icon)
            } else {
                &rule.icon
            };
        }
        let f = self.theme.fallbacks.as_ref().expect("validated theme");
        if is_dir {
            if expanded {
                f.folder_expanded.as_ref().unwrap_or(&f.folder)
            } else {
                &f.folder
            }
        } else {
            &f.file
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_folder_and_file_rules() {
        let resolver = Resolver::new(Theme::load("material").unwrap()).unwrap();
        let src = resolver.resolve("src", true, false);
        assert_eq!(src.color.to_lowercase(), "#4caf50");
        assert_ne!(src.color, resolver.resolve("schemas", true, false).color);
        assert_ne!(
            resolver.resolve("README.md", false, false).color,
            resolver.resolve("file.unknownthing", false, false).color
        );
        assert_eq!(
            resolver.resolve("C:\\repo\\SRC", true, false).glyph,
            src.glyph
        );
    }

    #[test]
    fn validation_and_precedence() {
        let mut t = Theme::load("material").unwrap();
        assert!(t.rules.len() > 100);
        let glyph = |s: &str| Icon {
            glyph: s.into(),
            color: "#abcdef".into(),
        };
        t.rules.push(Rule {
            kind: Kind::Extension,
            pattern: "schema.json".into(),
            icon: glyph("E"),
            expanded: None,
            source_icon: None,
        });
        t.rules.push(Rule {
            kind: Kind::File,
            pattern: "special.schema.json".into(),
            icon: glyph("F"),
            expanded: None,
            source_icon: None,
        });
        t.rules.push(Rule {
            kind: Kind::Path,
            pattern: "**/model.schema.json".into(),
            icon: glyph("P"),
            expanded: None,
            source_icon: None,
        });
        let r = Resolver::new(t).unwrap();
        assert_eq!(r.resolve("a/model.schema.json", false, false).glyph, "P");
        assert_eq!(r.resolve("special.schema.json", false, false).glyph, "F");
        assert_eq!(r.resolve("a/new.schema.json", false, false).glyph, "E");
        assert!(Theme::parse(r##"{"format":1,"name":"bad","fallbacks":{"file":{"glyph":"x","color":"bad"},"folder":{"glyph":"x","color":"#ffffff"}}}"##).is_err());
    }

    #[test]
    fn inheritance_and_cycles() {
        let dir = std::env::temp_dir().join(format!("glyfuse-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.json");
        let b = dir.join("b.json");
        fs::write(&a, r#"{"format":1,"name":"A","inherits":"b.json"}"#).unwrap();
        fs::write(&b, r#"{"format":1,"name":"B","inherits":"a.json"}"#).unwrap();
        assert!(
            Theme::load(&a.to_string_lossy())
                .unwrap_err()
                .to_string()
                .contains("cyclic")
        );
        fs::write(&b, r#"{"format":1,"name":"B","inherits":"material"}"#).unwrap();
        assert!(Theme::load(&a.to_string_lossy()).unwrap().rules.len() > 100);
        fs::remove_dir_all(dir).unwrap();
    }
}
