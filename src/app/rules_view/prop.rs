//! The properties a rule can set: what the file calls each one, the values it accepts, and the
//! one the form starts it at. The form and its menu read this, and nothing here touches a file.

use crate::app::ViewMode;
use crate::rules::{Kind, Prop, Props, SortKey};
use rune_kit::SortDir;

/// One property the editor can change, in the field that lists its values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleProp {
    View,
    Sort,
    Direction,
    ShowHidden,
    Hidden,
}

impl RuleProp {
    /// The properties a rule of `kind` can set, in the order the form lists them.
    pub fn all(kind: Kind) -> &'static [RuleProp] {
        match kind {
            Kind::Folder => &[
                RuleProp::View,
                RuleProp::Sort,
                RuleProp::Direction,
                RuleProp::ShowHidden,
            ],
            Kind::Entry => &[RuleProp::Hidden],
        }
    }

    /// The key the file writes it as, which is what the form calls it too.
    pub fn name(self) -> &'static str {
        match self {
            RuleProp::View => "view",
            RuleProp::Sort => "sort",
            RuleProp::Direction => "direction",
            RuleProp::ShowHidden => "show-hidden",
            RuleProp::Hidden => "hidden",
        }
    }

    /// The property a key names, for the menu that adds one to the rule.
    pub fn from_name(name: &str) -> Option<RuleProp> {
        [
            RuleProp::View,
            RuleProp::Sort,
            RuleProp::Direction,
            RuleProp::ShowHidden,
            RuleProp::Hidden,
        ]
        .into_iter()
        .find(|prop| prop.name() == name)
    }

    /// Every value the file accepts, as the form words them; `inherit` last.
    pub fn options(self) -> &'static [&'static str] {
        match self {
            RuleProp::View => &["list", "grid", "inherit"],
            RuleProp::Sort => &["name", "size", "modified", "inherit"],
            RuleProp::Direction => &["ascending", "descending", "inherit"],
            RuleProp::ShowHidden | RuleProp::Hidden => &["show", "hide", "inherit"],
        }
    }

    /// The value the property takes when it is added to the rule.
    pub fn initial(self) -> &'static str {
        match self {
            RuleProp::View => "list",
            RuleProp::Sort => "name",
            RuleProp::Direction => "ascending",
            RuleProp::ShowHidden => "show",
            RuleProp::Hidden => "hide",
        }
    }

    /// What the rule holds for it, as one of `options`; `None` when the rule leaves it unset.
    pub fn label(self, props: &Props) -> Option<&'static str> {
        match self {
            RuleProp::View => held(
                props.view,
                |view| if view.is_grid() { "grid" } else { "list" },
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

    /// Whether the rule sets it, to a value or to `inherit`.
    pub fn is_set(self, props: &Props) -> bool {
        self.label(props).is_some()
    }

    /// Sets it from one of `options`.
    pub fn set(self, props: &mut Props, value: &str) {
        let inherit = value == "inherit";
        match self {
            RuleProp::View => {
                props.view = if inherit {
                    Prop::Inherit
                } else if value == "grid" {
                    Prop::Set(ViewMode::Grid)
                } else {
                    Prop::Set(ViewMode::List)
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

/// The form's one open menu: the properties to add, a property's values, or a matcher's kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleMenu {
    AddProperty,
    Prop(RuleProp),
    Matcher { exclude: bool, index: usize },
}

/// What a property holds, as the form words it; `None` when it is unset.
fn held<T: Copy>(prop: Prop<T>, of: impl Fn(T) -> &'static str) -> Option<&'static str> {
    match prop {
        Prop::Unset => None,
        Prop::Inherit => Some("inherit"),
        Prop::Set(value) => Some(of(value)),
    }
}
