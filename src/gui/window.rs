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

use crate::config::{Config, ConfigManager, SnippetNode};

use super::editor::{SnippetEditor, show_import_dialog, show_export_dialog, show_confirm_dialog, show_input_dialog};

/// Shared state for the config window
struct WindowState {
    config: Config,
    config_path: PathBuf,
    current_path: Vec<usize>, // Path of indices to current folder
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
            current_path: Vec::new(),
        }));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Xpander - Text Expansion")
            .default_width(600)
            .default_height(500)
            .build();

        // Create header bar
        let header = HeaderBar::new();

        let back_button = Button::with_label("Back");
        back_button.set_icon_name("go-previous-symbolic");
        back_button.set_visible(false); // Hidden by default
        header.pack_start(&back_button);

        let add_button = Button::with_label("Add");
        add_button.add_css_class("suggested-action");
        header.pack_start(&add_button);

        let add_folder_button = Button::with_label("New Folder");
        header.pack_start(&add_folder_button);

        let import_button = Button::with_label("Import");
        header.pack_start(&import_button);

        let export_button = Button::with_label("Export");
        header.pack_start(&export_button);

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
        list_box.set_activate_on_single_click(true);
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

        // Connect signals and get refresh function
        let refresh = config_window.setup_signals(&back_button, &add_button, &add_folder_button, &import_button, &export_button, &enable_switch);
        
        // Initial refresh
        refresh();

        Ok(config_window)
    }


    fn setup_signals(
        &self,
        back_button: &Button,
        add_button: &Button,
        add_folder_button: &Button,
        import_button: &Button,
        export_button: &Button,
        enable_switch: &Switch,
    ) -> Rc<dyn Fn()> {
        // Shared state refs
        let state = self.state.clone();
        let list_box = self.list_box.clone();
        let back_btn_clone = back_button.clone();
        let stats_label = self.stats_label.clone();
        let window = self.window.clone();

        // Refresh function
        type RefreshFn = Box<dyn Fn()>;
        let refresh_cell: Rc<RefCell<Option<RefreshFn>>> = Rc::new(RefCell::new(None));
        let refresh_weak = Rc::downgrade(&refresh_cell);

        let refresh_impl = {
            let state = state.clone();
            let list_box = list_box.clone();
            let back_button = back_btn_clone;
            let stats_label = stats_label.clone();
            let refresh_weak_inner = refresh_weak.clone();
            let window = window.clone();

            move || {
                // Clear existing rows
                while let Some(row) = list_box.row_at_index(0) {
                    list_box.remove(&row);
                }

                let state_borrow = state.borrow();
                
                // Show/hide back button based on path
                back_button.set_visible(!state_borrow.current_path.is_empty());

                // Resolve current list
                let mut current_list = &state_borrow.config.snippets;
                let mut valid_path = true;
                
                for &idx in &state_borrow.current_path {
                    if let Some(SnippetNode::Folder(folder)) = current_list.get(idx) {
                        current_list = &folder.items;
                    } else {
                        valid_path = false;
                        break;
                    }
                }

                if valid_path {
                    for (index, node) in current_list.iter().enumerate() {
                        let state = state.clone();
                        let refresh_weak = refresh_weak_inner.clone();
                        let window = window.clone();

                        // Callback for deletion
                        let on_delete = {
                            let state = state.clone();
                            let refresh_weak = refresh_weak.clone(); 
                            let window = window.clone();
                            move || {
                                let state = state.clone();
                                let refresh_weak = refresh_weak.clone();
                                show_confirm_dialog(&window, "Delete Item", "Are you sure you want to delete this item?", move |confirmed| {
                                    if confirmed {
                                        {
                                            let mut s = state.borrow_mut();
                                            let path = s.current_path.clone();
                                            if let Some(list) = get_list_at_path_mut(&mut s.config.snippets, &path) {
                                                if index < list.len() {
                                                    list.remove(index);
                                                    let _ = ConfigManager::save_config(&s.config_path, &s.config);
                                                }
                                            }
                                        }
                                        if let Some(cell) = refresh_weak.upgrade() {
                                            if let Some(refresh) = cell.borrow().as_ref() {
                                                refresh();
                                            }
                                        }
                                    }
                                });
                            }
                        };

                        // Callback for editing (Rename folder)
                        let on_edit = {
                            let state = state.clone();
                            let refresh_weak = refresh_weak.clone();
                            let window = window.clone();
                            let node_name = if let SnippetNode::Folder(f) = node { f.folder.clone() } else { "".to_string() };
                            
                            move || {
                                show_input_dialog(&window, "Rename Folder", &node_name, {
                                    let state = state.clone();
                                    let refresh_weak = refresh_weak.clone();
                                    move |result| {
                                        if let Some(new_name) = result {
                                             {
                                                let mut s = state.borrow_mut();
                                                let path = s.current_path.clone();
                                                if let Some(list) = get_list_at_path_mut(&mut s.config.snippets, &path) {
                                                    if let Some(SnippetNode::Folder(f)) = list.get_mut(index) {
                                                        f.folder = new_name;
                                                        let _ = ConfigManager::save_config(&s.config_path, &s.config);
                                                    }
                                                }
                                            }
                                            if let Some(cell) = refresh_weak.upgrade() {
                                                if let Some(refresh) = cell.borrow().as_ref() {
                                                    refresh();
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        };
                        
                        ConfigWindow::add_snippet_node_row(&list_box, node, index, on_delete, on_edit);
                    }
                }
                
                let total = ConfigManager::flatten_snippets(&state_borrow.config.snippets).len();
                stats_label.set_text(&format!("{} snippets (total)", total));
            }
        };

        // Assign the implementation
        *refresh_cell.borrow_mut() = Some(Box::new(refresh_impl));

        // Create a callable wrapper
        let refresh = {
             let cell = refresh_cell.clone();
             Rc::new(move || {
                 if let Some(f) = cell.borrow().as_ref() {
                     f();
                 }
             })
        };

        // Back button
        let state_clone = state.clone();
        let refresh_clone = refresh.clone();
        back_button.connect_clicked(move |_| {
            state_clone.borrow_mut().current_path.pop();
            refresh_clone();
        });

        // Add Folder Button
        let window = self.window.clone();
        let state = self.state.clone();
        let refresh_clone = refresh.clone();
        
        add_folder_button.connect_clicked(move |_| {
            let state = state.clone();
            let refresh = refresh_clone.clone();
            show_input_dialog(&window, "New Folder", "", move |result| {
                if let Some(name) = result {
                    {
                        let mut s = state.borrow_mut();
                        let path = s.current_path.clone();
                        if let Some(list) = get_list_at_path_mut(&mut s.config.snippets, &path) {
                            list.push(SnippetNode::Folder(crate::config::Folder {
                                folder: name,
                                items: Vec::new(),
                                enabled: true,
                            }));
                            let _ = ConfigManager::save_config(&s.config_path, &s.config);
                        }
                    }
                    refresh();
                }
            });
        });

        // Add Snippet Button
        let window = self.window.clone();
        let state = self.state.clone();
        let refresh_clone = refresh.clone();

        add_button.connect_clicked(move |_| {
            let editor = SnippetEditor::new(&window, None);
            let state = state.clone();
            let refresh = refresh_clone.clone();

            editor.connect_save(move |snippet| {
                {
                    let mut s = state.borrow_mut();
                    let path = s.current_path.clone();
                    let current_list_opt = get_list_at_path_mut(&mut s.config.snippets, &path);
                    
                    if let Some(current_list) = current_list_opt {
                         current_list.push(SnippetNode::Snippet(snippet.clone()));
                        let _ = ConfigManager::save_config(&s.config_path, &s.config);
                    }
                }
                refresh();
            });
            editor.show();
        });
        
        // Import
        let window = self.window.clone();
        let state = self.state.clone();
        let refresh_clone = refresh.clone();
        
        import_button.connect_clicked(move |_| {
            let state = state.clone();
            let refresh = refresh_clone.clone();
            
            show_import_dialog(&window, move |path| {
                match crate::config::loader::import_custom_entries(&path) {
                    Ok(data) => {
                        let new_snippets = data.snippets;
                        {
                            let mut s = state.borrow_mut();
                            s.config.snippets.extend(new_snippets);
                             // Merge variables logic skipped for brevity, check previous
                            match (&mut s.config.variables, data.variables) {
                                (serde_yaml::Value::Mapping(map), serde_yaml::Value::Mapping(new_map)) => {
                                    for (k, v) in new_map { map.insert(k, v); }
                                },
                                (current, new) => { *current = new; }
                            }
                            let _ = ConfigManager::save_config(&s.config_path, &s.config);
                        }
                        refresh();
                    }
                    Err(e) => {
                        log::error!("Failed to import: {}", e);
                    }
                }
            });
        });
        
        // Export
        let window = self.window.clone();
        let state = self.state.clone();

        export_button.connect_clicked(move |_| {
            let state = state.clone();
            show_export_dialog(&window, move |path| {
                let s = state.borrow();
                if let Err(e) = crate::config::loader::export_custom_entries(&s.config.snippets, &s.config.variables, &path) {
                    log::error!("Failed to export entries: {}", e);
                }
            });
        });

        // Enable Switch
        let state = self.state.clone();
        enable_switch.connect_state_set(move |_, active| {
            {
                let mut s = state.borrow_mut();
                s.config.settings.enabled = active;
                let _ = ConfigManager::save_config(&s.config_path, &s.config);
            }
            glib::Propagation::Proceed
        });

        // Row interaction
        let window = self.window.clone();
        let state = self.state.clone();
        let refresh_clone = refresh.clone();

        self.list_box.connect_row_activated(move |_, row| {
            let index = row.index() as usize;

            let node = {
                let s = state.borrow();
                 let mut current_list = &s.config.snippets;
                 for &idx in &s.current_path {
                    if let Some(SnippetNode::Folder(f)) = current_list.get(idx) {
                        current_list = &f.items;
                    }
                 }
                current_list.get(index).cloned()
            };

            if let Some(node) = node {
                match node {
                    crate::config::SnippetNode::Snippet(snippet) => {
                        let editor = SnippetEditor::new(&window, Some(snippet));
                        let state = state.clone();
                        let refresh = refresh_clone.clone();
                        let row_index = index;

                        editor.connect_save(move |updated_snippet| {
                            {
                                let mut s = state.borrow_mut();
                                let path = s.current_path.clone();
                                if let Some(list) = get_list_at_path_mut(&mut s.config.snippets, &path) {
                                     if let Some(SnippetNode::Snippet(_)) = list.get(row_index) {
                                         list[row_index] = SnippetNode::Snippet(updated_snippet.clone());
                                         let _ = ConfigManager::save_config(&s.config_path, &s.config);
                                     }
                                }
                            }
                            refresh();
                        });
                        editor.show();
                    }
                    crate::config::SnippetNode::Folder(_) => {
                        state.borrow_mut().current_path.push(index);
                        refresh_clone();
                    }
                }
            }
        });
        
        refresh
    }

    /// Add a snippet node row to the list
    fn add_snippet_node_row(
        list_box: &ListBox,
        node: &crate::config::SnippetNode,
        _index: usize,
        on_delete: impl Fn() + 'static,
        on_edit: impl Fn() + 'static,
    ) {
        let row = ListBoxRow::new();
        let (child, delete_btn, edit_btn) = Self::create_node_widget(node);
        
        delete_btn.connect_clicked(move |_| on_delete());
        
        if let Some(edit_btn) = edit_btn {
            edit_btn.connect_clicked(move |_| on_edit());
        }
        
        row.set_child(Some(&child));
        list_box.append(&row);
    }
    
    /// Helper to create widget content for a node
    fn create_node_widget(node: &crate::config::SnippetNode) -> (GtkBox, Button, Option<Button>) {
        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);

        let edit_btn_opt;

        match node {
            crate::config::SnippetNode::Snippet(snippet) => {
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
                let display_text = if replace_text.len() > 30 {
                    format!("{}...", &replace_text[..30])
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
                enable_switch.set_sensitive(false); 
                hbox.append(&enable_switch);
                
                edit_btn_opt = None;
            },
            crate::config::SnippetNode::Folder(folder) => {
                // Folder icon/label
                let folder_label = Label::new(Some("📁"));
                folder_label.set_width_chars(3);
                hbox.append(&folder_label);
                
                let name_label = Label::new(Some(&folder.folder));
                name_label.add_css_class("title-4");
                name_label.set_xalign(0.0);
                name_label.set_hexpand(true);
                hbox.append(&name_label);
                
                let count_label = Label::new(Some(&format!("{} items", folder.items.len())));
                count_label.add_css_class("dim-label");
                hbox.append(&count_label);
                
                // Edit button for renaming
                let edit_btn = Button::from_icon_name("document-edit-symbolic");
                edit_btn.add_css_class("flat");
                edit_btn.set_tooltip_text(Some("Rename Folder"));
                hbox.append(&edit_btn);
                edit_btn_opt = Some(edit_btn);
            }
        }
        
        // Delete button
        let delete_btn = Button::from_icon_name("user-trash-symbolic");
        delete_btn.add_css_class("flat");
        delete_btn.add_css_class("destructive-action");
        delete_btn.set_tooltip_text(Some("Delete"));
        hbox.append(&delete_btn);
        
        (hbox, delete_btn, edit_btn_opt)
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

/// Helper to get mutable reference to the list at a specific path
fn get_list_at_path_mut<'a>(
    root: &'a mut Vec<crate::config::SnippetNode>,
    path: &[usize],
) -> Option<&'a mut Vec<crate::config::SnippetNode>> {
    if path.is_empty() {
        return Some(root);
    }
    
    // We need to split the path
    let (idx, rest) = path.split_first()?;
    
    if let Some(crate::config::SnippetNode::Folder(folder)) = root.get_mut(*idx) {
        get_list_at_path_mut(&mut folder.items, rest)
    } else {
        None
    }
}
