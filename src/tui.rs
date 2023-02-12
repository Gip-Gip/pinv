//! All things related to the pinv TUI

use crate::b64;
use crate::db;
use crate::db::Catagory;
use crate::db::CatagoryField;
use crate::db::Db;
use crate::db::Entry;
use crate::db::EntryField;
use chrono::{Local, TimeZone};
use cursive::align::HAlign;
use cursive::align::VAlign;
use cursive::event::Event;
use cursive::event::Key;
use cursive::view::Nameable;
use cursive::view::Resizable;
use cursive::view::Selector;
use cursive::views::Button;
use cursive::views::Dialog;
use cursive::views::EditView;
use cursive::views::LinearLayout;
use cursive::views::NamedView;
use cursive::views::ScrollView;
use cursive::views::SelectView;
use cursive::views::TextView;
use cursive::views::ViewRef;
use cursive::Cursive;
use cursive::CursiveExt;
use cursive::View;
use std::cmp;
use std::error::Error;

// ID of the list view
static TUI_LIST_ID: &str = "list";

// ID of the list header
static TUI_LIST_HEADER_ID: &str = "list_header";

// ID of the status header
static TUI_STATUS_HEADER_ID: &str = "status_header";

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

static TUI_LIST_SCROLL_ID: &str = "list_scroll";

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

        let tui_cache = TuiCache {
            catagory_selected: String::new(),
            catagories_queried: vec![],
            dialog_layers: 0,
            db,
            fields_edited: vec![String::new()],
            entries_queried: Vec::new(),
            entry_selected: 0,
        };

        tui.cursive.set_user_data(tui_cache);

        tui.prime(); // Prime all event handlers
        tui.layout(); // Lay out all the views

        Ok(tui)
    }

    /// Run the TUI instance
    pub fn run(&mut self) {
        Self::populate_with_catagories(&mut self.cursive);
        self.cursive.run_crossterm().unwrap();
    }

    /// Used for binding keys and other event handlers to the TUI instance.
    fn prime(&mut self) {
        // Bind escape to a special function which will either exit entry view or exit the program,
        // depending on what view we're in. Make it a post binding since we only want it to trigger
        // when in either catagory or entry view, not in creation dialogs or etc.
        self.cursive
            .set_on_post_event(Event::Key(Key::Esc), |cursive| Self::escape(cursive));

        // Bind a to add mode
        self.cursive
            .set_on_post_event(Event::Char('a'), |cursive| Self::add_dialog(cursive));

        // Bind f to find mode
        self.cursive
            .set_on_post_event(Event::Char('f'), |cursive| Self::find_dialog(cursive));

        // Bind + and - to give and take mode
        self.cursive.set_on_post_event(Event::Char('+'), |cursive| {
            Self::give_take_dialog(cursive, true)
        });
        self.cursive.set_on_post_event(Event::Char('-'), |cursive| {
            Self::give_take_dialog(cursive, false)
        });

        // Bind del to delete mode
        self.cursive
            .set_on_post_event(Event::Key(Key::Del), |cursive| Self::delete_dialog(cursive));
    }

    /// Used to lay out all views in the TUI instance.
    fn layout(&mut self) {
        // List view is the primary(unchangin) view for displaying data
        let list_view: SelectView<usize> = SelectView::new()
            .on_submit(|cursive, index| Self::list_view_on_submit(cursive, *index))
            .h_align(HAlign::Left)
            .v_align(VAlign::Top);

        // The scroll view for exclusively vertical scrolling of the list view
        let list_view_scroll =
            ScrollView::new(list_view.with_name(TUI_LIST_ID)).with_name(TUI_LIST_SCROLL_ID);

        // The list view header for designating what each column is/represents
        let list_view_header = TextView::new("").with_name(TUI_LIST_HEADER_ID);

        // Align everything vertically...
        let list_layout = LinearLayout::vertical()
            .child(list_view_header)
            .child(list_view_scroll);

        // And wrap it in a horizontal scroll...
        let list_layout_scroll = ScrollView::new(list_layout).scroll_y(false).scroll_x(true);

        // Finally the status header which just displays program status
        let status_header = TextView::new("Loading...")
            .center()
            .with_name(TUI_STATUS_HEADER_ID);

        self.cursive.clear();

        let mut layout = LinearLayout::vertical()
            .child(status_header)
            .child(list_layout_scroll);

        layout.focus_view(&Selector::Name(TUI_LIST_ID)).unwrap();

        self.cursive.add_fullscreen_layer(layout.full_width());
    }

    /// Called when enter is pressed on a the entry/catagory list view.
    fn list_view_on_submit(cursive: &mut Cursive, index: usize) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Only do something if in catagory view
        if cache.catagory_selected.len() == 0 {
            let catagory_name = cache.catagories_queried[index].clone();

            Self::populate_with_entries(cursive, &catagory_name);
        }
    }

    /// Populate the list view with catagories.
    fn populate_with_catagories(cursive: &mut Cursive) {
        // Grab all the views needed
        let mut list_view: ViewRef<SelectView<usize>> = cursive.find_name(TUI_LIST_ID).unwrap();
        let mut list_view_header: ViewRef<TextView> =
            cursive.find_name(TUI_LIST_HEADER_ID).unwrap();
        let mut status_header: ViewRef<TextView> = cursive.find_name(TUI_STATUS_HEADER_ID).unwrap();

        list_view.clear();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Set the status to inform the user that they're in catagory view
        status_header.set_content("CATAGORY VIEW");

        let catagories = match cache.db.list_catagories() {
            Ok(catagories) => catagories,
            Err(error) => {
                Self::fatal_error_dialog(cursive, error);
                return;
            }
        };

        let catagory_table = match cache.db.stat_catagories() {
            Ok(catagory_table) => catagory_table,
            Err(error) => {
                Self::fatal_error_dialog(cursive, error);
                return;
            }
        };

        let headers = vec!["NAME".to_string(), "ENTRIES".to_string()];

        let columnated_catagories = Self::columnator(headers, catagory_table);

        // Set the header to the first row
        list_view_header.set_content(&columnated_catagories[0]);

        for (i, name) in columnated_catagories[1..].iter().enumerate() {
            list_view.add_item(name, i);
        }

        cache.catagories_queried = catagories;
        cache.catagory_selected = String::new();
    }

    /// Populate the list view with entries and select an entry based off the
    /// given key
    fn populate_with_entries_and_select(cursive: &mut Cursive, catagory_name: &str, key: u64) {
        // Grab all the views needed
        let mut list_view: ViewRef<SelectView<usize>> = cursive.find_name(TUI_LIST_ID).unwrap();
        let mut list_view_header: ViewRef<TextView> =
            cursive.find_name(TUI_LIST_HEADER_ID).unwrap();
        let mut status_header: ViewRef<TextView> = cursive.find_name(TUI_STATUS_HEADER_ID).unwrap();
        let mut list_view_scroll: ViewRef<ScrollView<NamedView<SelectView<usize>>>> =
            cursive.find_name(TUI_LIST_SCROLL_ID).unwrap();

        list_view.clear();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Set the status to inform the user that they're in entry view
        status_header.set_content(&format!("ENTRY VIEW (CATAGORY={})", catagory_name));

        let entries = match cache.db.search_catagory(&catagory_name, vec!["KEY>=0"]) {
            Ok(entries) => entries,
            Err(error) => {
                Self::fatal_error_dialog(cursive, error);
                return;
            }
        };

        // Grab the catagory's field headers
        let headers = match cache.db.grab_catagory_fields(&catagory_name) {
            Ok(headers) => headers,
            Err(error) => {
                Self::fatal_error_dialog(cursive, error);
                return;
            }
        };

        // Convert the entries into a table
        let mut entry_table = Vec::<Vec<String>>::with_capacity(entries.len());

        cache.entry_selected = 0;

        for (i, entry) in entries.iter().enumerate() {
            let created_str = Local.timestamp_opt(entry.created, 0).unwrap().to_string();
            let modified_str = Local.timestamp_opt(entry.modified, 0).unwrap().to_string();

            // If the key is equal to the one specified, select it
            if entry.key == key {
                cache.entry_selected = i;
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

        list_view_header.set_content(&columnated_entries[0]);

        for (i, entry) in columnated_entries[1..].iter().enumerate() {
            list_view.add_item(entry, i);
        }

        // Focus on the selected entry
        list_view.set_selection(cache.entry_selected); // Ignore the callback
        drop(list_view); // Will not scroll unless the list view is dropped
        list_view_scroll.scroll_to_important_area();

        cache.catagory_selected = catagory_name.to_string();
        cache.entries_queried = entries;
    }

    /// Populate the list view with entries.
    fn populate_with_entries(cursive: &mut Cursive, catagory_name: &str) {
        Self::populate_with_entries_and_select(cursive, catagory_name, 0);
    }

    /// Function called when escape is pressed.
    fn escape(cursive: &mut Cursive) {
        // Grab the cache
        let mut cache = cursive.user_data::<TuiCache>().unwrap();

        // If in a dialog, simply pop the dialog...
        if cache.dialog_layers > 0 {
            cache.dialog_layers -= 1;
            cursive.pop_layer();
            return;
        }

        // If the list view is currently populated with entries, go back and populate with columns
        // instead...
        if cache.catagory_selected.len() != 0 {
            Self::populate_with_catagories(cursive);
            return;
        }

        // Otherwise, exit the program
        Self::exit_dialog(cursive);
    }

    /// Dialog used to find an entry given only a key
    fn find_dialog(cursive: &mut Cursive) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // If we're already in a dialog, do nothing
        if cache.dialog_layers > 0 {
            return;
        }

        let find_view = TextView::new("Key: ");
        let find_edit = EditView::new()
            .on_submit(|cursive, _| Self::find_dialog_submit(cursive))
            .with_name(TUI_FIND_KEY_ID)
            .fixed_width(TUI_FIELD_ENTRY_WIDTH);

        let find_row = LinearLayout::horizontal().child(find_view).child(find_edit);

        let dialog = Dialog::around(find_row)
            .button("Find", |cursive| Self::find_dialog_submit(cursive))
            .title("Find Entry");

        cache.dialog_layers += 1;
        cursive.add_layer(dialog);
    }

    /// Function called when the find button is selected in the find dialog
    fn find_dialog_submit(cursive: &mut Cursive) {
        let find_edit: ViewRef<EditView> = cursive.find_name(TUI_FIND_KEY_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let key_str = find_edit.get_content();
        let key = b64::to_u64(&key_str);

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

        cache.dialog_layers -= 1;
        cursive.pop_layer();
        Self::populate_with_entries_and_select(cursive, &catagory_name, key);
    }

    /// Dialog used to add either an entry or a catagory depending on the view.
    fn add_dialog(cursive: &mut Cursive) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        if cache.dialog_layers > 0 {
            return;
        }

        // See whether we're in catagory or entry view, and choose the correct dialog accordingly
        if cache.catagory_selected.len() == 0 {
            Self::add_catagory_dialog(cursive);
        } else {
            Self::add_entry_dialog(cursive);
        }
    }

    /// Dialog used to add a catagory.
    fn add_catagory_dialog(cursive: &mut Cursive) {
        // Grab the cache
        let mut cache = cursive.user_data::<TuiCache>().unwrap();

        let name_view = TextView::new("Name: ");
        let name_edit = EditView::new()
            .with_name(TUI_CATAGORY_NAME_ID)
            .fixed_width(TUI_FIELD_ENTRY_WIDTH);

        let name_row = LinearLayout::horizontal().child(name_view).child(name_edit);

        let add_field_button = Button::new("Add Field", |cursive| {
            Self::add_catagory_field_dialog(cursive)
        });

        let field_list = TextView::new("").with_name(TUI_FIELD_LIST_ID);

        let layout = LinearLayout::vertical()
            .child(name_row)
            .child(add_field_button)
            .child(field_list);

        let dialog = Dialog::around(layout)
            .title("Add Catagory")
            .button("Add Catagory", |cursive| {
                Self::add_catagory_dialog_submit(cursive)
            });

        cache.dialog_layers += 1;
        cursive.add_layer(dialog);
    }

    /// Function called when the submit button is pressed in the add catagory
    /// dialog.
    fn add_catagory_dialog_submit(cursive: &mut Cursive) {
        // Grab the views we need
        let catagory_name_view: ViewRef<EditView> =
            cursive.find_name(TUI_CATAGORY_NAME_ID).unwrap();
        let field_list_view: ViewRef<TextView> = cursive.find_name(TUI_FIELD_LIST_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let catagory_name = catagory_name_view.get_content();
        let field_list_content = field_list_view.get_content();

        let field_strs: Vec<&str> = field_list_content.source().split('\n').collect();

        let fields: Vec<CatagoryField> = field_strs
            .iter()
            .map(|field_str| CatagoryField::from_str(&field_str).unwrap())
            .collect();

        let catagory = Catagory::with_fields(&catagory_name, fields);

        match cache.db.add_catagory(catagory) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        cache.dialog_layers -= 1;
        cursive.pop_layer();

        Self::populate_with_catagories(cursive);
    }

    /// Dialog used to add a field to a catagory.
    fn add_catagory_field_dialog(cursive: &mut Cursive) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

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

        cache.dialog_layers += 1;
        cursive.add_layer(dialog);
    }

    /// Function called when the submit button is pressed in the add catagory
    /// field dialog.
    fn add_catagory_field_submit(cursive: &mut Cursive) {
        // Grab the views we need
        let type_menu_view: ViewRef<SelectView<db::DataType>> =
            cursive.find_name(TUI_TYPE_MENU_ID).unwrap();
        let mut field_list_view: ViewRef<TextView> = cursive.find_name(TUI_FIELD_LIST_ID).unwrap();
        let field_name_view: ViewRef<EditView> = cursive.find_name(TUI_FIELD_NAME_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Need to make sure the content of the field list view isn't in use by something else when
        // set content is called, so ensure the lifetime of any modifications end here
        let catagory_field_string = {
            let mut old_view_string = field_list_view.get_content().source().to_string();

            if old_view_string.len() > 0 {
                old_view_string.push('\n');
            }

            format!(
                "{}{}:{}",
                old_view_string,
                field_name_view.get_content(),
                type_menu_view.selection().unwrap().get_char()
            )
        };

        field_list_view.set_content(catagory_field_string);

        cache.dialog_layers -= 1;
        cursive.pop_layer();
    }

    /// Dialog used to add an entry to the database.
    fn add_entry_dialog(cursive: &mut Cursive) {
        // Grab the cache
        let mut cache = cursive.user_data::<TuiCache>().unwrap();

        let mut layout = LinearLayout::vertical();

        let fields = match cache.db.grab_catagory_fields(&cache.catagory_selected) {
            Ok(fields) => fields,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        // Remove created and modified because they are autogenerated
        let fields_a: Vec<String> = fields[..3].into();
        let fields_b: Vec<String> = fields[5..].into();
        let fields = [fields_a, fields_b].concat();

        // Subtract 2 because the created and modified date fields are autogenerated
        cache.fields_edited = vec![String::new(); fields.len()];

        // First find the largest field name
        let mut max_size: usize = 0;

        for field in &fields {
            max_size = cmp::max(max_size, field.len())
        }

        for (i, field) in fields.iter().enumerate() {
            let field = format!("{}:", field);
            let field_id = TextView::new(format!("{:<width$}", field, width = max_size + 2));
            let field_entry = EditView::new()
                .on_edit(move |cursive, string, _| Self::edit_field(cursive, string, i))
                .fixed_width(TUI_FIELD_ENTRY_WIDTH);

            let row = LinearLayout::horizontal()
                .child(field_id)
                .child(field_entry);

            layout.add_child(row);
        }

        let dialog = Dialog::around(layout)
            .title(format!("Add entry to {}...", cache.catagory_selected))
            .button("Add", |cursive| Self::add_entry_submit(cursive));

        cache.dialog_layers += 1;
        cursive.add_layer(dialog);
    }

    /// Function called when a field is edited(will most likely be removed in
    /// future updates)
    fn edit_field(cursive: &mut Cursive, string: &str, number: usize) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        cache.fields_edited[number] = string.to_string();
    }

    /// Function called when the submit button is pressed in the add entry
    /// dialog.
    fn add_entry_submit(cursive: &mut Cursive) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Ignore the first 5 fields we won't need them
        let fields = match cache.db.grab_catagory_fields(&cache.catagory_selected) {
            Ok(fields) => fields,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        let fields = &fields[5..]; // Ignore the first 5 fields

        let types = match cache.db.grab_catagory_types(&cache.catagory_selected) {
            Ok(types) => types,
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        };

        let types = &types[5..];

        let key = b64::to_u64(&cache.fields_edited[0]);
        let location = &cache.fields_edited[1];
        let quantity: u64 = match cache.fields_edited[2].parse() {
            Ok(quantity) => quantity,
            Err(error) => {
                Self::error_dialog(cursive, Box::new(error));
                return;
            }
        };
        let created = Local::now().timestamp();
        let modified = created;

        let mut entry = Entry::new(
            &cache.catagory_selected,
            key,
            location,
            quantity,
            created,
            modified,
        );

        for (i, value) in cache.fields_edited[3..].iter().enumerate() {
            if value.len() > 0 {
                let value_sql: String = match types[i] {
                    'i' | 'r' => value.clone(),
                    't' | _ => format!("'{}'", value),
                };
                entry.add_field(EntryField::new(&fields[i], &value_sql));
            }
        }

        eprintln!("{}", entry.to_string());

        match cache.db.add_entry(entry) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        }

        let catagory = cache.catagory_selected.clone();

        cache.dialog_layers -= 1;
        cursive.pop_layer();

        Self::populate_with_entries(cursive, &catagory);
    }

    /// Dialog used when either giving or taking from an entry. If give is
    /// true, we are giving to an entry. If false, we are taking from an entry.
    fn give_take_dialog(cursive: &mut Cursive, give: bool) {
        let list_view: ViewRef<SelectView<usize>> = cursive.find_name(TUI_LIST_ID).unwrap();

        // Grab the cache
        let mut cache = cursive.user_data::<TuiCache>().unwrap();

        // Return if already in a dialog or if not in entry mode
        if cache.dialog_layers > 0 || cache.catagory_selected.len() == 0 {
            return;
        }

        // Get the entry to give or take from
        let entry_pos: usize = list_view.selection().unwrap().as_ref().clone();
        let entry = &cache.entries_queried[entry_pos];

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

        cache.fields_edited = vec!["1".to_string()];

        let old_quantity_view = TextView::new(format!("Old Quantity: {}", quantity));

        // Create the entry row
        let quantity_entry_view = TextView::new(format!("{}: ", give_or_take));

        let give_take_edit = EditView::new()
            .content("1")
            .on_edit(move |cursive, string, _| {
                Self::edit_field(cursive, string, 0);
                Self::give_take_dialog_update(cursive, give);
            })
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

        cache.entry_selected = entry_pos;
        cache.dialog_layers += 1;
        cursive.add_layer(dialog);
    }

    /// Update the dialog to reflect the new quantity
    fn give_take_dialog_update(cursive: &mut Cursive, give: bool) {
        let mut new_quantity_view: ViewRef<TextView> =
            cursive.find_name(TUI_NEW_QUANTITY_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        let give_take_amt: u64 = match cache.fields_edited[0].parse() {
            Ok(number) => number,
            Err(_) => {
                return;
            }
        };

        let entry_pos = cache.entry_selected;
        let entry = &cache.entries_queried[entry_pos];

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
        // Grab the cache
        let mut cache = cursive.user_data::<TuiCache>().unwrap();

        let give_take_amt: u64 = match cache.fields_edited[0].parse() {
            Ok(number) => number,
            Err(_) => {
                return;
            }
        };

        let entry_pos = cache.entry_selected;
        let entry = &cache.entries_queried[entry_pos];

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

        let catagory = cache.catagory_selected.clone();

        cache.dialog_layers -= 1;
        cursive.pop_layer();

        Self::populate_with_entries(cursive, &catagory);
    }

    /// Dialog that confirms if you wish to delete an entry, and if so, deletes
    /// the entry.
    fn delete_dialog(cursive: &mut Cursive) {
        let list_view: ViewRef<SelectView<usize>> = cursive.find_name(TUI_LIST_ID).unwrap();

        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        // Return if already in a dialog or if not in entry mode
        if cache.dialog_layers > 0 || cache.catagory_selected.len() == 0 {
            return;
        }

        // Get the entry to give or take from
        let entry_pos: usize = list_view.selection().unwrap().as_ref().clone();
        let entry_key = cache.entries_queried[entry_pos].key;

        // Create the dialog
        let dialog = Dialog::text(format!("Delete entry {}?", b64::from_u64(entry_key)))
            .button("No...", |cursive| {
                // Grab the cache
                let mut cache = cursive.user_data::<TuiCache>().unwrap();

                cache.dialog_layers -= 1;
                cursive.pop_layer();
            })
            .button("Yes!", move |cursive| {
                Self::delete_dialog_submit(cursive, entry_key);
            });

        cache.dialog_layers += 1;
        cursive.add_layer(dialog);
    }

    /// Deletes the entry if "Yes" is selected on the delete dialog.
    fn delete_dialog_submit(cursive: &mut Cursive, key: u64) {
        // Grab the cache
        let cache = cursive.user_data::<TuiCache>().unwrap();

        match cache.db.delete_entry(key) {
            Ok(_) => {}
            Err(error) => {
                Self::error_dialog(cursive, error);
                return;
            }
        }

        let catagory = cache.catagory_selected.clone();

        cache.dialog_layers -= 1;
        cursive.pop_layer();

        Self::populate_with_entries(cursive, &catagory);
    }

    /// Dialog used to confirm that a used wishes to exit the program.
    fn exit_dialog(cursive: &mut Cursive) {
        let exit_dialog = Dialog::text("Are You Sure You Want To Exit?")
            .button("No...", |cursive| {
                cursive.pop_layer().unwrap();
            })
            .button("Yes!", |cursive| cursive.quit());

        cursive.add_layer(exit_dialog);
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
    /// The currently selected catagory. If empty, we are in catagory view.
    pub catagory_selected: String,
    /// The catagories queried. This is to prevent issues incase the database
    /// is altered outside of pinv while the program is running.
    pub catagories_queried: Vec<String>,
    /// The entries queried. This is to prevent issues incase the database is
    /// altered outside of pinv while the program is running.
    pub entries_queried: Vec<Entry>,
    /// Index of the selected entry
    pub entry_selected: usize,
    /// Number of dialog windows opened
    pub dialog_layers: usize,
    /// Database in use
    pub db: Db,
    /// May be removed in future update, used to hold the value of dialog
    /// fields.
    pub fields_edited: Vec<String>,
}
