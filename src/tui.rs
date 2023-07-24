//! All things related to the pinv TUI

// Copyright (c) 2023 Charles M. Thompson
//
// This file is part of pinv.
//
// pinv is free software: you can redistribute it and/or modify it under
// the terms only of version 3 of the GNU General Public License as published
// by the Free Software Foundation
//
// pinv is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License
// for more details.
//
// You should have received a copy of the GNU General Public License along with
// pinv(in a file named COPYING).
// If not, see <https://www.gnu.org/licenses/>.
use crate::b64;
use crate::db;
use crate::db::Catagory;
use crate::db::CatagoryField;
use crate::db::Condition;
use crate::db::ConditionOperator;
use crate::db::Db;
use crate::db::Entry;
use crate::db::EntryField;
use crate::templates;
use chrono::{Local, TimeZone};
use cursive::event::Event;
use cursive::event::Key;
use cursive::view::Nameable;
use cursive::view::Resizable;
use cursive::views::Button;
use cursive::views::Dialog;
use cursive::views::EditView;
use cursive::views::LinearLayout;
use cursive::views::NamedView;
use cursive::views::OnEventView;
use cursive::views::ScrollView;
use cursive::views::SelectView;
use cursive::views::TextView;
use cursive::views::ViewRef;
use cursive::Cursive;
use cursive::CursiveExt;
use directories::ProjectDirs;
use libflate::gzip::Decoder;
use simple_error::bail;
use std::cmp;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

// ID of the list view
static TUI_LIST_ID: &str = "list";

// Column Padding
static TUI_COLUMN_PADDING: &str = " | ";

// Column Padding Width
const TUI_COLUMN_PADDING_LEN: usize = 3;

// Field Entry Width
const TUI_FIELD_ENTRY_WIDTH: usize = 16;

// New quantity view
static TUI_NEW_QUANTITY_ID: &str = "new_quantity";

// ID of the field name edit view
static TUI_FIELD_NAME_ID: &str = "field_name";

static TUI_CATAGORY_NAME_ID: &str = "catagory_name";

// ID of the type select view
static TUI_TYPE_MENU_ID: &str = "type_menu";

static TUI_FIND_KEY_ID: &str = "find_key";

static TUI_FIELD_LIST_ID: &str = "field_list";

static TUI_OUT_FILE_ID: &str = "out_file";

static TUI_TEMPLATE_LIST_ID: &str = "template_list";

static TUI_MOD_FIELD_EDIT: &str = "mod_field_edit";

static TUI_CONSTRAINT_EDIT_ID: &str = "constraint_edit";

static TUI_FIELD_SELECT_ID: &str = "field_select";

static TUI_OP_SELECT_ID: &str = "op_select";

static TUI_VIEW_ID: &str = "view";

/// Enum used when loading templates to determin if it's a built in or a file
enum TemplateType {
    // Built-in template
    BuiltIn(String),
    // File
    File(String),
    // Not selected
    NS,
}

enum LayerType {
    View(NamedView<OnEventView<LinearLayout>>),
    Dialog(OnEventView<Dialog>),
}

/// Struct used for interfacing with the TUI. Uses the Cursive library.
pub struct Tui {
    cursive: Cursive,
}

impl Tui {
    /// Create a new TUI instance with a database.
    pub fn new(db: Db) -> Result<Self, Box<dyn Error>> {
        let mut tui = Self {
            cursive: Cursive::new(),
        };

        // Initialize all important paths
        let qualifier = "org";
        let organisation = crate::ORGANISATION;
        let application = crate::APPLICATION;

        let dirs = ProjectDirs::from(qualifier, organisation, application).unwrap();

        let mut template_dir = dirs.data_dir().to_owned();
        template_dir.push("templates");
        // Create directory if it doesn't exist
        if !template_dir.exists() {
            fs::create_dir_all(template_dir.as_path()).unwrap();
        }

        let tui_cache = TuiCache {
            db,
            template_dir,
            edited_ids: Vec::new(),
            constraints: Vec::new(),
            escape_action: Vec::new(),
            selected_catagory: String::new(),
            selected_key: 0,
        };

        tui.cursive.set_user_data(tui_cache);

        tui.prime(); // Prime all event handlers
        Ok(tui)
    }

    /// Run the TUI instance
    pub fn run(&mut self) {
        Self::push_layer(&mut self.cursive, Self::catagory_view);
        self.cursive.run_crossterm().unwrap();
    }

