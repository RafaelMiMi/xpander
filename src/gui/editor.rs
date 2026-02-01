use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, CheckButton, Dialog, DialogFlags, Entry, Frame, Label,
    Orientation, ResponseType, ScrolledWindow, TextBuffer, TextView, Window,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::config::Snippet;

/// Dialog for creating or editing a snippet
pub struct SnippetEditor {
    dialog: Dialog,
    trigger_entry: Entry,
    replace_buffer: TextBuffer,
    label_entry: Entry,
    propagate_case: CheckButton,
    cursor_position: CheckButton,
    word_boundary: CheckButton,
    regex_check: CheckButton,
    enabled_check: CheckButton,
    on_save: Rc<RefCell<Option<Box<dyn Fn(Snippet)>>>>,
}

impl SnippetEditor {
    /// Create a new snippet editor dialog
    pub fn new(parent: &impl IsA<Window>, existing: Option<Snippet>) -> Self {
        let title = if existing.is_some() {
            "Edit Snippet"
        } else {
            "New Snippet"
        };

        let dialog = Dialog::with_buttons(
            Some(title),
            Some(parent),
            DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
            &[
                ("Cancel", ResponseType::Cancel),
                ("Save", ResponseType::Accept),
            ],
        );

        dialog.set_default_width(500);
        dialog.set_default_height(400);

        // Make Save button suggested action
        if let Some(button) = dialog.widget_for_response(ResponseType::Accept) {
            button.add_css_class("suggested-action");
        }

        let content = dialog.content_area();
        content.set_spacing(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);

        // Trigger field
        let trigger_box = GtkBox::new(Orientation::Vertical, 4);
        let trigger_label = Label::new(Some("Trigger"));
        trigger_label.set_xalign(0.0);
        trigger_label.add_css_class("heading");
        let trigger_entry = Entry::new();
        trigger_entry.set_placeholder_text(Some("e.g., ;email"));
        trigger_entry.add_css_class("monospace");
        trigger_box.append(&trigger_label);
        trigger_box.append(&trigger_entry);
        content.append(&trigger_box);

        // Replacement field
        let replace_frame = Frame::new(Some("Replacement"));
        let scrolled = ScrolledWindow::builder()
            .min_content_height(120)
            .build();
        let replace_view = TextView::new();
        replace_view.set_monospace(true);
        replace_view.set_wrap_mode(gtk4::WrapMode::Word);
        replace_view.set_left_margin(8);
        replace_view.set_right_margin(8);
        replace_view.set_top_margin(8);
        replace_view.set_bottom_margin(8);
        let replace_buffer = replace_view.buffer();
        scrolled.set_child(Some(&replace_view));
        replace_frame.set_child(Some(&scrolled));
        content.append(&replace_frame);

        // Help text for variables
        let help_label = Label::new(Some(
            "Variables: {{date}}, {{time}}, {{clipboard}}, {{env:VAR}}, {{shell:cmd}}, {{random:N}}\n\
             Cursor position: $|$"
        ));
        help_label.set_xalign(0.0);
        help_label.add_css_class("dim-label");
        help_label.set_wrap(true);
        content.append(&help_label);

        // Label field (optional)
        let label_box = GtkBox::new(Orientation::Vertical, 4);
        let label_label = Label::new(Some("Label (optional)"));
        label_label.set_xalign(0.0);
        let label_entry = Entry::new();
        label_entry.set_placeholder_text(Some("Description for this snippet"));
        label_box.append(&label_label);
        label_box.append(&label_entry);
        content.append(&label_box);

        // Options
        let options_frame = Frame::new(Some("Options"));
        let options_box = GtkBox::new(Orientation::Vertical, 8);
        options_box.set_margin_start(12);
        options_box.set_margin_end(12);
        options_box.set_margin_top(8);
        options_box.set_margin_bottom(8);

        let propagate_case = CheckButton::with_label("Propagate case from trigger");
        let cursor_position = CheckButton::with_label("Position cursor at $|$ marker");
        let word_boundary = CheckButton::with_label("Only match at word boundaries");
        let regex_check = CheckButton::with_label("Use regex matching");
        let enabled_check = CheckButton::with_label("Enabled");
        enabled_check.set_active(true);

        options_box.append(&propagate_case);
        options_box.append(&cursor_position);
        options_box.append(&word_boundary);
        options_box.append(&regex_check);
        options_box.append(&enabled_check);

        options_frame.set_child(Some(&options_box));
        content.append(&options_frame);

        // Fill in existing values if editing
        if let Some(snippet) = &existing {
            trigger_entry.set_text(&snippet.trigger);
            replace_buffer.set_text(&snippet.replace);
            if let Some(label) = &snippet.label {
                label_entry.set_text(label);
            }
            propagate_case.set_active(snippet.propagate_case);
            cursor_position.set_active(snippet.cursor_position);
            word_boundary.set_active(snippet.word_boundary);
            regex_check.set_active(snippet.regex);
            enabled_check.set_active(snippet.enabled);
        }

        let editor = Self {
            dialog,
            trigger_entry,
            replace_buffer,
            label_entry,
            propagate_case,
            cursor_position,
            word_boundary,
            regex_check,
            enabled_check,
            on_save: Rc::new(RefCell::new(None)),
        };

        editor.setup_response();
        editor
    }

