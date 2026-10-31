use crate::app::App;
use crate::keybinds::Action;
use crate::recorder::{Outcome, Recorder};
use rune::{PhysicalInput, Shortcut, ShortcutError, TextEdit, Tx};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Rules,
    Keybinds,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 3] = [
        SettingsTab::General,
        SettingsTab::Rules,
        SettingsTab::Keybinds,
    ];

    pub fn id(self) -> &'static str {
        match self {
            SettingsTab::General => "general",
            SettingsTab::Rules => "rules",
            SettingsTab::Keybinds => "keybinds",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Rules => "Rules",
            SettingsTab::Keybinds => "Keybinds",
        }
    }
}

pub struct Palette {
    pub query: TextEdit,
    pub selected: usize,
}

impl App {
    pub fn open_palette(&mut self) {
        let mut query = TextEdit::new("");
        query.focused = true;
        self.palette = Some(Palette { query, selected: 0 });
    }

    pub fn close_palette(&mut self) {
        self.palette = None;
    }

    pub fn focus_search(&mut self) {
        self.search.focused = true;
    }

    pub fn palette_edit(&mut self) -> &mut TextEdit {
        &mut self
            .palette
            .as_mut()
            .expect("the open command bar has a field")
            .query
    }

    pub fn palette_actions(&self) -> Vec<Action> {
        let query = self
            .palette
            .as_ref()
            .map(|palette| palette.query.text.trim().to_lowercase())
            .unwrap_or_default();
        Action::ALL
            .into_iter()
            .filter(|action| *action != Action::CommandBar)
            .filter(|action| {
                query.is_empty()
                    || action.label().to_lowercase().contains(&query)
                    || action.id().contains(&query)
            })
            .collect()
    }

    pub fn palette_requery(&mut self, tx: &mut Tx) {
        if let Some(palette) = &mut self.palette {
            palette.selected = 0;
        }
        tx.scroll_to_y("command-scroll", 0.0);
    }

    pub fn palette_move(&mut self, delta: isize, tx: &mut Tx) {
        let actions = self.palette_actions();
        if actions.is_empty() {
            self.palette_requery(tx);
            return;
        }
        let Some(palette) = &mut self.palette else {
            return;
        };
        let last = actions.len() as isize - 1;
        palette.selected = (palette.selected as isize + delta).clamp(0, last) as usize;
        let action = actions[palette.selected];
        tx.scroll_into_view(format!("command-{}", action.id()));
    }

    pub fn run_palette_command(&mut self, tx: &mut Tx) {
        let actions = self.palette_actions();
        let selected = self.palette.as_ref().map(|p| p.selected).unwrap_or(0);
        let action = actions.get(selected).copied();
        self.close_palette();
        if let Some(action) = action {
            self.run_action(action, tx);
        }
    }

    pub fn choose_command(&mut self, action: Action, tx: &mut Tx) {
        self.close_palette();
        self.run_action(action, tx);
    }

    pub fn open_settings(&mut self) {
        if self.settings.is_none() {
            self.settings = Some(SettingsTab::General);
        }
    }

    pub fn open_tab(&mut self, tab: SettingsTab) {
        self.rule_query = None;
        self.rule_reveal = None;
        self.rule_delete_ask = None;
        if tab != SettingsTab::Rules {
            self.rule_edit = None;
        }
        self.settings = Some(tab);
    }

    pub fn dismiss_settings(&mut self) {
        if self.settings == Some(SettingsTab::Rules) && self.rule_edit.is_some() {
            self.discard_rule_edit();
            return;
        }
        self.close_settings();
    }

    pub fn close_settings(&mut self) {
        self.settings = None;
        self.rule_edit = None;
        self.rule_delete_ask = None;
        self.rule_drag = Default::default();
        self.rule_query = None;
        self.rule_reveal = None;
    }

    pub fn open_config_in_editor(&mut self) {
        if let Some(path) = self.store.path(crate::config::CONFIG) {
            let _ = super::paths::open_external(&path);
        }
    }

    pub fn set_binding(
        &mut self,
        action: Action,
        shortcut: Option<&Shortcut>,
    ) -> Result<(), ShortcutError> {
        self.keybinds.set(action, shortcut)?;
        let _ = self.store.edit(crate::config::KEYBINDS, |doc| {
            self.keybinds.write(doc, action)
        });
        Ok(())
    }

    pub fn record_key(&mut self, action: Action, event: &PhysicalInput) -> bool {
        let Some(recorder) = self.recorders.get_mut(&action) else {
            return false;
        };
        match recorder.input(event) {
            Outcome::Pass => false,
            Outcome::Consumed => true,
            Outcome::Change(next) => {
                let same = next.as_ref() == self.keybinds.chord(action);
                let result = if same {
                    Ok(())
                } else {
                    self.set_binding(action, next.as_ref())
                };
                self.recorders
                    .get_mut(&action)
                    .expect("every action has a recorder")
                    .finish(next, result);
                true
            }
        }
    }

    pub fn reset_binding(&mut self, action: Action) {
        self.keybinds.reset(action);
        let _ = self.store.edit(crate::config::KEYBINDS, |doc| {
            self.keybinds.write(doc, action)
        });
        self.recorders.insert(action, Recorder::default());
    }

    pub fn action_enabled(&self, action: Action) -> bool {
        match action {
            Action::Back => self.can_back(),
            Action::Forward => self.can_forward(),
            Action::EnclosingFolder => self.can_up(),
            Action::Rename => self.selection_len() == 1,
            Action::Move | Action::Trash | Action::Copy | Action::Cut | Action::Duplicate => {
                self.selection_len() > 0
            }
            Action::Paste => self.clipboard.is_some() && self.selection_len() > 0,
            Action::ShowEntry | Action::HideEntry | Action::ViewEntryRules => {
                self.selected.is_some()
            }
            _ => true,
        }
    }

    pub fn run_action(&mut self, action: Action, tx: &mut Tx) {
        if !self.action_enabled(action) {
            return;
        }
        self.perform(action, tx);
    }

    pub(super) fn perform(&mut self, action: Action, tx: &mut Tx) {
        match action {
            Action::CommandBar => self.open_palette(),
            Action::FocusSearch => self.focus_search(),
            Action::OpenSettings => self.open_settings(),
            Action::Back => self.back(),
            Action::Forward => self.forward(),
            Action::EnclosingFolder => self.up(),
            Action::Refresh => self.reload(),
            Action::NewFolder => self.new_folder(tx),
            Action::ToggleHidden => self.toggle_hidden(),
            Action::Rename => self.begin_rename(),
            Action::Move => self.begin_move(),
            Action::Trash => self.begin_trash(),
            Action::Copy => self.copy_selection(false),
            Action::Cut => self.copy_selection(true),
            Action::Paste => self.paste(),
            Action::Duplicate => self.duplicate_selection(),
            Action::ViewFolderRules => self.show_folder_rules(),
            Action::ViewEntryRules => self.show_entry_rules(),
            Action::SaveLayoutRule => self.save_layout_rule(),
            Action::ShowEntry => self.pin_entry(false),
            Action::HideEntry => self.pin_entry(true),
            Action::PickRule => self.open_rule_picker(),
        }
    }
}
