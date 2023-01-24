
use cursive::Cursive;
use cursive::CursiveExt;
use cursive::View;
use cursive::event::Key;
use cursive::event::Event;
use cursive::view::Nameable;
use cursive::view::Resizable;
use cursive::view::Selector;
use cursive::views::Dialog;
use cursive::views::EditView;
use cursive::views::LinearLayout;
use cursive::views::ScrollView;
use cursive::views::SelectView;
use cursive::align::HAlign;
use cursive::align::VAlign;
use cursive::views::TextView;
use cursive::views::ViewRef;
use chrono::{Local, TimeZone};
use crate::b64;
use crate::db::Entry;
use crate::db::EntryField;
use std::error::Error;
use std::cmp;

use crate::db::Db;

// ID of the list view
static TUI_LIST_ID: &str = "lst";

// ID of the list header
static TUI_LIST_HEADER_ID: &str = "lhd";

// ID of the status header
static TUI_STATUS_HEADER_ID: &str = "shd";

// Column Padding
static TUI_COLUMN_PADDING: &str = " | ";

// Column Padding Width
const TUI_COLUMN_PADDING_LEN: usize = 3;

// Field Entry Width
const TUI_FIELD_ENTRY_WIDTH: usize = 16;

pub struct Tui {
    cursive: Cursive,
}

impl Tui {
    pub fn new(mut db: Db) -> Result<Self, Box<dyn Error>> {
        let mut tui = Self {cursive: Cursive::new()};

        let tui_cache = TuiCache {catagory_selected: String::new(), catagories_queried: vec![], in_dialog: false, db: db, fields_edited: vec![String::new()]};

        tui.cursive.set_user_data(tui_cache);

        tui.prime(); // Prime all event handlers
        tui.layout(); // Lay out all the views
        
        Ok(tui)
    }

    pub fn run(&mut self) {
        Self::populate_with_catagories(&mut self.cursive);
        self.cursive.run_crossterm().unwrap();
    }

    fn prime(&mut self) {
        // Bind escape to a special function which will either exit entry view or exit the program,
        // depending on what view we're in. Make it a post binding since we only want it to trigger
        // when in either catagory or entry view, not in creation dialogs or etc.
        self.cursive.set_on_post_event(Event::Key(Key::Esc), |cursive| Self::escape(cursive));

        self.cursive.set_on_post_event(Event::Char('a'), |cursive| Self::add_dialog(cursive));
    }

    fn layout(&mut self) {
        // List view is the primary(unchangin) view for displaying data
        let list_view: SelectView<usize> = SelectView::new().on_submit(|cursive, index| Self::list_view_on_submit(cursive, *index)).h_align(HAlign::Left).v_align(VAlign::Top);

        // The scroll view for exclusively vertical scrolling of the list view
        let list_view_scroll = ScrollView::new(list_view.with_name(TUI_LIST_ID));

        // The list view header for designating what each column is/represents
        let list_view_header = TextView::new("").with_name(TUI_LIST_HEADER_ID);

        // Align everything vertically...
        let list_layout = LinearLayout::vertical().child(list_view_header).child(list_view_scroll);

        // And wrap it in a horizontal scroll...
        let list_layout_scroll = ScrollView::new(list_layout).scroll_y(false).scroll_x(true);

        // Finally the status header which just displays program status
        let status_header = TextView::new("Loading...").center().with_name(TUI_STATUS_HEADER_ID);

        self.cursive.clear();

        let mut layout = LinearLayout::vertical().child(status_header).child(list_layout_scroll);

        layout.focus_view(&Selector::Name(TUI_LIST_ID)).unwrap();
        
        self.cursive.add_fullscreen_layer(layout.full_width());
    }

    fn list_view_on_submit(cursive: &mut Cursive, index: usize) {
        // Grab the cache
        let cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        if cache.catagory_selected.len() == 0 {
            let catagory_name = cache.catagories_queried[index].clone();

            Self::populate_with_entries(cursive, &catagory_name);
        }
    }

