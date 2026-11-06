/// Everything the shared machinery needs to know about one command. An app implements this for
/// its own `Action` enum; nothing here names an app, a file or a widget.
pub trait Action: Copy + Ord + 'static {
    /// Every action the app has, in the order the palette shows them.
    const ALL: &'static [Self];

    fn from_id(id: &str) -> Option<Self>;

    fn id(self) -> &'static str;

    fn label(self) -> &'static str;

    /// The chord that runs it first, or `None` for an action that starts unbound.
    fn default_chord(self) -> Option<&'static str>;

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
