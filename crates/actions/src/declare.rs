//! One declaration per action, so the enum, the palette order and the default chords cannot drift.
//!
//! An app writes its commands once:
//!
//! ```
//! actions::declare_actions! {
//!     pub enum Example {
//!         Open = "open", "Open", chord("ctrl+o", "super+o"),
//!         Close = "close", "Close", unbound,
//!     }
//! }
//! ```
//!
//! The macro generates the enum, `ALL`, `id`, `label` and `default_chord`, plus the
//! `actions::Action` implementation. An app still writes its own `icon`, `in_palette`,
//! `ignores_shift` and the exhaustive match that performs each command; the compiler checks
//! those, so a new command cannot be added without deciding what it does.
//!
//! `chord(windows, macos)` names the two defaults; `unbound` starts the action with no chord.

/// Declares an app's action enum and the metadata the shared keymap needs.
#[macro_export]
macro_rules! declare_actions {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $variant:ident = $id:literal, $label:literal,
                    $chord:ident $(($($chord_arg:literal),+))?
            ),* $(,)?
        }
        $(impl {
            $($extra:item)*
        })?
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $vis enum $name {
            $($variant,)*
        }

        impl $name {
            /// Every action, in the order the palette lists them.
            pub const ALL: &'static [Self] = &[$($name::$variant,)*];

            pub fn id(self) -> &'static str {
                match self {
                    $($name::$variant => $id,)*
                }
            }

            pub fn label(self) -> &'static str {
                match self {
                    $($name::$variant => $label,)*
                }
            }

            /// The default chord, or `None` for an action that starts unbound.
            pub fn default_chord(self) -> Option<&'static str> {
                match self {
                    $(
                        $name::$variant => $crate::declare_actions!(
                            @chord $chord $(($($chord_arg),+))?
                        ),
                    )*
                }
            }

            pub fn from_id(id: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|action| action.id() == id)
            }
        }

        impl $crate::Action for $name {
            const ALL: &'static [Self] = $name::ALL;

            fn from_id(id: &str) -> Option<Self> {
                $name::from_id(id)
            }

            fn id(self) -> &'static str {
                $name::id(self)
            }

            fn label(self) -> &'static str {
                $name::label(self)
            }

            fn default_chord(self) -> Option<&'static str> {
                $name::default_chord(self)
            }

            $($($extra)*)?
        }
    };

    (@chord chord($windows:literal, $macos:literal)) => {
        Some(if cfg!(target_os = "macos") { $macos } else { $windows })
    };

    (@chord unbound) => {
        None
    };
}
