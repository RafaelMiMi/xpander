use anyhow::Result;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CenterBox, HeaderBar,
    Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, SelectionMode, Switch,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::config::{Config, ConfigManager, Snippet};

use super::editor::SnippetEditor;

/// Shared state for the config window
struct WindowState {
    config: Config,
    config_path: PathBuf,
}

/// The main configuration window
pub struct ConfigWindow {
    window: ApplicationWindow,
    list_box: ListBox,
    stats_label: Label,
    state: Rc<RefCell<WindowState>>,
}

impl ConfigWindow {
    /// Create a new configuration window
    pub fn new(app: &Application, config_path: PathBuf) -> Result<Self> {
        // Load config synchronously
        let config = ConfigManager::load_config(&config_path)?;

        let state = Rc::new(RefCell::new(WindowState {
            config,
            config_path,
        }));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Xpander - Text Expansion")
            .default_width(600)
            .default_height(500)
            .build();

        // Create header bar
        let header = HeaderBar::new();

        let add_button = Button::with_label("Add");
        add_button.add_css_class("suggested-action");
        header.pack_start(&add_button);

        window.set_titlebar(Some(&header));

        // Main content
        let main_box = GtkBox::new(Orientation::Vertical, 0);

        // Toolbar with enable switch
        let toolbar = CenterBox::new();
        toolbar.set_margin_start(12);
        toolbar.set_margin_end(12);
        toolbar.set_margin_top(8);
        toolbar.set_margin_bottom(8);

        let enable_box = GtkBox::new(Orientation::Horizontal, 8);
        let enable_label = Label::new(Some("Enable Expansions"));
        let enable_switch = Switch::new();
        enable_switch.set_active(state.borrow().config.settings.enabled);
        enable_box.append(&enable_label);
        enable_box.append(&enable_switch);
        toolbar.set_start_widget(Some(&enable_box));

        let snippet_count = state.borrow().config.snippets.len();
        let stats_label = Label::new(Some(&format!("{} snippets", snippet_count)));
        stats_label.add_css_class("dim-label");
        toolbar.set_end_widget(Some(&stats_label));

        main_box.append(&toolbar);

        // Scrolled list of snippets
        let scrolled = ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .build();

        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("boxed-list");
        list_box.set_margin_start(12);
        list_box.set_margin_end(12);
        list_box.set_margin_bottom(12);

        scrolled.set_child(Some(&list_box));
        main_box.append(&scrolled);

        window.set_child(Some(&main_box));

        let config_window = Self {
            window,
            list_box,
            stats_label,
            state,
        };

        // Load existing snippets into the list
        config_window.refresh_list();

        // Connect signals
        config_window.setup_signals(&add_button, &enable_switch);

        Ok(config_window)
    }

    /// Refresh the snippet list from state
    fn refresh_list(&self) {
        // Clear existing rows
        while let Some(row) = self.list_box.row_at_index(0) {
            self.list_box.remove(&row);
        }

        // Add rows for each snippet
        let state = self.state.borrow();
        for (index, snippet) in state.config.snippets.iter().enumerate() {
            Self::add_snippet_row(&self.list_box, snippet, index);
        }

        self.stats_label.set_text(&format!("{} snippets", state.config.snippets.len()));
    }

    /// Save config to file
    fn save_config(&self) -> Result<()> {
        let state = self.state.borrow();
        ConfigManager::save_config(&state.config_path, &state.config)?;
        log::info!("Configuration saved");
        Ok(())
    }

