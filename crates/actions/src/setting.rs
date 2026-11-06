/// A setting the app persists. The kernel only needs its two names; the labels, groups and
/// controls a settings page draws are the app's, and the file format is a codec's.
pub trait Setting: Copy + Ord + 'static {
    fn from_name(name: &str) -> Option<Self>;

    /// The key the config file uses.
    fn name(self) -> &'static str;
}