    /// Call to add a layer
    fn push_layer(
        cursive: &mut Cursive,
        init: fn(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>>,
    ) {
        let cache = cursive.user_data::<TuiCache>().unwrap();

        cache.escape_action.push(init);

        let layer = match init(cursive) {
            Ok(layer) => layer,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        match layer {
            LayerType::View(view) => {
                cursive.pop_layer();
                cursive.add_fullscreen_layer(view);
            }
            LayerType::Dialog(dialog) => {
                // Clear all bindings of the view
                let mut view: ViewRef<OnEventView<LinearLayout>> =
                    cursive.find_name(TUI_VIEW_ID).unwrap();

                view.clear_callbacks();

                cursive.add_layer(dialog);
            }
        }
    }

    /// Call to pop a layer
    fn pop_layer(cursive: &mut Cursive) {
        let cache = cursive.user_data::<TuiCache>().unwrap();

        if cache.escape_action.len() > 1 {
            cache.escape_action.pop();

            let escape_action = cache.escape_action.last().unwrap();

            let layer = match escape_action(cursive) {
                Ok(layer) => layer,
                Err(error) => {
                    Self::fatal_error_dialog(cursive, error);
                    return;
                }
            };

            cursive.pop_layer();

            // If it's a dialog, do nothing. If it's a view, rebuild the view
            if let LayerType::View(view) = layer {
                // We also need to pop the view
                cursive.pop_layer();
                cursive.add_fullscreen_layer(view);
            }
        } else {
            Self::push_layer(cursive, Self::exit_dialog);
        }
    }

    /// Pop all layers except the base layer
    fn base_layer(cursive: &mut Cursive) {
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let escape_action = cache.escape_action[0];

        cache.escape_action.clear();

        while let Some(_) = cursive.pop_layer() {}

        Self::push_layer(cursive, escape_action);
    }

    /// Used for binding keys and other event handlers to the TUI instance.
    fn prime(&mut self) {
        // Bind esc to do whatever is at the top of the escape action stack
        self.cursive
            .set_on_post_event(Event::Key(Key::Esc), |cursive| Self::pop_layer(cursive));
    }

    /// Bindings for all views
    fn prime_view(view: &mut OnEventView<LinearLayout>) {
        // Bind f to find mode
        view.set_on_event(Event::Char('f'), |cursive| {
            Self::push_layer(cursive, Self::find_dialog)
        });

        // Bind p to fill template mode
        view.set_on_event(Event::Char('p'), |cursive| {
            Self::push_layer(cursive, Self::fill_template_dialog)
        });
    }

    /// Bindings for catagory view
    fn prime_catagory_view(view: &mut OnEventView<LinearLayout>) {
        Self::prime_view(view);

        // Bind a to add_catagory mode
        view.set_on_event(Event::Char('a'), |cursive| {
            Self::push_layer(cursive, Self::add_catagory_dialog)
        });

        // Bind Del to the delete catagory dialog
        view.set_on_event(Event::Key(Key::Del), |cursive| {
            Self::push_layer(cursive, Self::delete_catagory_dialog)
        });
    }

    /// Bindings for entry view
    fn prime_entry_view(view: &mut OnEventView<LinearLayout>) {
        Self::prime_view(view);

        // Bind a to add_entry mode
        view.set_on_event(Event::Char('a'), |cursive| {
            Self::push_layer(cursive, Self::add_entry_dialog)
        });

        // Bind + and - to give and take mode
        view.set_on_event(Event::Char('+'), |cursive| {
            Self::push_layer(cursive, Self::give_dialog)
        });
        view.set_on_event(Event::Char('-'), |cursive| {
            Self::push_layer(cursive, Self::take_dialog)
        });

        // Bind m to modify mode
        view.set_on_event(Event::Char('m'), |cursive| {
            Self::push_layer(cursive, Self::mod_entry_dialog)
        });

        // Bind y to yank_entry mode
        view.set_on_event(Event::Char('y'), |cursive| {
            Self::push_layer(cursive, Self::yank_entry_dialog)
        });

        // Bind f to filter mode
        view.set_on_event(Event::Char('F'), |cursive| {
            Self::push_layer(cursive, Self::filter_dialog)
        });

        // Bind c to clear last constraint
        view.set_on_event(Event::Char('c'), |cursive| {
            Self::push_layer(cursive, Self::pop_constraint)
        });

        // Bind C to clear all constraints
        view.set_on_event(Event::Char('C'), |cursive| {
            Self::push_layer(cursive, Self::clear_constraints)
        });

        // Bind Del to the delete dialog
        view.set_on_event(Event::Key(Key::Del), |cursive| {
            Self::push_layer(cursive, Self::delete_entry_dialog)
        });
    }

    /// Bindings for all dialog views
    fn prime_dialog(_: &mut OnEventView<Dialog>) {
        // Currently there are no universal dialog bindings
    }

    /// Bindings for the add catagory dialog
    fn prime_add_catagory_dialog(dialog: &mut OnEventView<Dialog>) {
        Self::prime_dialog(dialog);

        dialog.set_on_event(Event::Key(Key::Del), |cursive| {
            // Grab the field list
            let mut field_list_view: ViewRef<SelectView<CatagoryField>> =
                cursive.find_name(TUI_FIELD_LIST_ID).unwrap();

            let id = match field_list_view.selected_id() {
                Some(id) => id,
                None => {
                    return;
                }
            };

            field_list_view.remove_item(id);
        })
    }

    /// Populate the list view with catagories.
    fn catagory_view(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        cursive.clear();
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let catagories = cache.db.list_catagories()?;

        let catagory_table = cache.db.stat_catagories()?;

        let headers = vec!["NAME".to_string(), "ENTRIES".to_string()];

        let columnated_catagories = Self::columnator(headers, catagory_table);

        // Ensure there are no remaining constraints as this can cause errors...
        cache.constraints.clear();

        let status_header = TextView::new("CATAGORY VIEW").center().full_width();
        let list_view_header = TextView::new(&columnated_catagories[0]).full_width();
        let list_view = SelectView::new()
            .with_all(
                catagories
                    .into_iter()
                    .enumerate()
                    .map(move |(i, catagory)| (columnated_catagories[i + 1].clone(), catagory)),
            )
            .on_submit(|cursive, catagory: &str| {
                let cache = cursive.user_data::<TuiCache>().unwrap();

                cache.selected_catagory = catagory.to_string();
                cache.selected_key = 0;
                Self::push_layer(cursive, Self::entry_view)
            })
            .with_name(TUI_LIST_ID)
            .full_width();

        let mut list_view_scroll = ScrollView::new(list_view).show_scrollbars(false);
        list_view_scroll.scroll_to_important_area();

        let list_layout = LinearLayout::vertical()
            .child(list_view_header)
            .child(list_view_scroll);

        let list_layout_scroll = ScrollView::new(list_layout).scroll_x(true).scroll_y(false);

        let layout = LinearLayout::vertical()
            .child(status_header)
            .child(list_layout_scroll);

        // Make keys bindable to this view
        let mut layout = OnEventView::new(layout);

        Self::prime_catagory_view(&mut layout);

        let layout = layout.with_name(TUI_VIEW_ID);
        // Clear all and add the layout to cursive
        cursive.pop_layer();

        Ok(LayerType::View(layout))
    }

    /// Populate the list view with entries and select an entry based off the
    /// given key
    fn entry_view(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let catagory_name = cache.selected_catagory.clone();
        let key = cache.selected_key;

        let entries = cache
            .db
            .search_catagory(&catagory_name, &cache.constraints)?;

        // Grab the catagory's field headers
        let headers = cache.db.grab_catagory_fields(&catagory_name)?;

        // Convert the entries into a table
        let mut entry_table = Vec::<Vec<String>>::with_capacity(entries.len());

        let mut entry_selected: usize = 0;

        for (i, entry) in entries.iter().enumerate() {
            let created_str = Local.timestamp_opt(entry.created, 0).unwrap().to_string();
            let modified_str = Local.timestamp_opt(entry.modified, 0).unwrap().to_string();

            // If the key is equal to the one specified, select it
            if entry.key == key {
                entry_selected = i;
            }

            let mut entry_row = Vec::<String>::with_capacity(headers.len());

            // Push the key, location, quantity, created, and modified
            entry_row.push(b64::from_u64(entry.key));
            entry_row.push(entry.location.clone());
            entry_row.push(entry.quantity.to_string());
            entry_row.push(created_str);
            entry_row.push(modified_str);

            // Push the rest of the fields
            for field in &entry.fields {
                entry_row.push(field.value.clone());
            }

            // Push the entry to the entry table
            entry_table.push(entry_row);
        }

        // Columnate the entries
        let columnated_entries = Self::columnator(headers, entry_table);

        // Set the status to inform the user that they're in entry view
        let mut status_string = format!("ENTRY VIEW (CATAGORY={})\n", catagory_name);
        // Add the constraints to the status message
        for (i, constraint) in cache.constraints.iter().enumerate() {
            if i > 0 {
                status_string.push_str(", ");
            }
            status_string.push_str(&constraint.to_string());
        }

        let status_header = TextView::new(status_string).center().full_width();
        let list_view_header = TextView::new(&columnated_entries[0]).full_width();
        let list_view = SelectView::new()
            .with_all(
                entries
                    .into_iter()
                    .enumerate()
                    .map(move |(i, entry)| (columnated_entries[i + 1].clone(), entry)),
            )
            .selected(entry_selected)
            .with_name(TUI_LIST_ID)
            .full_width();

        let mut list_view_scroll = ScrollView::new(list_view).show_scrollbars(false);
        list_view_scroll.scroll_to_important_area();

        let list_layout = LinearLayout::vertical()
            .child(list_view_header)
            .child(list_view_scroll);

        let list_layout_scroll = ScrollView::new(list_layout).scroll_x(true).scroll_y(false);

        let layout = LinearLayout::vertical()
            .child(status_header)
            .child(list_layout_scroll);

        // Make keys bindable to this view
        let mut layout = OnEventView::new(layout);
        Self::prime_entry_view(&mut layout);
        let layout = layout.with_name(TUI_VIEW_ID);

        Ok(LayerType::View(layout))
    }

    /// Dialog used to find an entry given only a key
    fn find_dialog(_: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let find_view = TextView::new("Key: ");
        let find_edit = EditView::new()
            .on_submit(|cursive, _| Self::find_dialog_submit(cursive))
            .with_name(TUI_FIND_KEY_ID)
            .fixed_width(TUI_FIELD_ENTRY_WIDTH);

        let find_row = LinearLayout::horizontal().child(find_view).child(find_edit);

        let dialog = Dialog::around(find_row)
            .button("Find", |cursive| Self::find_dialog_submit(cursive))
            .title("Find Entry");

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Function called when the find button is selected in the find dialog
    fn find_dialog_submit(cursive: &mut Cursive) {
        let find_edit: ViewRef<EditView> = cursive.find_name(TUI_FIND_KEY_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let key_str = find_edit.get_content();
        let key = match b64::to_u64(&key_str) {
            Ok(key) => key,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        // We don't need to find the exact entry at the moment, we just need to
        // find the catagory so we know which catagory to display the contents
        // of
        let catagory_name = match cache.db.grab_catagory_from_key(key) {
            Ok(catagory_name) => catagory_name,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        drop(cache);
        Self::base_layer(cursive);

        let cache = cursive.user_data::<TuiCache>().unwrap();
        cache.selected_key = key;
        cache.selected_catagory = catagory_name;

        Self::push_layer(cursive, Self::entry_view);
    }

    /// Dialog used to add a catagory.
    fn add_catagory_dialog(_: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let name_view = TextView::new("Name: ");
        let name_edit = EditView::new()
            .with_name(TUI_CATAGORY_NAME_ID)
            .fixed_width(TUI_FIELD_ENTRY_WIDTH);

        let name_row = LinearLayout::horizontal().child(name_view).child(name_edit);

        let add_field_button = Button::new("Add Field", |cursive| {
            Self::push_layer(cursive, Self::add_catagory_field_dialog)
        });

        let field_list = SelectView::<CatagoryField>::new().with_name(TUI_FIELD_LIST_ID);

        let layout = LinearLayout::vertical()
            .child(name_row)
            .child(add_field_button)
            .child(field_list);

        let dialog = Dialog::around(layout)
            .title("Add Catagory")
            .button("Add Catagory", |cursive| {
                Self::add_catagory_dialog_submit(cursive)
            });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_add_catagory_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Function called when the submit button is pressed in the add catagory
    /// dialog.
    fn add_catagory_dialog_submit(cursive: &mut Cursive) {
        // Grab the views we need
        let catagory_name_view: ViewRef<EditView> =
            cursive.find_name(TUI_CATAGORY_NAME_ID).unwrap();
        let field_list_view: ViewRef<SelectView<CatagoryField>> =
            cursive.find_name(TUI_FIELD_LIST_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let catagory_name = catagory_name_view.get_content();

        let fields = field_list_view
            .iter()
            .map(|row| {
                let (_, field) = row;

                field.clone()
            })
            .collect();

        let catagory = Catagory::with_fields(&catagory_name, fields);

        match cache.db.add_catagory(catagory) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        Self::pop_layer(cursive);
    }

    /// Dialog used to add a field to a catagory.
    fn add_catagory_field_dialog(_: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let name_view = TextView::new("Name: ");
        let name_edit = EditView::new()
            .with_name(TUI_FIELD_NAME_ID)
            .fixed_width(TUI_FIELD_ENTRY_WIDTH);
        let name_row = LinearLayout::horizontal().child(name_view).child(name_edit);

        let type_view = TextView::new("Type: ");
        let type_menu = SelectView::<db::DataType>::new()
            .popup()
            .item("INTEGER", db::DataType::INTEGER)
            .item("REAL", db::DataType::REAL)
            .item("TEXT", db::DataType::TEXT);
        let type_row = LinearLayout::horizontal()
            .child(type_view)
            .child(type_menu.with_name(TUI_TYPE_MENU_ID));

        let layout = LinearLayout::vertical().child(name_row).child(type_row);

        let dialog = Dialog::around(layout).button("Add Field", |cursive| {
            Self::add_catagory_field_submit(cursive)
        });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Function called when the submit button is pressed in the add catagory
    /// field dialog.
    fn add_catagory_field_submit(cursive: &mut Cursive) {
        // Grab the views we need
        let type_menu_view: ViewRef<SelectView<db::DataType>> =
            cursive.find_name(TUI_TYPE_MENU_ID).unwrap();
        let mut field_list_view: ViewRef<SelectView<CatagoryField>> =
            cursive.find_name(TUI_FIELD_LIST_ID).unwrap();
        let field_name_view: ViewRef<EditView> = cursive.find_name(TUI_FIELD_NAME_ID).unwrap();

        let field = CatagoryField::new(
            &field_name_view.get_content().to_uppercase(),
            *type_menu_view.selection().unwrap(),
        );

        field_list_view.add_item(field.to_string(), field);

        Self::pop_layer(cursive);
    }

    /// Dialog used to add an entry to the database.
    fn add_entry_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let mut layout = LinearLayout::vertical();

        let fields = cache.db.grab_catagory_fields(&cache.selected_catagory)?;

        // Remove created and modified because they are autogenerated
        let fields_a: Vec<String> = fields[..3].into();
        let fields_b: Vec<String> = fields[5..].into();
        let fields = [fields_a, fields_b].concat();

        // First find the largest field name
        let mut max_size: usize = 0;

        for field in &fields {
            max_size = cmp::max(max_size, field.len())
        }

        for (i, field) in fields.iter().enumerate() {
            let field_id_str = format!("{}:", field);
            let field_id = TextView::new(format!(
                "{:<width$}",
                field_id_str.clone(),
                width = max_size + 2
            ));

            let mut field_entry = EditView::new().on_edit(move |cursive, _, _| {
                let cache = cursive.user_data::<TuiCache>().unwrap();

                // If the id hasn't been edited it, add it to the list of edited ids
                if !cache.edited_ids.contains(&i)
                {
                    cache.edited_ids.push(i);
                }
            });

            if field_id_str == "KEY:" {
                field_entry.set_content(b64::from_u64(cache.db.grab_next_available_key(0)?));
                
                // Since we are pre-adding the key, the key has technically ben pre-edited.
                cache.edited_ids.push(i);
            }

            let field_entry = field_entry
                .with_name(format!("{}{}", TUI_MOD_FIELD_EDIT, i))
                .fixed_width(TUI_FIELD_ENTRY_WIDTH);

            let row = LinearLayout::horizontal()
                .child(field_id)
                .child(field_entry);

            layout.add_child(row);
        }

        cache.edited_ids.clear();

        let dialog = Dialog::around(layout)
            .title(format!("Add entry to {}...", cache.selected_catagory))
            .button("Add", |cursive| Self::add_entry_submit(cursive));

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Function called when the submit button is pressed in the add entry
    /// dialog.
    fn add_entry_submit(cursive: &mut Cursive) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let edited_ids = cache.edited_ids.clone();

        let field_ids = match cache.db.grab_catagory_fields(&cache.selected_catagory) {
            Ok(ids) => ids,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        // Remove created and modified because they are autogenerated
        let fields_a: Vec<String> = field_ids[..3].into();
        let fields_b: Vec<String> = field_ids[5..].into();
        let field_ids = [fields_a, fields_b].concat();

        let catagory = cache.selected_catagory.clone();

        // Drop the cache so we can get the edit views we need...
        drop(cache);

        let mut fields: Vec<EntryField> = Vec::with_capacity(edited_ids.len());
        for id in edited_ids {
            let edit_view: ViewRef<EditView> = cursive
                .find_name(&format!("{}{}", TUI_MOD_FIELD_EDIT, id))
                .unwrap();

            let field_id = &field_ids[id];
            let field_value = edit_view.get_content();

            let field = EntryField::new(field_id, &field_value);

            fields.push(field);
        }

        // Create the entry from the aquired fields
        // This is ugly
        let key = match b64::to_u64(
            &fields
                .iter()
                .find(|field| field.id == "KEY")
                .unwrap_or(&EntryField::new("", ""))
                .value,
        ) {
            Ok(key) => key,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        let location = fields
            .iter()
            .find(|field| field.id == "LOCATION")
            .unwrap_or(&EntryField::new("", ""))
            .value
            .clone();

        let quantity = match fields
            .iter()
            .find(|field| field.id == "QUANTITY")
            .unwrap_or(&EntryField::new("", ""))
            .value
            .parse::<u64>()
        {
            Ok(quantity) => quantity,
            Err(error) => {
                Self::error_dialog(cursive, Box::new(error));
                return;
            }
        };

        let created = Local::now().timestamp();
        let modified = created;

        let mut entry = Entry::new(&catagory, key, &location, quantity, created, modified);

        // Prime jank
        entry.add_fields(
            &fields
                .into_iter()
                .filter(|field| {
                    field.id != "KEY" && field.id != "LOCATION" && field.id != "QUANTITY"
                })
                .collect::<Vec<EntryField>>(),
        );

        // Get the cache again
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Set the selected key
        cache.selected_key = entry.key;
        match cache.db.add_entry(entry) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        }

        Self::pop_layer(cursive);
    }

    /// Dialog used to modify entries
    fn mod_entry_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let list_view: ViewRef<SelectView<Entry>> = cursive.find_name(TUI_LIST_ID).unwrap();
        // Grab the cache
        let mut cache = cursive.user_data::<TuiCache>().unwrap();

        // Get the entry
        let entry = match list_view.selection() {
            Some(entry) => entry,
            None => {
                bail!("No entry to operate on!");
            }
        };

        // Set the selected key
        cache.selected_key = entry.key;
        // Build fields based on what the entry has
        let key = EntryField::new("KEY", &b64::from_u64(entry.key));
        let location = EntryField::new("LOCATION", &format!("{}", entry.location));
        let quantity = EntryField::new("QUANTITY", &entry.quantity.to_string());
        let mut fields: Vec<EntryField> = vec![key, location, quantity];

        fields.extend_from_slice(&entry.fields);

        // Generate rows in the dialog to reflect the fields to be modified
        let mut layout = LinearLayout::vertical();
        // First find the largest field name(for padding reasons)
        let mut max_size: usize = 0;

        for field in &fields {
            max_size = cmp::max(max_size, field.id.len())
        }

        for (i, field) in fields.iter().enumerate() {
            let field_id = format!("{}:", field.id);
            let field_id = TextView::new(format!("{:<width$}", field_id, width = max_size + 2));

            let field_value = field.value.clone();

            let field_entry = EditView::new()
                .content(field_value)
                .on_edit(move |cursive, _, _| {
                    let cache = cursive.user_data::<TuiCache>().unwrap();

                    cache.edited_ids.push(i);
                })
                .with_name(format!("{}{}", TUI_MOD_FIELD_EDIT, i))
                .fixed_width(TUI_FIELD_ENTRY_WIDTH);

            let row = LinearLayout::horizontal()
                .child(field_id)
                .child(field_entry);

            layout.add_child(row);
        }

        cache.edited_ids.clear();

        let dialog = Dialog::around(layout)
            .button("Modify!", |cursive| Self::mod_entry_dialog_submit(cursive));

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Called when the modify button is selected
    fn mod_entry_dialog_submit(cursive: &mut Cursive) {
        let list_view: ViewRef<SelectView<Entry>> = cursive.find_name(TUI_LIST_ID).unwrap();
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let entry = match list_view.selection() {
            Some(entry) => entry,
            None => {
                return;
            }
        };

        let edited_ids = cache.edited_ids.clone();

        // Get all of the field ids(minus creation and mod time)
        let mut field_ids: Vec<String> = vec!["KEY".into(), "LOCATION".into(), "QUANTITY".into()];

        for field in &entry.fields {
            field_ids.push(field.id.clone());
        }

        // Drop the cache so we can get the edit views we need...
        drop(cache);

        let mut fields: Vec<EntryField> = Vec::with_capacity(edited_ids.len());
        for id in edited_ids {
            let edit_view: ViewRef<EditView> = cursive
                .find_name(&format!("{}{}", TUI_MOD_FIELD_EDIT, id))
                .unwrap();

            let field_id = &field_ids[id];
            let field_value = edit_view.get_content();

            let field = EntryField::new(field_id, &field_value);

            fields.push(field);
        }

        // Get the cache again
        let cache = cursive.user_data::<TuiCache>().unwrap();

        match cache.db.mod_entry(entry.key, fields) {
            Ok(types) => types,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        Self::pop_layer(cursive);
    }

    /// Dialog used to yank an entry
    fn yank_entry_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let list_view: ViewRef<SelectView<Entry>> = cursive.find_name(TUI_LIST_ID).unwrap();
        // Grab the cache
        let mut cache = cursive.user_data::<TuiCache>().unwrap();

        // Get the entry to give or take from
        let entry = match list_view.selection() {
            Some(entry) => entry,
            None => {
                bail!("No entry to operate on!");
            }
        };

        // Set the selected key
        cache.selected_key = entry.key;
        // Build fields based on what the entry has
        // require only a new key be specified
        let key = EntryField::new("KEY", "");
        let location = EntryField::new("LOCATION", &format!("{}", entry.location));
        let quantity = EntryField::new("QUANTITY", &entry.quantity.to_string());
        let mut fields: Vec<EntryField> = vec![key, location, quantity];

        fields.extend_from_slice(&entry.fields);

        // Generate rows in the dialog to reflect the fields to be modified
        let mut layout = LinearLayout::vertical();
        // First find the largest field name(for padding reasons)
        let mut max_size: usize = 0;

        for field in &fields {
            max_size = cmp::max(max_size, field.id.len())
        }

        for (i, field) in fields.iter().enumerate() {
            let field_id = format!("{}:", field.id);
            let field_id = TextView::new(format!("{:<width$}", field_id, width = max_size + 2));

            let field_value = field.value.clone();

            let field_entry = EditView::new()
                .content(field_value)
                .on_edit(move |cursive, _, _| {
                    let cache = cursive.user_data::<TuiCache>().unwrap();

                    cache.edited_ids.push(i);
                })
                .with_name(format!("{}{}", TUI_MOD_FIELD_EDIT, i))
                .fixed_width(TUI_FIELD_ENTRY_WIDTH);

            let row = LinearLayout::horizontal()
                .child(field_id)
                .child(field_entry);

            layout.add_child(row);
        }

        cache.edited_ids.clear();

        let dialog = Dialog::around(layout).button("Yank & Add!", |cursive| {
            Self::yank_entry_dialog_submit(cursive)
        });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Called when the "Yank & Add!" button is selected
    fn yank_entry_dialog_submit(cursive: &mut Cursive) {
        let list_view: ViewRef<SelectView<Entry>> = cursive.find_name(TUI_LIST_ID).unwrap();
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let original_entry = match list_view.selection() {
            Some(entry) => entry,
            None => {
                return;
            }
        };

        let catagory = cache.selected_catagory.clone();

        let edited_ids = cache.edited_ids.clone();

        // Get all of the field ids(minus creation and mod time)
        let mut field_ids: Vec<String> = vec!["KEY".into(), "LOCATION".into(), "QUANTITY".into()];

        for field in &original_entry.fields {
            field_ids.push(field.id.clone());
        }

        // Drop the cache so we can get the edit views we need...
        drop(cache);

        let mut fields: Vec<EntryField> = Vec::with_capacity(edited_ids.len());
        for id in edited_ids {
            let edit_view: ViewRef<EditView> = cursive
                .find_name(&format!("{}{}", TUI_MOD_FIELD_EDIT, id))
                .unwrap();

            let field_id = &field_ids[id];
            let field_value = edit_view.get_content();

            let field = EntryField::new(field_id, &field_value);

            fields.push(field);
        }

        // Create the entry from the aquired fields
        // This is ugly
        let key = match b64::to_u64(
            &fields
                .iter()
                .find(|field| field.id == "KEY")
                .unwrap_or(&EntryField::new("", ""))
                .value,
        ) {
            Ok(key) => key,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        let location = fields
            .iter()
            .find(|field| field.id == "LOCATION")
            .unwrap_or(&EntryField::new("", &original_entry.location))
            .value
            .clone();

        let quantity = match fields
            .iter()
            .find(|field| field.id == "QUANTITY")
            .unwrap_or(&EntryField::new("", &original_entry.quantity.to_string()))
            .value
            .parse::<u64>()
        {
            Ok(quantity) => quantity,
            Err(error) => {
                Self::error_dialog(cursive, Box::new(error));
                return;
            }
        };

        let created = Local::now().timestamp();
        let modified = created;

        let mut entry = Entry::new(&catagory, key, &location, quantity, created, modified);

        let fields_copy = fields.clone();

        // Prime jank
        entry.add_fields(
            &fields
                .into_iter()
                .filter(|field| {
                    field.id != "KEY" && field.id != "LOCATION" && field.id != "QUANTITY"
                })
                .collect::<Vec<EntryField>>(),
        );

        // Add the original entry fields that weren't edited
        entry.add_fields(
            &original_entry
                .fields
                .clone()
                .into_iter()
                .filter(move |field| !match fields_copy
                    .iter()
                    .find(move |new_field| &field == new_field)
                {
                    Some(_) => true,
                    None => false,
                })
                .collect::<Vec<EntryField>>(),
        );

        // Get the cache again
        let cache = cursive.user_data::<TuiCache>().unwrap();

        match cache.db.add_entry(entry) {
            Ok(types) => types,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        Self::pop_layer(cursive);
    }

    /// Dialog used to add filter constraints
    fn filter_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let fields = cache.db.grab_catagory_fields(&cache.selected_catagory)?;

        // Remove created and modified because they are autogenerated
        let fields_a: Vec<String> = fields[..3].into();
        let fields_b: Vec<String> = fields[5..].into();
        let fields = [fields_a, fields_b].concat();

        // Fields the user can select from
        let mut field_select_list = SelectView::new().popup();

        field_select_list.add_all_str(fields);

        let field_select_list = field_select_list.with_name(TUI_FIELD_SELECT_ID);

        // The operators the user can use
        let mut operator_select_list = SelectView::<ConditionOperator>::new().popup();

        operator_select_list.add_all(
            vec![
                ConditionOperator::Equal,
                ConditionOperator::NotEqual,
                ConditionOperator::LessThan,
                ConditionOperator::GreaterThan,
                ConditionOperator::LessThanEqual,
                ConditionOperator::GreaterThanEqual,
            ]
            .into_iter()
            .map(|x| (format!("{}", x), x)),
        );

        let operator_select_list = operator_select_list.with_name(TUI_OP_SELECT_ID);

        // The value to compare fields to...
        let constraint_edit_view = EditView::new()
            .with_name(TUI_CONSTRAINT_EDIT_ID)
            .fixed_width(TUI_FIELD_ENTRY_WIDTH);

        // Lay it all out horizontally
        let layout = LinearLayout::horizontal()
            .child(field_select_list)
            .child(operator_select_list)
            .child(constraint_edit_view);

        let dialog =
            Dialog::around(layout).button("Filter!", |cursive| Self::filter_dialog_submit(cursive));

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Called when the "Filter!" button is selected
    fn filter_dialog_submit(cursive: &mut Cursive) {
        // Grab the needed views
        let field_select_list: ViewRef<SelectView> =
            cursive.find_name(TUI_FIELD_SELECT_ID).unwrap();
        let operator_select_list: ViewRef<SelectView<ConditionOperator>> =
            cursive.find_name(TUI_OP_SELECT_ID).unwrap();
        let constraint_edit_view: ViewRef<EditView> =
            cursive.find_name(TUI_CONSTRAINT_EDIT_ID).unwrap();

        let cache = cursive.user_data::<TuiCache>().unwrap();

        let field_id = field_select_list.selection().unwrap();
        let operator = operator_select_list.selection().unwrap();
        // Format the constraint value according to it's type
        let constraint_value = constraint_edit_view.get_content();

        let constraint = Condition::new(&field_id, *operator, &constraint_value);

        cache.constraints.push(constraint);

        Self::pop_layer(cursive);
    }

    /// Remove last applied constraint
    fn pop_constraint(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Return if no constraints are found
        if cache.constraints.len() == 0 {
            bail!("No constraints to remove!");
        }

        // Ask the user if they want to remove the constraint

        // Create the dialog
        // We are sure that there are constraints in the constraint vec so it's safe to put an
        // unwrap here...
        let dialog = Dialog::text(format!(
            "Remove constraint {}?",
            cache.constraints.last().unwrap()
        ))
        .button("No...", |cursive| Self::pop_layer(cursive))
        .button("Yes!", move |cursive| {
            let cache = cursive.user_data::<TuiCache>().unwrap();

            cache.constraints.pop();

            Self::pop_layer(cursive);
        });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Remove all constraints
    fn clear_constraints(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Return if no constraints are found
        if cache.constraints.len() == 0 {
            bail!("No constraints to remove!");
        }

        // Ask the user if they want to remove the constraint

        // Create the dialog
        // We are sure that there are constraints in the constraint vec so it's safe to put an
        // unwrap here...
        let dialog = Dialog::text("Remove all constraints?")
            .button("No...", |cursive| {
                Self::pop_layer(cursive);
            })
            .button("Yes!", move |cursive| {
                let cache = cursive.user_data::<TuiCache>().unwrap();

                cache.constraints.clear();

                Self::pop_layer(cursive);
            });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Dialog used to give to an entry
    fn give_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        Self::give_take_dialog(cursive, true)
    }

    /// Dialog used to take from an entry
    fn take_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        Self::give_take_dialog(cursive, false)
    }

    /// Dialog used when either giving or taking from an entry. If give is
    /// true, we are giving to an entry. If false, we are taking from an entry.
    fn give_take_dialog(cursive: &mut Cursive, give: bool) -> Result<LayerType, Box<dyn Error>> {
        let list_view: ViewRef<SelectView<Entry>> = cursive.find_name(TUI_LIST_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Get the entry to give or take from
        let entry = match list_view.selection() {
            Some(entry) => entry,
            None => {
                bail!("No entry to operate on!");
            }
        };

        // Set the selected key
        cache.selected_key = entry.key;

        // Get the quantity
        let quantity = entry.quantity;

        let give_or_take = match give {
            true => "Give",
            false => "Take",
        };

        let to_or_from = match give {
            true => "to",
            false => "from",
        };

        let old_quantity_view = TextView::new(format!("Old Quantity: {}", quantity));

        // Create the entry row
        let quantity_entry_view = TextView::new(format!("{}: ", give_or_take));

        let give_take_edit = EditView::new()
            .content("1")
            .on_edit(move |cursive, string, _| {
                Self::give_take_dialog_update(cursive, string, give);
            })
            .on_submit(move |cursive, _| Self::give_take_dialog_submit(cursive, give))
            .with_name(TUI_MOD_FIELD_EDIT)
            .fixed_width(TUI_FIELD_ENTRY_WIDTH);

        let entry_row = LinearLayout::horizontal()
            .child(quantity_entry_view)
            .child(give_take_edit);

        // Create the updating "New Quantity" View
        let new_quantity = match give {
            true => quantity + 1,
            false => quantity - 1,
        };

        let new_quantity_view =
            TextView::new(format!("New Quantity: {}", new_quantity)).with_name(TUI_NEW_QUANTITY_ID);

        // Lay it all out together vertically
        let layout = LinearLayout::vertical()
            .child(old_quantity_view)
            .child(entry_row)
            .child(new_quantity_view);

        let dialog = Dialog::around(layout)
            .title(format!(
                "{} {} {}",
                give_or_take,
                to_or_from,
                b64::from_u64(entry.key)
            ))
            .button(give_or_take, move |cursive| {
                Self::give_take_dialog_submit(cursive, give)
            });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Update the dialog to reflect the new quantity
    fn give_take_dialog_update(cursive: &mut Cursive, give_take_amt: &str, give: bool) {
        let mut new_quantity_view: ViewRef<TextView> =
            cursive.find_name(TUI_NEW_QUANTITY_ID).unwrap();
        let list_view: ViewRef<SelectView<Entry>> = cursive.find_name(TUI_LIST_ID).unwrap();

        let give_take_amt: u64 = match give_take_amt.parse() {
            Ok(number) => number,
            Err(_) => {
                return;
            }
        };

        // Get the entry to give or take from
        let entry = match list_view.selection() {
            Some(entry) => entry,
            None => {
                return;
            }
        };

        let quantity: u64 = match give {
            true => entry.quantity + give_take_amt,

            false => {
                if entry.quantity > give_take_amt {
                    entry.quantity - give_take_amt
                } else {
                    0
                }
            }
        };

        new_quantity_view.set_content(format!("New Quantity: {}", quantity));
    }

    /// Function called when the submit button on the give or take dialog is
    /// pressed.
    fn give_take_dialog_submit(cursive: &mut Cursive, give: bool) {
        let list_view: ViewRef<SelectView<Entry>> = cursive.find_name(TUI_LIST_ID).unwrap();
        let new_quantity_edit: ViewRef<EditView> = cursive.find_name(TUI_MOD_FIELD_EDIT).unwrap();
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let give_take_amt: u64 = match new_quantity_edit.get_content().parse() {
            Ok(number) => number,
            Err(error) => {
                Self::error_dialog(cursive, Box::new(error));
                return;
            }
        };

        // Get the entry to give or take from
        let entry = match list_view.selection() {
            Some(entry) => entry,
            None => {
                return;
            }
        };

        let quantity: u64 = match give {
            true => entry.quantity + give_take_amt,

            false => {
                if entry.quantity > give_take_amt {
                    entry.quantity - give_take_amt
                } else {
                    0
                }
            }
        };

        match cache.db.mod_entry(
            entry.key,
            vec![EntryField::new("QUANTITY", &quantity.to_string())],
        ) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        }

        Self::pop_layer(cursive);
    }

    /// Dialog that confirms if you wish to delete an entry, and if so, deletes
    /// the entry.
    fn delete_entry_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let list_view: ViewRef<SelectView<Entry>> = cursive.find_name(TUI_LIST_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Get the entry to give or take from
        let entry = match list_view.selection() {
            Some(entry) => entry,
            None => {
                bail!("No entry to operate on!");
            }
        };

        // Set the selected key
        cache.selected_key = entry.key;

        // Create the dialog
        let dialog = Dialog::text(format!("Delete entry {}?", b64::from_u64(entry.key)))
            .button("No...", |cursive| Self::pop_layer(cursive))
            .button("Yes!", move |cursive| {
                Self::delete_entry_dialog_submit(cursive, entry.key);
            });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Deletes the entry if "Yes" is selected on the delete dialog.
    fn delete_entry_dialog_submit(cursive: &mut Cursive, key: u64) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        match cache.db.delete_entry(key) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        }

        Self::pop_layer(cursive);
    }

    /// Dialog that confirms if you wish to delete a catagory, and if so, deletes
    /// the catagory.
    fn delete_catagory_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let list_view: ViewRef<SelectView> = cursive.find_name(TUI_LIST_ID).unwrap();

        // Get the entry to give or take from
        let catagory = match list_view.selection() {
            Some(catagory) => catagory,
            None => {
                bail!("No catagory to operate on!");
            }
        };

        // Create the dialog
        let dialog = Dialog::text(format!("Delete catagory {}?", catagory))
            .button("No...", |cursive| Self::pop_layer(cursive))
            .button("Yes!", move |cursive| {
                Self::delete_catagory_dialog_submit(cursive, &catagory);
            });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Deletes the entry if "Yes" is selected on the delete dialog.
    fn delete_catagory_dialog_submit(cursive: &mut Cursive, name: &str) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        match cache.db.delete_empty_catagory(name) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        }

        Self::pop_layer(cursive);
    }

    /// Dialog used to confirm that a used wishes to exit the program.
    fn exit_dialog(_: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        let exit_dialog = Dialog::text("Are You Sure You Want To Exit?")
            .button("No...", |cursive| Self::pop_layer(cursive))
            .button("Yes!", |cursive| cursive.quit());

        Ok(LayerType::Dialog(OnEventView::new(exit_dialog)))
    }

    /// Dialog used to select a label template file to fill out
    fn fill_template_dialog(cursive: &mut Cursive) -> Result<LayerType, Box<dyn Error>> {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let template_list_header = TextView::new("Select Template File:");
        let mut template_list = SelectView::<TemplateType>::new().popup();

        template_list.add_item("<Select Template>", TemplateType::NS);

        // List the built in templates
        for template in &templates::TEMPLATES {
            let template_id = template.id.to_string();

            template_list.add_item(template_id.clone(), TemplateType::BuiltIn(template_id));
        }
        // List the template files
        let template_paths = fs::read_dir(cache.template_dir.as_path())?;

        for entry in template_paths {
            let path = entry?.path();

            if !path.is_dir() {
                let template_name = path.file_name().unwrap().to_str().unwrap().to_string();

                template_list.add_item(
                    template_name,
                    TemplateType::File(path.to_str().unwrap().to_string()),
                );
            }
        }

        let template_list = template_list.with_name(TUI_TEMPLATE_LIST_ID);

        let out_file_view = TextView::new("Out File: ");
        let out_file_edit = EditView::new()
            .with_name(TUI_OUT_FILE_ID)
            .fixed_width(TUI_FIELD_ENTRY_WIDTH);
        let out_file_row = LinearLayout::horizontal()
            .child(out_file_view)
            .child(out_file_edit);

        let layout = LinearLayout::vertical()
            .child(template_list_header)
            .child(template_list)
            .child(out_file_row);

        let dialog = Dialog::around(layout)
            .title("Fill Out Printable SVG Template")
            .button("Fill!", |cursive| {
                Self::fill_template_dialog_submit(cursive)
            });

        // Prime the default dialog bindings
        let mut dialog = OnEventView::new(dialog);
        Self::prime_dialog(&mut dialog);

        Ok(LayerType::Dialog(dialog))
    }

    /// Fills the template if the "Fill!" button is selected
    fn fill_template_dialog_submit(cursive: &mut Cursive) {
        // Grab the needed views
        let template_list: ViewRef<SelectView<TemplateType>> =
            cursive.find_name(TUI_TEMPLATE_LIST_ID).unwrap();
        let out_file_edit: ViewRef<EditView> = cursive.find_name(TUI_OUT_FILE_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let selection = template_list.selection().unwrap();

        let in_data = match selection.as_ref() {
            TemplateType::BuiltIn(template_id) => templates::TEMPLATES
                .iter()
                .find(|template| template.id == template_id)
                .expect("Template not found!")
                .get_data(),
            TemplateType::File(filename) => {
                let filedata = match fs::read(filename) {
                    Ok(data) => data,
                    Err(error) => {
                        Self::error_dialog(cursive, Box::new(error));
                        return;
                    }
                };

                let mut decoder = match Decoder::new(&filedata[..]) {
                    Ok(decoder) => decoder,
                    Err(error) => {
                        Self::error_dialog(cursive, Box::new(error));
                        return;
                    }
                };

                let mut data: Vec<u8> = Vec::new();

                match decoder.read_to_end(&mut data) {
                    Ok(_) => {}
                    Err(error) => {
                        Self::error_dialog(cursive, Box::new(error));
                        return;
                    }
                };

                data
            }
            TemplateType::NS => {
                Self::info_dialog(cursive, "You need to select a template!");
                return;
            }
        };

        let out_path = out_file_edit.get_content();

        let in_string = String::from_utf8_lossy(&in_data);

        let out_data = match cache.db.fill_svg_template(&in_string) {
            Ok(out_data) => out_data,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        match fs::write(out_path.as_ref(), out_data) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, Box::new(error));
                return;
            }
        };

        Self::pop_layer(cursive);
    }

    /// Converts a table into strings that mimic an excel table, or something
    /// alike that.
    fn columnator(headers: Vec<String>, table: Vec<Vec<String>>) -> Vec<String> {
        // First calculate the widths of each column
        let mut column_widths = Vec::<usize>::with_capacity(headers.len());
        let mut out_string_size: usize = 0;

        for (i, header) in headers.iter().enumerate() {
            let mut width = header.len();

            for row in &table {
                width = cmp::max(width, row[i].len());
            }

            column_widths.push(width);
            out_string_size += width + TUI_COLUMN_PADDING_LEN;
        }

        // Next generate strings of each row with padding to make each column the same width
        // starting with the headers
        let mut out_strings = Vec::<String>::with_capacity(table.len() + 1);

        let mut out_string = String::with_capacity(out_string_size);

        for (i, header) in headers.iter().enumerate() {
            out_string.push_str(&format!(
                "{:<width$}{}",
                header,
                TUI_COLUMN_PADDING,
                width = column_widths[i]
            ));
        }

        out_strings.push(out_string);

        for row in table {
            let mut out_string = String::with_capacity(out_string_size);

            for (i, column) in row.iter().enumerate() {
                out_string.push_str(&format!(
                    "{:<width$}{}",
                    column,
                    TUI_COLUMN_PADDING,
                    width = column_widths[i]
                ));
            }

            out_strings.push(out_string);
        }

        out_strings
    }

    /// Dialog presenting a non-fatal error
    fn info_dialog(cursive: &mut Cursive, string: &str) {
        let dialog = Dialog::info(string).title("Info:");

        cursive.add_layer(dialog)
    }
    /// Dialog presenting a non-fatal error
    fn error_dialog(cursive: &mut Cursive, error: Box<dyn Error>) {
        let dialog = Dialog::info(format!("{}", error)).title("Error!");

        cursive.add_layer(dialog)
    }

    /// Dialog presenting a fatal error, and closes cursive when exited
    fn fatal_error_dialog(cursive: &mut Cursive, error: Box<dyn Error>) {
        let dialog = Dialog::text(format!("{}", error))
            .button("Ok", |cursive| cursive.quit())
            .title("Fatal Error!");

        cursive.add_layer(dialog)
    }
}

/// Data cache during the TUI session
struct TuiCache {
    /// The directory for templates
    pub template_dir: PathBuf,
    /// Database in use
    pub db: Db,
    /// IDs of the fields edited
    pub edited_ids: Vec<usize>,
    /// Constraints that affect what is displated in entry view
    pub constraints: Vec<Condition>,
    /// Binding to call when popping out of a dialog
    pub escape_action: Vec<fn(&mut Cursive) -> Result<LayerType, Box<dyn Error>>>,
    pub selected_catagory: String,
    pub selected_key: u64,
}
