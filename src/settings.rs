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
            $(
                ! $extra:ident : $extra_type:ty = $extra_default:expr;
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
            $(pub $extra: $extra_type,)*
            present: [bool; Setting::ALL.len()],
        }

        impl Default for $settings {
            fn default() -> Self {
                $settings {
                    $($field: $default,)*
                    $($extra: $extra_default,)*
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
        // Not a switch: the window's material is a choice between named backdrops, written as
        // `window-material "mica"`, so it has its own value, presence flag and file helpers.
        ! material: crate::material::Material = crate::material::Material::Off;
        ! material_set: bool = false;
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
        if let Some(material) = doc
            .and_then(|doc| {
                actions_kdl::settings::read_string(doc, crate::material::Material::FILE_NAME)
            })
            .and_then(|value| crate::material::Material::from_name(&value))
        {
            settings.material = material;
            settings.material_set = true;
        }
        settings
    }

    /// Records a window material the settings panel chose.
    pub fn set_material(&mut self, material: crate::material::Material) {
        self.material = material;
        self.material_set = true;
    }

    /// Back to the default, so the key leaves `config.kdl`.
    pub fn reset_material(&mut self) {
        self.material = crate::material::Material::Off;
        self.material_set = false;
    }

    pub fn write_material(doc: &mut KdlDocument, material: crate::material::Material) {
        actions_kdl::settings::write_string(
            doc,
            crate::material::Material::FILE_NAME,
            material.name(),
        );
    }

    pub fn clear_material(doc: &mut KdlDocument) {
        actions_kdl::settings::remove_string(doc, crate::material::Material::FILE_NAME);
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
