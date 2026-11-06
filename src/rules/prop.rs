use crate::app::Layout;
use rune_kit::SortDir;

use super::{Kind, SortKey};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Prop<T> {
    #[default]
    Unset,
    Inherit,
    Set(T),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Props {
    pub layout: Prop<Layout>,
    pub sort: Prop<SortKey>,
    pub direction: Prop<SortDir>,
    pub show_hidden: Prop<bool>,
    pub hidden: Prop<bool>,
}

/// One rule property: its file name, which kind of rule it belongs to, its editor options and
/// the value it sets. Parsing, writing, details and the editor all go through this, so a new
/// property is added in one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropId {
    Layout,
    Sort,
    Direction,
    ShowHidden,
    Hidden,
}

impl PropId {
    pub const ALL: &'static [PropId] = &[
        PropId::Layout,
        PropId::Sort,
        PropId::Direction,
        PropId::ShowHidden,
        PropId::Hidden,
    ];

    pub fn all(kind: Kind) -> &'static [PropId] {
        match kind {
            Kind::Folder => &[
                PropId::Layout,
                PropId::Sort,
                PropId::Direction,
                PropId::ShowHidden,
            ],
            Kind::Entry => &[PropId::Hidden],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PropId::Layout => "layout",
            PropId::Sort => "sort",
            PropId::Direction => "direction",
            PropId::ShowHidden => "show-hidden",
            PropId::Hidden => "hidden",
        }
    }

    pub fn from_name(name: &str) -> Option<PropId> {
        PropId::ALL.iter().copied().find(|prop| prop.name() == name)
    }

    pub fn kind(self) -> Kind {
        match self {
            PropId::Layout | PropId::Sort | PropId::Direction | PropId::ShowHidden => Kind::Folder,
            PropId::Hidden => Kind::Entry,
        }
    }

    /// The values the rule editor offers, including `inherit`.
    pub fn options(self) -> &'static [&'static str] {
        match self {
            PropId::Layout => &["list", "grid", "inherit"],
            PropId::Sort => &["name", "kind", "size", "modified", "inherit"],
            PropId::Direction => &["ascending", "descending", "inherit"],
            PropId::ShowHidden | PropId::Hidden => &["show", "hide", "inherit"],
        }
    }

    /// The value the editor starts a newly added property with.
    pub fn initial(self) -> &'static str {
        match self {
            PropId::Layout => "list",
            PropId::Sort => "name",
            PropId::Direction => "ascending",
            PropId::ShowHidden => "show",
            PropId::Hidden => "hide",
        }
    }

    /// The editor's label for the held value; `None` when the property is unset.
    pub fn label(self, props: &Props) -> Option<&'static str> {
        match self {
            PropId::Layout => held(props.layout, Layout::as_str),
            PropId::Sort => held(props.sort, SortKey::as_str),
            PropId::Direction => held(props.direction, |direction| match direction {
                SortDir::Ascending => "ascending",
                SortDir::Descending => "descending",
            }),
            PropId::ShowHidden => held(props.show_hidden, show_label),
            PropId::Hidden => held(props.hidden, |hidden| show_label(!hidden)),
        }
    }

    pub fn is_set(self, props: &Props) -> bool {
        self.label(props).is_some()
    }

    /// Sets the property from an editor value. Returns `false` when the value is not one of
    /// `options`, so a caller can leave the rule untouched.
    pub fn set(self, props: &mut Props, value: &str) -> bool {
        if value == "inherit" {
            self.inherit(props);
            return true;
        }
        match self {
            PropId::Layout => Layout::from_str(value)
                .map(|value| props.layout = Prop::Set(value))
                .is_some(),
            PropId::Sort => SortKey::from_column(value)
                .map(|value| props.sort = Prop::Set(value))
                .is_some(),
            PropId::Direction => match value {
                "ascending" => {
                    props.direction = Prop::Set(SortDir::Ascending);
                    true
                }
                "descending" => {
                    props.direction = Prop::Set(SortDir::Descending);
                    true
                }
                _ => false,
            },
            PropId::ShowHidden => parse_show(value)
                .map(|on| props.show_hidden = Prop::Set(on))
                .is_some(),
            PropId::Hidden => parse_show(value)
                .map(|on| props.hidden = Prop::Set(on))
                .is_some(),
        }
    }

    pub fn clear(self, props: &mut Props) {
        match self {
            PropId::Layout => props.layout = Prop::Unset,
            PropId::Sort => props.sort = Prop::Unset,
            PropId::Direction => props.direction = Prop::Unset,
            PropId::ShowHidden => props.show_hidden = Prop::Unset,
            PropId::Hidden => props.hidden = Prop::Unset,
        }
    }

    fn inherit(self, props: &mut Props) {
        match self {
            PropId::Layout => props.layout = Prop::Inherit,
            PropId::Sort => props.sort = Prop::Inherit,
            PropId::Direction => props.direction = Prop::Inherit,
            PropId::ShowHidden => props.show_hidden = Prop::Inherit,
            PropId::Hidden => props.hidden = Prop::Inherit,
        }
    }

    /// The property as the rule file writes it, or `None` when it is unset.
    pub(super) fn kdl_line(self, props: &Props) -> Option<String> {
        match self {
            PropId::Layout => kdl(self.name(), props.layout, Layout::as_str),
            PropId::Sort => kdl(self.name(), props.sort, SortKey::as_str),
            PropId::Direction => kdl(self.name(), props.direction, |direction| match direction {
                SortDir::Ascending => "ascending",
                SortDir::Descending => "descending",
            }),
            PropId::ShowHidden => kdl(self.name(), props.show_hidden, below),
            PropId::Hidden => kdl(self.name(), props.hidden, below),
        }
    }
}

fn held<T: Copy>(prop: Prop<T>, of: impl Fn(T) -> &'static str) -> Option<&'static str> {
    match prop {
        Prop::Unset => None,
        Prop::Inherit => Some("inherit"),
        Prop::Set(value) => Some(of(value)),
    }
}

fn show_label(on: bool) -> &'static str {
    if on { "show" } else { "hide" }
}

fn below(on: bool) -> &'static str {
    if on { "#true" } else { "#false" }
}

fn parse_show(value: &str) -> Option<bool> {
    match value {
        "show" | "true" => Some(true),
        "hide" | "false" => Some(false),
        _ => None,
    }
}

fn kdl<T: Copy>(name: &str, prop: Prop<T>, value: impl Fn(T) -> &'static str) -> Option<String> {
    match prop {
        Prop::Unset => None,
        Prop::Inherit => Some(format!("{name} inherit")),
        Prop::Set(set) => Some(format!("{name} {}", value(set))),
    }
}
