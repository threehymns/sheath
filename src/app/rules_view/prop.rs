use crate::app::Layout;
use crate::rules::{Kind, Prop, Props, SortKey};
use rune_kit::SortDir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleProp {
    Layout,
    Sort,
    Direction,
    ShowHidden,
    Hidden,
}

impl RuleProp {
    pub fn all(kind: Kind) -> &'static [RuleProp] {
        match kind {
            Kind::Folder => &[
                RuleProp::Layout,
                RuleProp::Sort,
                RuleProp::Direction,
                RuleProp::ShowHidden,
            ],
            Kind::Entry => &[RuleProp::Hidden],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            RuleProp::Layout => "layout",
            RuleProp::Sort => "sort",
            RuleProp::Direction => "direction",
            RuleProp::ShowHidden => "show-hidden",
            RuleProp::Hidden => "hidden",
        }
    }

    pub fn from_name(name: &str) -> Option<RuleProp> {
        [
            RuleProp::Layout,
            RuleProp::Sort,
            RuleProp::Direction,
            RuleProp::ShowHidden,
            RuleProp::Hidden,
        ]
        .into_iter()
        .find(|prop| prop.name() == name)
    }

    pub fn options(self) -> &'static [&'static str] {
        match self {
            RuleProp::Layout => &["list", "grid", "inherit"],
            RuleProp::Sort => &["name", "size", "modified", "inherit"],
            RuleProp::Direction => &["ascending", "descending", "inherit"],
            RuleProp::ShowHidden | RuleProp::Hidden => &["show", "hide", "inherit"],
        }
    }

    pub fn initial(self) -> &'static str {
        match self {
            RuleProp::Layout => "list",
            RuleProp::Sort => "name",
            RuleProp::Direction => "ascending",
            RuleProp::ShowHidden => "show",
            RuleProp::Hidden => "hide",
        }
    }

    pub fn label(self, props: &Props) -> Option<&'static str> {
        match self {
            RuleProp::Layout => held(
                props.layout,
                |layout| if layout.is_grid() { "grid" } else { "list" },
            ),
            RuleProp::Sort => held(props.sort, SortKey::as_str),
            RuleProp::Direction => held(props.direction, |direction| match direction {
                SortDir::Ascending => "ascending",
                SortDir::Descending => "descending",
            }),
            RuleProp::ShowHidden => held(props.show_hidden, |on| if on { "show" } else { "hide" }),
            RuleProp::Hidden => held(props.hidden, |on| if on { "hide" } else { "show" }),
        }
    }

    pub fn is_set(self, props: &Props) -> bool {
        self.label(props).is_some()
    }

    pub fn set(self, props: &mut Props, value: &str) {
        let inherit = value == "inherit";
        match self {
            RuleProp::Layout => {
                props.layout = if inherit {
                    Prop::Inherit
                } else if value == "grid" {
                    Prop::Set(Layout::Grid)
                } else {
                    Prop::Set(Layout::List)
                }
            }
            RuleProp::Sort => {
                props.sort = if inherit {
                    Prop::Inherit
                } else {
                    Prop::Set(match value {
                        "size" => SortKey::Size,
                        "modified" => SortKey::Modified,
                        _ => SortKey::Name,
                    })
                }
            }
            RuleProp::Direction => {
                props.direction = if inherit {
                    Prop::Inherit
                } else if value == "descending" {
                    Prop::Set(SortDir::Descending)
                } else {
                    Prop::Set(SortDir::Ascending)
                }
            }
            RuleProp::ShowHidden => {
                props.show_hidden = if inherit {
                    Prop::Inherit
                } else {
                    Prop::Set(value == "show")
                }
            }
            RuleProp::Hidden => {
                props.hidden = if inherit {
                    Prop::Inherit
                } else {
                    Prop::Set(value == "hide")
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleMenu {
    AddProperty,
    Prop(RuleProp),
    Matcher { exclude: bool, index: usize },
}

fn held<T: Copy>(prop: Prop<T>, of: impl Fn(T) -> &'static str) -> Option<&'static str> {
    match prop {
        Prop::Unset => None,
        Prop::Inherit => Some("inherit"),
        Prop::Set(value) => Some(of(value)),
    }
}