    fn populate_with_catagories(cursive: &mut Cursive) {
        // Grab all the views needed
        let mut list_view: ViewRef<SelectView<usize>> = cursive.find_name(TUI_LIST_ID).unwrap();
        let mut list_view_header: ViewRef<TextView> = cursive.find_name(TUI_LIST_HEADER_ID).unwrap();
        let mut status_header: ViewRef<TextView> = cursive.find_name(TUI_STATUS_HEADER_ID).unwrap();
        
        list_view.clear();

        // Grab the cache
        let cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        // Set the status to inform the user that they're in catagory view
        status_header.set_content("CATAGORY VIEW");

        let catagories = cache.db.list_catagories().unwrap();
        let catagory_table = cache.db.stat_catagories().unwrap();

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

    fn populate_with_entries(cursive: &mut Cursive, catagory_name: &str) {
        // Grab all the views needed
        let mut list_view: ViewRef<SelectView<usize>> = cursive.find_name(TUI_LIST_ID).unwrap();
        let mut list_view_header: ViewRef<TextView> = cursive.find_name(TUI_LIST_HEADER_ID).unwrap();
        let mut status_header: ViewRef<TextView> = cursive.find_name(TUI_STATUS_HEADER_ID).unwrap();

        list_view.clear();
        
        // Grab the cache
        let cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        // Set the status to inform the user that they're in entry view
        status_header.set_content(&format!("ENTRY VIEW (CATAGORY={})", catagory_name));

        let entries = cache.db.search_catagory(&catagory_name, vec!["KEY>=0"]).unwrap();

        // Grab the catagory's field headers
        let headers = cache.db.grab_catagory_fields(&catagory_name).unwrap();

        // Convert the entries into a table
        let mut entry_table = Vec::<Vec<String>>::with_capacity(entries.len());

        for entry in &entries {
            let created_str = Local.timestamp(entry.created, 0).to_string();
            let modified_str = Local.timestamp(entry.modified, 0).to_string();

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

        cache.catagory_selected = catagory_name.to_string();
    }

    fn escape(cursive: &mut Cursive) {
        // Grab the cache
        let mut cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        // If in a dialog, simply pop the dialog...
        if cache.in_dialog == true {
            cache.in_dialog = false;
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

    fn add_dialog(cursive: &mut Cursive) {
        // Grab the cache
        let cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        if cache.in_dialog == true {
            return;
        }


        // See whether we're in catagory or entry view, and choose the correct dialog accordingly
        if cache.catagory_selected.len() == 0 {
            Self::add_catagory_dialog(cursive);
        }

        else {
            Self::add_entry_dialog(cursive);
        }
    }

    fn add_catagory_dialog(cursive: &mut Cursive) {
    }

    fn add_entry_dialog(cursive: &mut Cursive) {
        // Grab the cache
        let mut cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        cache.in_dialog = true;
        
        let mut layout = LinearLayout::vertical();

        let fields = cache.db.grab_catagory_fields(&cache.catagory_selected).unwrap();

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
            let field_entry = EditView::new().on_edit(move |cursive, string, _| {Self::edit_field(cursive, string, i)}).fixed_width(TUI_FIELD_ENTRY_WIDTH);

            let row = LinearLayout::horizontal().child(field_id).child(field_entry);

            layout.add_child(row);
        }

        let dialog = Dialog::around(layout).title(format!("Add entry to {}...", cache.catagory_selected))
            .button("Add", |cursive| Self::add_entry_submit(cursive));

        cursive.add_layer(dialog);
    }

    fn edit_field(cursive: &mut Cursive, string: &str, number: usize) {
        // Grab the cache
        let cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        cache.fields_edited[number] = string.to_string();
    }

    fn add_entry_submit(cursive: &mut Cursive) {
        // Grab the cache
        let cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        let fields = cache.db.grab_catagory_fields(&cache.catagory_selected).unwrap();
        
        // Remove created and modified because they are autogenerated
        let fields_a: Vec<String> = fields[..3].into();
        let fields_b: Vec<String> = fields[5..].into();
        let fields = [fields_a, fields_b].concat();

        let key = b64::to_u64(&cache.fields_edited[0]);
        let location = &cache.fields_edited[1];
        let quantity: u64 = cache.fields_edited[2].parse().unwrap();
        let created = Local::now().timestamp();
        let modified = created;

        let mut entry = Entry::new(&cache.catagory_selected, key, location, quantity, created, modified);
        
        for (i, value) in cache.fields_edited[3..].iter().enumerate() {
            if value.len() > 0 {
                entry.add_field(EntryField::new(&fields[i+3], value));
            }
        }

        eprintln!("{}", entry.to_string());

        cache.db.add_entry(entry).unwrap();

        let catagory = cache.catagory_selected.clone();

        cache.in_dialog = false;
        cursive.pop_layer();

        Self::populate_with_entries(cursive, &catagory);
    }

    fn exit_dialog(cursive: &mut Cursive) {
        let exit_dialog = Dialog::text("Are You Sure You Want To Exit?")
            .button("No...", |cursive| {cursive.pop_layer().unwrap();})
            .button("Yes!", |cursive| cursive.quit());
        
        cursive.add_layer(exit_dialog);
    }

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
            out_string.push_str(&format!("{:<width$}{}", header, TUI_COLUMN_PADDING, width = column_widths[i]));
        }

        out_strings.push(out_string);
        
        for row in table {
            let mut out_string = String::with_capacity(out_string_size);
            
            for (i, column) in row.iter().enumerate() {
                out_string.push_str(&format!("{:<width$}{}", column, TUI_COLUMN_PADDING, width = column_widths[i]));
            }

            out_strings.push(out_string);

        }

        out_strings
    }
}

pub struct TuiCache {
    pub catagory_selected: String, // If empty, we know we are at catagory view
    pub catagories_queried: Vec<String>,
    pub in_dialog: bool,
    pub db: Db,
    pub fields_edited: Vec<String>,
}