    /// Set up response handling
    fn setup_response(&self) {
        let trigger_entry = self.trigger_entry.clone();
        let replace_buffer = self.replace_buffer.clone();
        let label_entry = self.label_entry.clone();
        let propagate_case = self.propagate_case.clone();
        let cursor_position = self.cursor_position.clone();
        let word_boundary = self.word_boundary.clone();
        let regex_check = self.regex_check.clone();
        let enabled_check = self.enabled_check.clone();
        let on_save = self.on_save.clone();

        self.dialog.connect_response(move |dialog, response| {
            if response == ResponseType::Accept {
                let trigger = trigger_entry.text().to_string();

                // Validate
                if trigger.is_empty() {
                    // Show error (in a real app, you'd highlight the field)
                    log::warn!("Trigger cannot be empty");
                    return;
                }

                let (start, end) = replace_buffer.bounds();
                let replace = replace_buffer.text(&start, &end, true).to_string();

                let label = {
                    let text = label_entry.text();
                    if text.is_empty() {
                        None
                    } else {
                        Some(text.to_string())
                    }
                };

                let snippet = Snippet {
                    trigger,
                    replace,
                    label,
                    propagate_case: propagate_case.is_active(),
                    cursor_position: cursor_position.is_active(),
                    word_boundary: word_boundary.is_active(),
                    regex: regex_check.is_active(),
                    applications: None,
                    exclude_applications: None,
                    enabled: enabled_check.is_active(),
                };

                // Call the save callback
                if let Some(callback) = on_save.borrow().as_ref() {
                    callback(snippet);
                }
            }

            dialog.close();
        });
    }

    /// Connect a callback for when the snippet is saved
    pub fn connect_save<F: Fn(Snippet) + 'static>(&self, callback: F) {
        *self.on_save.borrow_mut() = Some(Box::new(callback));
    }

    /// Show the dialog
    pub fn show(&self) {
        self.dialog.present();
    }
}

/// Simple dialog for importing snippets
pub fn show_import_dialog<F>(parent: &impl IsA<Window>, on_selected: F)
where
    F: Fn(std::path::PathBuf) + 'static,
{
    let dialog = gtk4::FileChooserDialog::new(
        Some("Import Snippets"),
        Some(parent),
        gtk4::FileChooserAction::Open,
        &[
            ("Cancel", ResponseType::Cancel),
            ("Import", ResponseType::Accept),
        ],
    );

    let filter = gtk4::FileFilter::new();
    filter.add_pattern("*.yaml");
    filter.add_pattern("*.yml");
    filter.set_name(Some("YAML files"));
    dialog.add_filter(&filter);

    dialog.connect_response(move |d, response| {
        if response == ResponseType::Accept {
            if let Some(file) = d.file() {
                if let Some(path) = file.path() {
                    on_selected(path);
                }
            }
        }
        d.close();
    });

    dialog.present();
}

/// Simple dialog for exporting snippets
pub fn show_export_dialog<F>(parent: &impl IsA<Window>, on_selected: F)
where
    F: Fn(std::path::PathBuf) + 'static,
{
    let dialog = gtk4::FileChooserDialog::new(
        Some("Export Snippets"),
        Some(parent),
        gtk4::FileChooserAction::Save,
        &[
            ("Cancel", ResponseType::Cancel),
            ("Export", ResponseType::Accept),
        ],
    );

    let filter = gtk4::FileFilter::new();
    filter.add_pattern("*.yaml");
    filter.set_name(Some("YAML files"));
    dialog.add_filter(&filter);

    dialog.connect_response(move |d, response| {
        if response == ResponseType::Accept {
            if let Some(file) = d.file() {
                if let Some(path) = file.path() {
                    on_selected(path);
                }
            }
        }
        d.close();
    });

    dialog.present();
}

/// Show a confirmation dialog
pub fn show_confirm_dialog<F>(
    parent: &impl IsA<Window>,
    title: &str,
    message: &str,
    on_response: F,
) where
    F: Fn(bool) + 'static,
{
    let dialog = gtk4::MessageDialog::new(
        Some(parent),
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        gtk4::MessageType::Question,
        gtk4::ButtonsType::YesNo,
        message,
    );
    dialog.set_title(Some(title));

    dialog.connect_response(move |d, response| {
        let confirmed = response == ResponseType::Yes;
        d.close();
        on_response(confirmed);
    });

    dialog.present();
}

/// Show a simple input dialog (e.g. for folder names)
pub fn show_input_dialog<F>(
    parent: &impl IsA<Window>,
    title: &str,
    initial_text: &str,
    on_response: F,
) where
    F: Fn(Option<String>) + 'static,
{
    let dialog = Dialog::with_buttons(
        Some(title),
        Some(parent),
        DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("Cancel", ResponseType::Cancel),
            ("OK", ResponseType::Accept),
        ],
    );
    dialog.set_default_width(300);

    let content = dialog.content_area();
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_spacing(8);

    let entry = Entry::new();
    entry.set_text(initial_text);
    entry.set_activates_default(true);
    content.append(&entry);

    if let Some(btn) = dialog.widget_for_response(ResponseType::Accept) {
        btn.add_css_class("suggested-action");
        dialog.set_default_widget(Some(&btn));
    }

    let entry_clone = entry.clone();
    dialog.connect_response(move |d, response| {
        let result = if response == ResponseType::Accept {
            let text = entry_clone.text().to_string();
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        } else {
            None
        };
        d.close();
        on_response(result);
    });

    dialog.present();
}
