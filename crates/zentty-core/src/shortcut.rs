use std::collections::{HashMap, HashSet};

use serde::Deserialize;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutKey {
    Character(char),
    Space,
    Delete,
    Return,
    Tab,
    Left,
    Right,
    Up,
    Down,
}

impl ShortcutKey {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "space" => Some(Self::Space),
            "delete" => Some(Self::Delete),
            "return" | "enter" => Some(Self::Return),
            "tab" => Some(Self::Tab),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            value => {
                let mut characters = value.chars();
                let character = characters.next()?;
                (characters.next().is_none()).then_some(Self::Character(character))
            }
        }
    }

    fn storage_token(&self) -> String {
        match self {
            Self::Character(character) => character.to_lowercase().to_string(),
            Self::Space => "space".into(),
            Self::Delete => "delete".into(),
            Self::Return => "return".into(),
            Self::Tab => "tab".into(),
            Self::Left => "left".into(),
            Self::Right => "right".into(),
            Self::Up => "up".into(),
            Self::Down => "down".into(),
        }
    }

    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Character(character) => character.to_uppercase().to_string(),
            Self::Space => "Space".into(),
            Self::Delete => "Delete".into(),
            Self::Return => "Return".into(),
            Self::Tab => "Tab".into(),
            Self::Left => "Left".into(),
            Self::Right => "Right".into(),
            Self::Up => "Up".into(),
            Self::Down => "Down".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShortcutModifier {
    Command,
    Control,
    Option,
    Shift,
}

impl ShortcutModifier {
    const STORAGE_ORDER: [Self; 4] = [Self::Command, Self::Control, Self::Option, Self::Shift];

    fn parse(token: &str) -> Option<Self> {
        match token {
            "command" => Some(Self::Command),
            "control" => Some(Self::Control),
            "option" => Some(Self::Option),
            "shift" => Some(Self::Shift),
            _ => None,
        }
    }

    const fn storage_token(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Control => "control",
            Self::Option => "option",
            Self::Shift => "shift",
        }
    }

    const fn display(self) -> &'static str {
        match self {
            Self::Command => "Ctrl",
            Self::Control => "Control",
            Self::Option => "Alt",
            Self::Shift => "Shift",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardShortcut {
    pub key: ShortcutKey,
    pub modifiers: HashSet<ShortcutModifier>,
}

impl std::hash::Hash for KeyboardShortcut {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        for modifier in ShortcutModifier::STORAGE_ORDER {
            self.modifiers.contains(&modifier).hash(state);
        }
    }
}

impl KeyboardShortcut {
    #[must_use]
    pub fn parse(storage: &str) -> Option<Self> {
        let tokens = storage
            .trim()
            .split('+')
            .map(|token| token.trim().to_lowercase())
            .collect::<Vec<_>>();
        let (key, modifier_tokens) = tokens.split_last()?;
        if key.is_empty() {
            return None;
        }
        let mut modifiers = HashSet::new();
        for token in modifier_tokens {
            if !modifiers.insert(ShortcutModifier::parse(token)?) {
                return None;
            }
        }
        Some(Self {
            key: ShortcutKey::parse(key)?,
            modifiers,
        })
    }

    #[must_use]
    pub fn storage_string(&self) -> String {
        ShortcutModifier::STORAGE_ORDER
            .into_iter()
            .filter(|modifier| self.modifiers.contains(modifier))
            .map(|modifier| modifier.storage_token().to_owned())
            .chain(std::iter::once(self.key.storage_token()))
            .collect::<Vec<_>>()
            .join("+")
    }

    #[must_use]
    pub fn display(&self) -> String {
        let mut tokens = ShortcutModifier::STORAGE_ORDER
            .into_iter()
            .filter(|modifier| self.modifiers.contains(modifier))
            .map(|modifier| modifier.display().to_owned())
            .collect::<Vec<_>>();
        tokens.push(self.key.display());
        tokens.join("+")
    }

