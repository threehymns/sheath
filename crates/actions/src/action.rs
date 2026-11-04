/// Everything the shared machinery needs to know about one command. An app implements this for
/// its own `Action` enum; nothing here names an app, a file or a widget.
pub trait Action: Copy + Ord + 'static {
    /// Every action the app has, in the order the palette shows them.
    const ALL: &'static [Self];

    fn from_id(id: &str) -> Option<Self>;

    fn id(self) -> &'static str;

    fn label(self) -> &'static str;

    /// The palette section this action sits in.
    fn group(self) -> &'static str {
        "Commands"
    }

    /// The chord that runs it first, or `None` for an action that starts unbound.
    fn default_chord(self) -> Option<&'static str>;

    fn scopes(self) -> Scopes {
        Scopes::default()
    }

    /// Whether the command bar lists it. The command bar itself does not.
    fn in_palette(self) -> bool {
        true
    }

    /// Whether Shift held with the chord still matches it. Only the command bar needs this:
    /// it is the same chord whether the user is reaching for it with a capital letter.
    fn ignores_shift(self) -> bool {
        false
    }
}

/// When an action may fire. The app answers the questions; the kernel does not know what a
/// text field or a terminal is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scopes {
    /// May fire while a text input has focus.
    pub while_typing: bool,
    /// May fire while the terminal pane has focus. Narrow this for commands that belong to the
    /// app rather than to the shell.
    pub in_pane: bool,
    /// May fire while a modal is open. Modals that take keys themselves handle it before the
    /// keymap is asked, so this is for commands the modal deliberately passes through.
    pub in_modal: bool,
}

impl Default for Scopes {
    fn default() -> Self {
        Scopes {
            while_typing: true,
            in_pane: true,
            in_modal: true,
        }
    }
}

/// What the host knows about the frame a key arrived in. The kernel does not inspect the app;
/// the host answers these three questions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub typing: bool,
    pub pane: bool,
    pub modal: bool,
}
