
use cursive::Cursive;
use cursive::CursiveExt;
use cursive::View;
use cursive::view::Nameable;
use cursive::view::Resizable;
use cursive::view::Selector;
use cursive::views::LinearLayout;
use cursive::views::ScrollView;
use cursive::views::SelectView;
use cursive::align::HAlign;
use cursive::align::VAlign;
use cursive::views::ViewRef;
use std::error::Error;

use crate::db::Db;

// ID of the list view
const TUI_LIST_ID: &str = "lst";

pub struct Tui {
    cursive: Cursive,
    db: Db,
}

impl Tui {
    pub fn new(mut db: Db) -> Result<Self, Box<dyn Error>> {
        let mut tui = Self {cursive: Cursive::new(), db: db};

        tui.prime(); // Prime all event handlers
        tui.layout(); // Lay out all the views
        
        Ok(tui)
    }

    pub fn run(&mut self) {
        self.populate_with_catagories();
        self.cursive.run_crossterm().unwrap();
    }

    fn prime(&mut self) {
    }

    fn layout(&mut self) {

        let mut list_view: SelectView<usize> = SelectView::new().on_submit(|cursive, index| Self::list_view_on_submit(cursive, *index)).h_align(HAlign::Left).v_align(VAlign::Top);

        let mut list_view_scroll = ScrollView::new(list_view.with_name(TUI_LIST_ID));

        self.cursive.clear();

        let mut layout = LinearLayout::vertical().child(list_view_scroll);

        layout.focus_view(&Selector::Name(TUI_LIST_ID)).unwrap();
        
        self.cursive.add_fullscreen_layer(layout.full_width());
    }

    fn list_view_on_submit(cursive: &mut Cursive, index: usize) {
        let mut list_view: ViewRef<SelectView<usize>> = cursive.find_name(TUI_LIST_ID).unwrap();
        // Grab the cache
        let mut cache = match cursive.user_data::<TuiCache>() {
            Some(cache) => cache,
            None => {
                panic!("Failed to initialize Cursive instance with cache! this should not happen!");
            }
        };

        let catagory_name = cache.catagories_queried[index].clone();

        todo!();
    }

    fn populate_with_catagories(&mut self) {
        let mut list_view: ViewRef<SelectView<usize>> = self.cursive.find_name(TUI_LIST_ID).unwrap();

        let catagories = self.db.list_catagories().unwrap();

        for (i, name) in catagories.iter().enumerate() {
            list_view.add_item(name, i);
        }
    }
}

pub struct TuiCache {
    pub catagory_selected: String,
    pub catagories_queried: Vec<String>,
}