    #[must_use]
    pub fn is_eligible_command_binding(&self) -> bool {
        self.modifiers.iter().any(|modifier| {
            matches!(
                modifier,
                ShortcutModifier::Command | ShortcutModifier::Control | ShortcutModifier::Option
            )
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutBinding {
    pub command_id: String,
    pub shortcut: Option<KeyboardShortcut>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutDefinition {
    pub command_id: &'static str,
    pub default_shortcut: Option<KeyboardShortcut>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutConflict {
    pub command_id: String,
    pub shortcut: KeyboardShortcut,
}

pub struct ShortcutManager {
    definitions: HashMap<&'static str, Option<KeyboardShortcut>>,
    active_by_command: HashMap<String, KeyboardShortcut>,
    command_by_shortcut: HashMap<KeyboardShortcut, String>,
    unbound: HashSet<String>,
    bindings: Vec<ShortcutBinding>,
}

impl ShortcutManager {
    /// Builds the effective shortcut map from definitions and persisted overrides.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate registry IDs, unknown or duplicate override
    /// IDs, redundant defaults, ineligible chords, or conflicts.
    pub fn new(
        definitions: &[ShortcutDefinition],
        bindings: &[ShortcutBinding],
    ) -> Result<Self, String> {
        let definition_map = definitions
            .iter()
            .map(|definition| (definition.command_id, definition.default_shortcut.clone()))
            .collect::<HashMap<_, _>>();
        if definition_map.len() != definitions.len() {
            return Err("shortcut registry contains duplicate command IDs".into());
        }
        let bindings = Self::sanitize(definitions, bindings)?;
        let overridden = bindings
            .iter()
            .map(|binding| binding.command_id.as_str())
            .collect::<HashSet<_>>();
        let mut active_by_command = definitions
            .iter()
            .filter(|definition| !overridden.contains(definition.command_id))
            .filter_map(|definition| {
                definition
                    .default_shortcut
                    .clone()
                    .map(|shortcut| (definition.command_id.to_owned(), shortcut))
            })
            .collect::<HashMap<_, _>>();
        let mut unbound = HashSet::new();
        for binding in &bindings {
            match &binding.shortcut {
                Some(shortcut) => {
                    active_by_command.insert(binding.command_id.clone(), shortcut.clone());
                }
                None => {
                    unbound.insert(binding.command_id.clone());
                }
            }
        }
        let command_by_shortcut = active_by_command
            .iter()
            .map(|(command, shortcut)| (shortcut.clone(), command.clone()))
            .collect();
        Ok(Self {
            definitions: definition_map,
            active_by_command,
            command_by_shortcut,
            unbound,
            bindings,
        })
    }

    #[must_use]
    pub fn command_for(&self, shortcut: &KeyboardShortcut) -> Option<&str> {
        self.command_by_shortcut.get(shortcut).map(String::as_str)
    }

    #[must_use]
    pub fn shortcut_for(&self, command_id: &str) -> Option<&KeyboardShortcut> {
        self.active_by_command.get(command_id)
    }

    #[must_use]
    pub fn is_unbound(&self, command_id: &str) -> bool {
        self.unbound.contains(command_id)
    }

    #[must_use]
    pub fn bindings(&self) -> &[ShortcutBinding] {
        &self.bindings
    }

    #[must_use]
    pub fn conflict_for(
        &self,
        shortcut: &KeyboardShortcut,
        command_id: &str,
    ) -> Option<ShortcutConflict> {
        let conflicting = self.command_by_shortcut.get(shortcut)?;
        (conflicting != command_id).then(|| ShortcutConflict {
            command_id: conflicting.clone(),
            shortcut: shortcut.clone(),
        })
    }

    /// Returns normalized overrides after replacing one command's binding.
    ///
    /// # Errors
    ///
    /// Returns an error when the command is unknown or the requested shortcut
    /// conflicts with another effective command binding.
    pub fn updated_bindings(
        &self,
        command_id: &str,
        shortcut: Option<KeyboardShortcut>,
    ) -> Result<Vec<ShortcutBinding>, String> {
        let default = self
            .definitions
            .get(command_id)
            .ok_or_else(|| format!("unknown shortcut command: {command_id}"))?;
        let mut bindings = self
            .bindings
            .iter()
            .filter(|binding| binding.command_id != command_id)
            .cloned()
            .collect::<Vec<_>>();
        if default != &shortcut {
            bindings.push(ShortcutBinding {
                command_id: command_id.to_owned(),
                shortcut,
            });
        }
        let definitions = self
            .definitions
            .iter()
            .map(|(command_id, shortcut)| ShortcutDefinition {
                command_id,
                default_shortcut: shortcut.clone(),
            })
            .collect::<Vec<_>>();
        Self::sanitize(&definitions, &bindings)
    }

    /// Validates and normalizes persisted shortcut overrides.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown or duplicate commands, redundant defaults,
    /// ineligible shortcuts, or any effective shortcut conflict.
    pub fn sanitize(
        definitions: &[ShortcutDefinition],
        bindings: &[ShortcutBinding],
    ) -> Result<Vec<ShortcutBinding>, String> {
        let defaults = definitions
            .iter()
            .map(|definition| (definition.command_id, definition.default_shortcut.as_ref()))
            .collect::<HashMap<_, _>>();
        let mut seen_commands = HashSet::new();
        for binding in bindings {
            if !defaults.contains_key(binding.command_id.as_str()) {
                return Err(format!("unknown shortcut command: {}", binding.command_id));
            }
            if !seen_commands.insert(binding.command_id.as_str()) {
                return Err(format!(
                    "duplicate shortcut command: {}",
                    binding.command_id
                ));
            }
            if binding
                .shortcut
                .as_ref()
                .is_some_and(|shortcut| !shortcut.is_eligible_command_binding())
            {
                return Err(format!(
                    "shortcut for {} requires Control, Command, or Option",
                    binding.command_id
                ));
            }
        }
        let overridden = bindings
            .iter()
            .map(|binding| binding.command_id.as_str())
            .collect::<HashSet<_>>();
        let mut used = definitions
            .iter()
            .filter(|definition| !overridden.contains(definition.command_id))
            .filter_map(|definition| definition.default_shortcut.as_ref())
            .cloned()
            .collect::<HashSet<_>>();
        let mut result = Vec::new();
        for binding in bindings {
            if defaults[binding.command_id.as_str()] == binding.shortcut.as_ref() {
                continue;
            }
            if let Some(shortcut) = &binding.shortcut
                && !used.insert(shortcut.clone())
            {
                return Err(format!(
                    "shortcut conflict for {}: {}",
                    binding.command_id,
                    shortcut.storage_string()
                ));
            }
            result.push(binding.clone());
        }
        Ok(result)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct ShortcutDocument {
    pub(crate) bindings: Vec<ShortcutBindingDocument>,
}

#[derive(Deserialize)]
pub(crate) struct ShortcutBindingDocument {
    command_id: String,
    shortcut: String,
}

impl ShortcutDocument {
    pub(crate) fn into_bindings(self) -> Result<Vec<ShortcutBinding>, String> {
        self.bindings
            .into_iter()
            .map(|binding| {
                let shortcut =
                    if binding.shortcut.is_empty() {
                        None
                    } else {
                        Some(KeyboardShortcut::parse(&binding.shortcut).ok_or_else(|| {
                            format!("invalid shortcut for {}", binding.command_id)
                        })?)
                    };
                Ok(ShortcutBinding {
                    command_id: binding.command_id,
                    shortcut,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{DefaultHasher, Hash, Hasher};

    fn shortcut(storage: &str) -> KeyboardShortcut {
        KeyboardShortcut::parse(storage).unwrap()
    }

    fn definitions() -> Vec<ShortcutDefinition> {
        vec![
            ShortcutDefinition {
                command_id: "one",
                default_shortcut: Some(shortcut("command+a")),
            },
            ShortcutDefinition {
                command_id: "two",
                default_shortcut: Some(shortcut("command+b")),
            },
            ShortcutDefinition {
                command_id: "none",
                default_shortcut: None,
            },
        ]
    }

    #[test]
    fn storage_is_source_compatible_and_canonical() {
        for storage in [
            "command+control+option+shift+x",
            "command+space",
            "command+delete",
            "command+return",
            "control+tab",
            "command+left",
            "command+right",
            "command+up",
            "command+down",
        ] {
            assert_eq!(shortcut(storage).storage_string(), storage);
        }
        assert_eq!(
            shortcut("OPTION+COMMAND+X").storage_string(),
            "command+option+x"
        );
        assert_eq!(shortcut("command+enter").storage_string(), "command+return");
        assert_eq!(
            shortcut("command+option+shift+left").display(),
            "Ctrl+Alt+Shift+Left"
        );
        assert_eq!(shortcut("control+return").display(), "Control+Return");
    }

    #[test]
    fn shortcut_hash_includes_key_and_modifier_identity() {
        fn digest(shortcut: &KeyboardShortcut) -> u64 {
            let mut hasher = DefaultHasher::new();
            shortcut.hash(&mut hasher);
            hasher.finish()
        }

        let base = shortcut("command+x");
        assert_ne!(digest(&base), digest(&shortcut("command+y")));
        assert_ne!(digest(&base), digest(&shortcut("option+x")));
    }

    #[test]
    fn malformed_and_bare_bindings_are_rejected() {
        for storage in ["", "command", "command++a", "hyper+a", "shift+x", "xy"] {
            let parsed = KeyboardShortcut::parse(storage);
            assert!(parsed.is_none() || !parsed.unwrap().is_eligible_command_binding());
        }
    }

    #[test]
    fn overrides_clear_defaults_and_resolve_commands() {
        let manager = ShortcutManager::new(
            &definitions(),
            &[
                ShortcutBinding {
                    command_id: "one".into(),
                    shortcut: None,
                },
                ShortcutBinding {
                    command_id: "none".into(),
                    shortcut: Some(shortcut("command+c")),
                },
            ],
        )
        .unwrap();
        assert!(manager.is_unbound("one"));
        assert!(!manager.is_unbound("none"));
        assert_eq!(manager.shortcut_for("one"), None);
        assert_eq!(manager.shortcut_for("none"), Some(&shortcut("command+c")));
        assert_eq!(manager.bindings().len(), 2);
        assert_eq!(manager.command_for(&shortcut("command+a")), None);
        assert_eq!(manager.command_for(&shortcut("command+c")), Some("none"));
        assert_eq!(manager.conflict_for(&shortcut("command+c"), "none"), None);
        assert_eq!(
            manager.conflict_for(&shortcut("command+c"), "one"),
            Some(ShortcutConflict {
                command_id: "none".into(),
                shortcut: shortcut("command+c"),
            })
        );
    }

    #[test]
    fn unknown_duplicates_conflicts_and_redundant_defaults_are_governed() {
        let unknown = ShortcutBinding {
            command_id: "missing".into(),
            shortcut: None,
        };
        assert!(ShortcutManager::new(&definitions(), &[unknown]).is_err());
        let duplicate = ShortcutBinding {
            command_id: "one".into(),
            shortcut: None,
        };
        assert!(ShortcutManager::new(&definitions(), &[duplicate.clone(), duplicate]).is_err());
        let conflict = ShortcutBinding {
            command_id: "none".into(),
            shortcut: Some(shortcut("command+a")),
        };
        assert!(ShortcutManager::new(&definitions(), &[conflict]).is_err());
        let redundant = ShortcutBinding {
            command_id: "one".into(),
            shortcut: Some(shortcut("command+a")),
        };
        assert!(
            ShortcutManager::new(&definitions(), &[redundant])
                .unwrap()
                .bindings()
                .is_empty()
        );
    }

    #[test]
    fn update_removes_redundant_overrides_and_preserves_explicit_unbinds() {
        let manager = ShortcutManager::new(&definitions(), &[]).unwrap();
        assert!(
            manager
                .updated_bindings("one", Some(shortcut("command+a")))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            manager.updated_bindings("one", None).unwrap(),
            [ShortcutBinding {
                command_id: "one".into(),
                shortcut: None,
            }]
        );

        let manager = ShortcutManager::new(
            &definitions(),
            &[ShortcutBinding {
                command_id: "none".into(),
                shortcut: Some(shortcut("command+c")),
            }],
        )
        .unwrap();
        assert_eq!(
            manager.updated_bindings("one", None).unwrap(),
            [
                ShortcutBinding {
                    command_id: "none".into(),
                    shortcut: Some(shortcut("command+c")),
                },
                ShortcutBinding {
                    command_id: "one".into(),
                    shortcut: None,
                },
            ]
        );
    }
}
