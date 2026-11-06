use crate::app::App;
use crate::config;
use crate::keybinds::{Action, Keybinds};
use crate::settings::Setting;
use actions::{Outcome, Recorder};
use actions_ui::{KeybindHost, Palette, PaletteHost};
use rune::{El, PhysicalInput, Shortcut, ShortcutError, Tx, col};

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

impl App {
    pub fn open_palette(&mut self) {
        self.palette = Some(Palette::open());
    }

    pub fn close_palette(&mut self) {
        self.palette = None;
    }

    pub fn focus_search(&mut self) {
        self.search.focused = true;
    }

    #[cfg(test)]
    pub fn palette_actions(&self) -> Vec<Action> {
        self.palette
            .as_ref()
            .map(Palette::actions::<Action>)
            .unwrap_or_default()
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
        // The chord is set in memory; a failed write surfaces through `keybinds_error`, so the
        // editor does not show a chord the next launch will not have.
        let _ = self.store.edit(crate::config::KEYBINDS, |doc| {
            actions_kdl::keybinds::set(doc, &self.keybinds, action)
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
            actions_kdl::keybinds::set(doc, &self.keybinds, action)
        });
        self.recorders.insert(action, Recorder::default());
    }

    pub fn action_enabled(&self, action: Action) -> bool {
        match action {
            Action::Back => self.can_back(),
            Action::Forward => self.can_forward(),
            Action::EnclosingFolder => self.can_up(),
            Action::Rename => self.selection_len() > 0,
            Action::CopyPath | Action::CopyName => self.selection_len() > 0,
            Action::Move
            | Action::Trash
            | Action::Delete
            | Action::Copy
            | Action::Cut
            | Action::Duplicate
            | Action::MakeAlias => self.selection_len() > 0,
            Action::Paste => self.clipboard.is_some(),
            Action::SelectAll => !self.visible.is_empty(),
            Action::OpenTerminal => crate::vfs::is_dir(&self.cwd),
            Action::Reveal => self.selected.is_some(),
            Action::DeselectAll | Action::InvertSelection => self.selection_len() > 0,
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
            Action::GoToFolder => self.begin_go_to(),
            Action::Refresh => self.reload(),
            Action::NewFolder => self.new_folder(tx),
            Action::NewFile => self.new_file(tx),
            Action::ToggleHidden => self.toggle_hidden(),
            Action::Rename => self.begin_rename(),
            Action::Move => self.begin_move(),
            Action::Trash => self.begin_trash(),
            Action::Delete => self.begin_delete(),
            Action::Copy => self.copy_selection(false),
            Action::CopyPath => self.copy_paths(tx),
            Action::CopyName => self.copy_names(tx),
            Action::AddPlace => self.add_current_place(),
            Action::MakeAlias => self.make_aliases(),
            Action::ToggleLayout => self.toggle_layout(),
            Action::ToggleInspector => self.toggle_setting(Setting::Inspector),
            Action::ToggleFoldersFirst => self.toggle_setting(Setting::FoldersFirst),
            Action::Cut => self.copy_selection(true),
            Action::Paste => self.paste(),
            Action::Duplicate => self.duplicate_selection(),
            Action::SelectAll => self.select_all(),
            Action::DeselectAll => self.deselect_all(),
            Action::InvertSelection => self.invert_selection(),
            Action::ViewFolderRules => self.show_folder_rules(),
            Action::ViewEntryRules => self.show_entry_rules(),
            Action::SaveLayoutRule => self.save_layout_rule(),
            Action::ShowEntry => self.pin_entry(false),
            Action::HideEntry => self.pin_entry(true),
            Action::PickRule => self.open_rule_picker(),
            Action::OpenTerminal => self.toggle_terminal(),
            Action::Reveal => self.reveal_selected(),
        }
    }
}

impl PaletteHost<Action> for App {
    fn palette(&self) -> &Palette {
        self.palette
            .as_ref()
            .expect("the open command bar has a field")
    }

    fn palette_mut(&mut self) -> &mut Palette {
        self.palette
            .as_mut()
            .expect("the open command bar has a field")
    }

    fn keybinds(&self) -> &Keybinds {
        &self.keybinds
    }

    fn palette_action(&self) -> Action {
        Action::CommandBar
    }

    fn palette_enabled(&self, action: Action) -> bool {
        self.action_enabled(action)
    }

    fn palette_run(&mut self, action: Action, tx: &mut Tx) {
        self.run_action(action, tx);
    }

    fn palette_close(&mut self) {
        self.close_palette();
    }

    fn palette_scroll(&self, content: El) -> El {
        scroll::area(
            self,
            "command-scroll",
            col()
                .w_full()
                .max_h(380.0)
                .scroll_edge_fade(12.0)
                .scroll_margin(
                    actions_ui::palette::CLEARANCE_ROWS
                        * (actions_ui::palette::ROW_HEIGHT + actions_ui::palette::ROW_GAP),
                ),
            content,
        )
    }
}

impl KeybindHost<Action> for App {
    fn recorder(&self, action: Action) -> Option<&Recorder> {
        self.recorders.get(&action)
    }

    fn begin_recording(&mut self, action: Action) {
        self.recorders.entry(action).or_default().begin();
    }

    fn keybinds(&self) -> &Keybinds {
        &self.keybinds
    }

    fn record_key(&mut self, action: Action, event: &PhysicalInput) -> bool {
        App::record_key(self, action, event)
    }

    fn reset_binding(&mut self, action: Action) {
        App::reset_binding(self, action);
    }

    fn keybinds_saved_in(&self) -> Option<String> {
        self.store
            .path(config::KEYBINDS)
            .map(|path| path.display().to_string())
    }

    fn keybinds_error(&self) -> Option<String> {
        self.store
            .write_error
            .clone()
            .or_else(|| self.store.error(config::KEYBINDS).map(str::to_owned))
    }
}
