//! Menus and the keyboard shortcuts of their items, which every platform builds with its
//! own menus.

use std::fmt;

coded_enum! {
    /// A key a shortcut presses with the platform's command key.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Key {
        A,
        B,
        C,
        D,
        E,
        F,
        G,
        H,
        I,
        J,
        K,
        L,
        M,
        N,
        O,
        P,
        Q,
        R,
        S,
        T,
        U,
        V,
        W,
        X,
        Y,
        Z,
        Digit0,
        Digit1,
        Digit2,
        Digit3,
        Digit4,
        Digit5,
        Digit6,
        Digit7,
        Digit8,
        Digit9,
        Comma,
        Period,
        Slash,
        Semicolon,
        Quote,
        LeftBracket,
        RightBracket,
        Minus,
        Equals,
        Return,
        Delete,
        Up,
        Down,
        Left,
        Right,
    }
}

impl Key {
    /// How a shortcut names the key: the character it types, or a word for the others.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::E => "e",
            Self::F => "f",
            Self::G => "g",
            Self::H => "h",
            Self::I => "i",
            Self::J => "j",
            Self::K => "k",
            Self::L => "l",
            Self::M => "m",
            Self::N => "n",
            Self::O => "o",
            Self::P => "p",
            Self::Q => "q",
            Self::R => "r",
            Self::S => "s",
            Self::T => "t",
            Self::U => "u",
            Self::V => "v",
            Self::W => "w",
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::Digit0 => "0",
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::Digit5 => "5",
            Self::Digit6 => "6",
            Self::Digit7 => "7",
            Self::Digit8 => "8",
            Self::Digit9 => "9",
            Self::Comma => ",",
            Self::Period => ".",
            Self::Slash => "/",
            Self::Semicolon => ";",
            Self::Quote => "'",
            Self::LeftBracket => "[",
            Self::RightBracket => "]",
            Self::Minus => "-",
            Self::Equals => "=",
            Self::Return => "return",
            Self::Delete => "delete",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// The character the key types without Shift, for keys that type one.
    #[must_use]
    pub fn character(self) -> Option<char> {
        let mut chars = self.name().chars();
        match (chars.next(), chars.next()) {
            (Some(character), None) => Some(character),
            _ => None,
        }
    }

    fn named(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.name() == name)
    }
}

/// A keyboard shortcut: the platform's command key, Cmd on macOS and Ctrl on Windows,
/// with `key`, and Shift or Alt (Option on macOS) when set. `repr(C)` because compiled
/// apps store it as is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Shortcut {
    pub key: Key,
    pub shift: bool,
    pub alt: bool,
}

/// The modifiers a shortcut may add to the command key, as code writes them.
pub const SHIFT: &str = "shift";
pub const ALT: &str = "alt";

impl Shortcut {
    #[must_use]
    pub const fn command(key: Key) -> Self {
        Self {
            key,
            shift: false,
            alt: false,
        }
    }

    /// Reads a shortcut as code writes it: its modifiers, then its key, joined by `+`,
    /// such as `"n"` or `"shift+n"`.
    ///
    /// # Errors
    /// When a part is no modifier or key, or a modifier is written twice.
    pub fn parse(text: &str) -> Result<Self, ShortcutError> {
        let (modifiers, key) = text.rsplit_once('+').unwrap_or(("", text));
        let key = Key::named(key).ok_or_else(|| ShortcutError::Key(key.to_owned()))?;
        let mut shortcut = Self::command(key);
        for modifier in modifiers.split('+').filter(|modifier| !modifier.is_empty()) {
            let held = match modifier {
                SHIFT => &mut shortcut.shift,
                ALT => &mut shortcut.alt,
                other => return Err(ShortcutError::Modifier(other.to_owned())),
            };
            if std::mem::replace(held, true) {
                return Err(ShortcutError::Repeated(modifier.to_owned()));
            }
        }
        Ok(shortcut)
    }

    /// The system command that already uses this shortcut, if one does.
    #[must_use]
    pub fn taken_by(self) -> Option<SystemCommand> {
        SystemCommand::ALL
            .iter()
            .copied()
            .find(|command| command.shortcut() == self)
    }
}

impl fmt::Display for Shortcut {
    /// As code writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.shift {
            write!(f, "{SHIFT}+")?;
        }
        if self.alt {
            write!(f, "{ALT}+")?;
        }
        f.write_str(self.key.name())
    }
}

