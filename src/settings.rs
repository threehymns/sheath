use kdl::KdlDocument;

/// One row per setting: variant, field, file name, UI id and default. Everything else — the
/// enum, `ALL`, the struct, its `Default`, and the value/set matches — is generated from it, so
/// the file format, the UI switch and the stored value cannot drift apart.
macro_rules! settings {
    (
        pub struct $settings:ident {
            $(
                $variant:ident => $field:ident : $name:literal, $id:literal, $default:literal;
            )*
        }
    ) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum Setting {
            $($variant,)*
        }

        impl Setting {
            pub const ALL: &'static [Setting] = &[$(Setting::$variant,)*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Setting::$variant => $name,)*
                }
            }

            pub fn id(self) -> &'static str {
                match self {
                    $(Setting::$variant => $id,)*
                }
            }

            pub fn default_on(self) -> bool {
                match self {
                    $(Setting::$variant => $default,)*
                }
            }

            /// The variant's position, which is also its slot in `Settings::present`.
            fn index(self) -> usize {
                self as usize
            }

            /// The value currently stored for this setting.
            pub fn value(self, settings: &$settings) -> bool {
                match self {
                    $(Setting::$variant => settings.$field,)*
                }
            }

            fn set(self, settings: &mut $settings, on: bool) {
                match self {
                    $(Setting::$variant => settings.$field = on,)*
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $settings {
            $(pub $field: bool,)*
            present: [bool; Setting::ALL.len()],
        }

        impl Default for $settings {
            fn default() -> Self {
                $settings {
                    $($field: $default,)*
                    present: [false; Setting::ALL.len()],
                }
            }
        }
    };
}

settings! {
    pub struct Settings {
        ShowHidden => show_hidden: "show-hidden", "hidden", false;
        Inspector => inspector: "inspector", "inspector", true;
        GridLayout => grid: "grid", "grid", false;
        ZebraStripes => zebra_stripes: "zebra-stripes", "zebra-stripes", true;
        FoldersFirst => folders_first: "folders-first", "folders-first", true;
        ConfirmTrash => confirm_trash: "confirm-trash", "confirm-trash", true;
        ConfirmDelete => confirm_delete: "confirm-delete", "confirm-delete", true;
        ShowKind => show_kind: "show-kind", "kind-column", true;
        ShowSize => show_size: "show-size", "size-column", true;
        ShowModified => show_modified: "show-modified", "modified-column", true;
        ScrollInertia => scroll_inertia: "scroll-inertia", "inertia", true;
        Overscroll => overscroll: "overscroll", "overscroll", true;
        ShowThumbnails => show_thumbnails: "show-thumbnails", "thumbnails", true;
        SystemIcons => system_icons: "system-icons", "system-icons", false;
    }
}

impl Settings {
    /// Reads the settings a document names; every other setting keeps its default.
    pub fn from_doc(doc: Option<&KdlDocument>) -> Settings {
        let mut settings = Settings::default();
        for (setting, on) in actions_kdl::settings::read::<Setting>(doc) {
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
    fn from_name(name: &str) -> Option<Setting> {
        Setting::ALL
            .iter()
            .copied()
            .find(|setting| setting.name() == name)
    }

    fn name(self) -> &'static str {
        Setting::name(self)
    }
}
