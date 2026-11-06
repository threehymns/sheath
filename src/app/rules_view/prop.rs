//! The rule editor's vocabulary. The properties themselves live in `rules`, beside the parser
//! and writer that use them; this module only adds the menu the editor opens.

pub use crate::rules::PropId as RuleProp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleMenu {
    AddProperty,
    Prop(RuleProp),
    Matcher { exclude: bool, index: usize },
}