/// Why a shortcut can't be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutError {
    Key(String),
    Modifier(String),
    Repeated(String),
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(key) => write!(
                f,
                "`{key}` isn't a key a shortcut can use; use a letter, a digit, one of `, . / ; ' [ ] - =`, or `return`, `delete`, `up`, `down`, `left` or `right`"
            ),
            Self::Modifier(modifier) => write!(
                f,
                "`{modifier}` isn't a modifier; a shortcut always uses Cmd on macOS and Ctrl on Windows, and may add `{SHIFT}` and `{ALT}`"
            ),
            Self::Repeated(modifier) => write!(f, "`{modifier}` is written twice"),
        }
    }
}

coded_enum! {
    /// The commands the system's own menus and text fields take shortcuts for, which an
    /// app's menus leave alone.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SystemCommand {
        Quit,
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
    }
}

impl SystemCommand {
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Quit => "Quit",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
        }
    }

    #[must_use]
    pub const fn shortcut(self) -> Shortcut {
        match self {
            Self::Quit => Shortcut::command(Key::Q),
            Self::Undo => Shortcut::command(Key::Z),
            Self::Redo => Shortcut {
                key: Key::Z,
                shift: true,
                alt: false,
            },
            Self::Cut => Shortcut::command(Key::X),
            Self::Copy => Shortcut::command(Key::C),
            Self::Paste => Shortcut::command(Key::V),
            Self::SelectAll => Shortcut::command(Key::A),
        }
    }
}

/// A menu: its title, and its entries in order. `A` is what an item does: an action the
/// app runs, or, as backends receive it, the number of the item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu<A> {
    pub title: String,
    pub entries: Vec<MenuEntry<A>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry<A> {
    Item {
        title: String,
        shortcut: Option<Shortcut>,
        action: A,
    },
    Separator,
    Submenu(Menu<A>),
}

impl<A> Menu<A> {
    /// The same menu with each item's action moved to the end of `actions`, and the item
    /// numbered by where it went.
    pub fn number(self, actions: &mut Vec<A>) -> Menu<usize> {
        Menu {
            title: self.title,
            entries: self
                .entries
                .into_iter()
                .map(|entry| entry.number(actions))
                .collect(),
        }
    }
}

impl<A> MenuEntry<A> {
    fn number(self, actions: &mut Vec<A>) -> MenuEntry<usize> {
        match self {
            Self::Item {
                title,
                shortcut,
                action,
            } => {
                actions.push(action);
                MenuEntry::Item {
                    title,
                    shortcut,
                    action: actions.len() - 1,
                }
            }
            Self::Separator => MenuEntry::Separator,
            Self::Submenu(menu) => MenuEntry::Submenu(menu.number(actions)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_read_as_code_writes_them() {
        assert_eq!(Shortcut::parse("n"), Ok(Shortcut::command(Key::N)));
        assert_eq!(
            Shortcut::parse("shift+alt+return"),
            Ok(Shortcut {
                key: Key::Return,
                shift: true,
                alt: true,
            })
        );
        assert_eq!(Shortcut::parse(","), Ok(Shortcut::command(Key::Comma)));
        assert_eq!(
            Shortcut::parse("ctrl+n"),
            Err(ShortcutError::Modifier("ctrl".to_owned()))
        );
        assert_eq!(
            Shortcut::parse("shift+shift+n"),
            Err(ShortcutError::Repeated("shift".to_owned()))
        );
        assert_eq!(
            Shortcut::parse("f1"),
            Err(ShortcutError::Key("f1".to_owned()))
        );
        for &key in Key::ALL {
            let shortcut = Shortcut {
                key,
                shift: true,
                alt: false,
            };
            assert_eq!(Shortcut::parse(&shortcut.to_string()), Ok(shortcut));
        }
    }

    #[test]
    fn system_commands_keep_their_shortcuts() {
        assert_eq!(
            Shortcut::parse("c").ok().and_then(Shortcut::taken_by),
            Some(SystemCommand::Copy)
        );
        assert_eq!(
            Shortcut::parse("shift+c").ok().and_then(Shortcut::taken_by),
            None
        );
    }

    #[test]
    fn numbering_moves_actions_out_in_order() {
        let menu = Menu {
            title: "File".to_owned(),
            entries: vec![
                MenuEntry::Item {
                    title: "New".to_owned(),
                    shortcut: None,
                    action: "new",
                },
                MenuEntry::Separator,
                MenuEntry::Submenu(Menu {
                    title: "Sort".to_owned(),
                    entries: vec![MenuEntry::Item {
                        title: "By name".to_owned(),
                        shortcut: None,
                        action: "sort",
                    }],
                }),
            ],
        };
        let mut actions = Vec::new();
        let numbered = menu.number(&mut actions);
        assert_eq!(actions, ["new", "sort"]);
        let MenuEntry::Submenu(sort) = &numbered.entries[2] else {
            panic!("the third entry is a submenu");
        };
        assert!(matches!(sort.entries[0], MenuEntry::Item { action: 1, .. }));
    }
}