    /// Set up signal handlers
    fn setup_signals(&self, add_button: &Button, enable_switch: &Switch) {
        // Add button - open editor dialog
        let window = self.window.clone();
        let list_box = self.list_box.clone();
        let stats_label = self.stats_label.clone();
        let state = self.state.clone();

        add_button.connect_clicked(move |_| {
            let editor = SnippetEditor::new(&window, None);

            let list_box = list_box.clone();
            let stats_label = stats_label.clone();
            let state = state.clone();

            editor.connect_save(move |snippet| {
                {
                    let mut s = state.borrow_mut();
                    s.config.snippets.push(snippet.clone());

                    // Save to file
                    if let Err(e) = ConfigManager::save_config(&s.config_path, &s.config) {
                        log::error!("Failed to save config: {}", e);
                    }
                }

                let count = state.borrow().config.snippets.len();
                Self::add_snippet_row(&list_box, &snippet, count - 1);
                stats_label.set_text(&format!("{} snippets", count));
            });

            editor.show();
        });

        // Enable switch
        let state = self.state.clone();
        enable_switch.connect_state_set(move |_, active| {
            {
                let mut s = state.borrow_mut();
                s.config.settings.enabled = active;

                if let Err(e) = ConfigManager::save_config(&s.config_path, &s.config) {
                    log::error!("Failed to save config: {}", e);
                }
            }
            glib::Propagation::Proceed
        });

        // Double-click to edit
        let window = self.window.clone();
        let list_box = self.list_box.clone();
        let state = self.state.clone();

        self.list_box.connect_row_activated(move |_, row| {
            let index = row.index() as usize;

            let snippet = {
                let s = state.borrow();
                s.config.snippets.get(index).cloned()
            };

            if let Some(snippet) = snippet {
                let editor = SnippetEditor::new(&window, Some(snippet));
                let state = state.clone();
                let list_box = list_box.clone();
                let row_index = index;

                editor.connect_save(move |updated_snippet| {
                    {
                        let mut s = state.borrow_mut();
                        if row_index < s.config.snippets.len() {
                            s.config.snippets[row_index] = updated_snippet.clone();

                            if let Err(e) = ConfigManager::save_config(&s.config_path, &s.config) {
                                log::error!("Failed to save config: {}", e);
                            }
                        }
                    }

                    // Refresh the specific row
                    if let Some(row) = list_box.row_at_index(row_index as i32) {
                        // Update the row content
                        let hbox = GtkBox::new(Orientation::Horizontal, 12);
                        hbox.set_margin_start(12);
                        hbox.set_margin_end(12);
                        hbox.set_margin_top(8);
                        hbox.set_margin_bottom(8);

                        let trigger_label = Label::new(Some(&updated_snippet.trigger));
                        trigger_label.add_css_class("monospace");
                        trigger_label.set_xalign(0.0);
                        trigger_label.set_width_chars(15);
                        hbox.append(&trigger_label);

                        let arrow = Label::new(Some("→"));
                        arrow.add_css_class("dim-label");
                        hbox.append(&arrow);

                        let replace_text = updated_snippet.replace.lines().next().unwrap_or("");
                        let display_text = if replace_text.len() > 40 {
                            format!("{}...", &replace_text[..40])
                        } else if updated_snippet.replace.contains('\n') {
                            format!("{}...", replace_text)
                        } else {
                            replace_text.to_string()
                        };

                        let replace_label = Label::new(Some(&display_text));
                        replace_label.set_xalign(0.0);
                        replace_label.set_hexpand(true);
                        replace_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                        hbox.append(&replace_label);

                        let enable_switch = Switch::new();
                        enable_switch.set_active(updated_snippet.enabled);
                        enable_switch.set_valign(gtk4::Align::Center);
                        hbox.append(&enable_switch);

                        row.set_child(Some(&hbox));
                    }
                });

                editor.show();
            }
        });
    }

    /// Add a snippet row to the list
    fn add_snippet_row(list_box: &ListBox, snippet: &Snippet, _index: usize) {
        let row = ListBoxRow::new();

        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);

        // Trigger
        let trigger_label = Label::new(Some(&snippet.trigger));
        trigger_label.add_css_class("monospace");
        trigger_label.set_xalign(0.0);
        trigger_label.set_width_chars(15);
        hbox.append(&trigger_label);

        // Arrow
        let arrow = Label::new(Some("→"));
        arrow.add_css_class("dim-label");
        hbox.append(&arrow);

        // Replacement (truncated)
        let replace_text = snippet.replace.lines().next().unwrap_or("");
        let display_text = if replace_text.len() > 40 {
            format!("{}...", &replace_text[..40])
        } else if snippet.replace.contains('\n') {
            format!("{}...", replace_text)
        } else {
            replace_text.to_string()
        };

        let replace_label = Label::new(Some(&display_text));
        replace_label.set_xalign(0.0);
        replace_label.set_hexpand(true);
        replace_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        hbox.append(&replace_label);

        // Enable switch
        let enable_switch = Switch::new();
        enable_switch.set_active(snippet.enabled);
        enable_switch.set_valign(gtk4::Align::Center);
        hbox.append(&enable_switch);

        row.set_child(Some(&hbox));
        list_box.append(&row);
    }

    /// Show the window
    pub fn show(&self) {
        self.window.present();
    }
}

/// Create and run the GTK application for the config window
pub fn create_config_app() -> Application {
    let app = Application::builder()
        .application_id("com.xpander.config")
        .build();

    app.connect_activate(move |app| {
        // Get config path
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("xpander")
            .join("config.yaml");

        match ConfigWindow::new(app, config_path) {
            Ok(window) => {
                window.show();
            }
            Err(e) => {
                log::error!("Failed to create config window: {}", e);
                eprintln!("Error: Failed to load config: {}", e);
            }
        }
    });

    app
}
