use kdl::KdlDocument;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// A setting the file does not name keeps its default; saving removes the key instead of writing
// the default down.
pub enum Setting {
    ShowHidden,
    Inspector,
    GridLayout,
    FoldersFirst,
    ConfirmTrash,
    ConfirmDelete,
    ShowKind,
    ShowSize,
    ShowModified,
    ScrollInertia,
    Overscroll,
    ShowThumbnails,
    SystemIcons,
}

impl Setting {
    pub const ALL: [Setting; 13] = [
        Setting::ShowHidden,
        Setting::Inspector,
        Setting::GridLayout,
        Setting::FoldersFirst,
        Setting::ConfirmTrash,
        Setting::ConfirmDelete,
        Setting::ShowKind,
        Setting::ShowSize,
        Setting::ShowModified,
        Setting::ScrollInertia,
        Setting::Overscroll,
        Setting::ShowThumbnails,
        Setting::SystemIcons,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Setting::ShowHidden => "show-hidden",
            Setting::Inspector => "inspector",
            Setting::GridLayout => "grid",
            Setting::FoldersFirst => "folders-first",
            Setting::ConfirmTrash => "confirm-trash",
            Setting::ConfirmDelete => "confirm-delete",
            Setting::ShowKind => "show-kind",
            Setting::ShowSize => "show-size",
            Setting::ShowModified => "show-modified",
            Setting::ScrollInertia => "scroll-inertia",
            Setting::Overscroll => "overscroll",
            Setting::ShowThumbnails => "show-thumbnails",
            Setting::SystemIcons => "system-icons",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Setting::ShowHidden => "hidden",
            Setting::Inspector => "inspector",
            Setting::GridLayout => "grid",
            Setting::FoldersFirst => "folders-first",
            Setting::ConfirmTrash => "confirm-trash",
            Setting::ConfirmDelete => "confirm-delete",
            Setting::ShowKind => "kind-column",
            Setting::ShowSize => "size-column",
            Setting::ShowModified => "modified-column",
            Setting::ScrollInertia => "inertia",
            Setting::Overscroll => "overscroll",
            Setting::ShowThumbnails => "thumbnails",
            Setting::SystemIcons => "system-icons",
        }
    }

    pub fn default_on(self) -> bool {
        match self {
            Setting::ShowHidden | Setting::GridLayout | Setting::SystemIcons => false,
            Setting::Inspector
            | Setting::FoldersFirst
            | Setting::ConfirmTrash
            | Setting::ConfirmDelete
            | Setting::ShowKind
            | Setting::ShowSize
            | Setting::ShowModified
            | Setting::ScrollInertia
            | Setting::Overscroll
            | Setting::ShowThumbnails => true,
        }
    }

    fn index(self) -> usize {
        match self {
            Setting::ShowHidden => 0,
            Setting::Inspector => 1,
            Setting::GridLayout => 2,
            Setting::FoldersFirst => 3,
            Setting::ConfirmTrash => 4,
            Setting::ConfirmDelete => 5,
            Setting::ShowKind => 6,
            Setting::ShowSize => 7,
            Setting::ShowModified => 8,
            Setting::ScrollInertia => 9,
            Setting::Overscroll => 10,
            Setting::ShowThumbnails => 11,
            Setting::SystemIcons => 12,
        }
    }

    fn value(self, settings: &Settings) -> bool {
        match self {
            Setting::ShowHidden => settings.show_hidden,
            Setting::Inspector => settings.inspector,
            Setting::GridLayout => settings.grid,
            Setting::FoldersFirst => settings.folders_first,
            Setting::ConfirmTrash => settings.confirm_trash,
            Setting::ConfirmDelete => settings.confirm_delete,
            Setting::ShowKind => settings.show_kind,
            Setting::ShowSize => settings.show_size,
            Setting::ShowModified => settings.show_modified,
            Setting::ScrollInertia => settings.scroll_inertia,
            Setting::Overscroll => settings.overscroll,
            Setting::ShowThumbnails => settings.show_thumbnails,
            Setting::SystemIcons => settings.system_icons,
        }
    }

    fn set(self, settings: &mut Settings, on: bool) {
        match self {
            Setting::ShowHidden => settings.show_hidden = on,
            Setting::Inspector => settings.inspector = on,
            Setting::GridLayout => settings.grid = on,
            Setting::FoldersFirst => settings.folders_first = on,
            Setting::ConfirmTrash => settings.confirm_trash = on,
            Setting::ConfirmDelete => settings.confirm_delete = on,
            Setting::ShowKind => settings.show_kind = on,
            Setting::ShowSize => settings.show_size = on,
            Setting::ShowModified => settings.show_modified = on,
            Setting::ScrollInertia => settings.scroll_inertia = on,
            Setting::Overscroll => settings.overscroll = on,
            Setting::ShowThumbnails => settings.show_thumbnails = on,
            Setting::SystemIcons => settings.system_icons = on,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub show_hidden: bool,
    pub inspector: bool,
    pub grid: bool,
    pub folders_first: bool,
    pub confirm_trash: bool,
    pub confirm_delete: bool,
    pub show_kind: bool,
    pub show_size: bool,
    pub show_modified: bool,
    pub scroll_inertia: bool,
    pub overscroll: bool,
    pub show_thumbnails: bool,
    pub system_icons: bool,
    present: [bool; 13],
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_hidden: false,
            inspector: true,
            grid: false,
            folders_first: true,
            confirm_trash: true,
            confirm_delete: true,
            show_kind: true,
            show_size: true,
            show_modified: true,
            scroll_inertia: true,
            overscroll: true,
            show_thumbnails: true,
            system_icons: false,
            present: [false; 13],
        }
    }
}

impl Settings {
    pub fn defaults() -> Settings {
        Settings::default()
    }

    pub fn from_doc(doc: Option<&KdlDocument>) -> Settings {
        let mut settings = Settings::defaults();
        let parsed: Vec<(Setting, bool)> = actions_kdl::settings::read(doc);
        for (setting, on) in parsed {
            settings.present[setting.index()] = true;
            setting.set(&mut settings, on);
        }
        settings
    }

    pub fn is_set(&self, setting: Setting) -> bool {
        self.present[setting.index()]
    }

    pub fn write(&self, doc: &mut KdlDocument, setting: Setting) {
        actions_kdl::settings::write(doc, setting, setting.value(self));
    }

    pub fn remember(&mut self, changed: Setting, on: bool) {
        changed.set(self, on);
        self.present[changed.index()] = true;
    }

    pub fn reset(&mut self, setting: Setting) {
        setting.set(self, setting.default_on());
        self.present[setting.index()] = false;
    }

    pub fn clear(doc: &mut KdlDocument, setting: Setting) {
        actions_kdl::settings::remove(doc, setting);
    }
}

impl actions::Setting for Setting {
    const ALL: &'static [Self] = &Setting::ALL;

    fn from_name(name: &str) -> Option<Setting> {
        Setting::ALL
            .into_iter()
            .find(|setting| setting.name() == name)
    }

    fn name(self) -> &'static str {
        Setting::name(self)
    }

    fn id(self) -> &'static str {
        Setting::id(self)
    }
}
