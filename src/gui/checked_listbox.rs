use gtk::{glib, prelude::*};
use glib::clone;
use std::{cell::RefCell, rc::Rc};

mod prop_names {
    pub const ID: &str = "id";
    pub const CHECKED: &str = "checked";
    pub const LABEL: &str = "label";
}

mod row_data {
    use super::prop_names;

    mod imp {
        use super::prop_names;
        use glib::subclass::prelude::*;
        use gtk::{glib, prelude::*};
        use std::cell::RefCell;

        #[derive(Default)]
        pub struct RowData {
            id: RefCell<u64>,
            checked: RefCell<bool>,
            label: RefCell<String>
        }

        #[glib::object_subclass]
        impl ObjectSubclass for RowData {
            const NAME: &'static str = "ControllerListRowData";
            type Type = super::RowData;
            type ParentType = glib::Object;
        }

        impl ObjectImpl for RowData {
            fn properties() -> &'static [glib::ParamSpec] {
                use gtk::glib::once_cell::sync::Lazy;
                static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                    vec![
                        glib::ParamSpec::new_uint64(
                            prop_names::ID,
                            "Id",
                            "Id",
                            0,
                            std::u64::MAX,
                            0,
                            glib::ParamFlags::READWRITE
                        ),
                        glib::ParamSpec::new_boolean(
                            prop_names::CHECKED,
                            "Checked",
                            "Checked",
                            true,
                            glib::ParamFlags::READWRITE,
                        ),
                        glib::ParamSpec::new_string(
                            prop_names::LABEL,
                            "Label",
                            "Label",
                            "".into(),
                            glib::ParamFlags::READWRITE
                        ),
                    ]
                });

                PROPERTIES.as_ref()
            }

            fn set_property(
                &self,
                _obj: &Self::Type,
                _id: usize,
                value: &glib::Value,
                pspec: &glib::ParamSpec,
            ) {
                match pspec.name() {
                    prop_names::ID => {
                        let v = value
                            .get()
                            .expect("type conformity checked by `Object::set_property`");
                        self.id.replace(v);
                    },
                    prop_names::CHECKED => {
                        let v = value
                            .get()
                            .expect("type conformity checked by `Object::set_property`");
                        self.checked.replace(v);
                    },
                    prop_names::LABEL => {
                        let v = value
                            .get()
                            .expect("type conformity checked by `Object::set_property`");
                        self.label.replace(v);
                    }
                    _ => unimplemented!(),
                }
            }

            fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
                match pspec.name() {
                    prop_names::ID => self.id.borrow().to_value(),
                    prop_names::CHECKED => self.checked.borrow().to_value(),
                    prop_names::LABEL => self.label.borrow().to_value(),
                    _ => unimplemented!(),
                }
            }
        }
    }

    gtk::glib::wrapper! {
        pub struct RowData(ObjectSubclass<imp::RowData>);
    }

    impl RowData {
        pub fn new(id: u64, checked: bool, label: &str) -> RowData {
            gtk::glib::Object::new(&[
                (prop_names::ID, &id),
                (prop_names::CHECKED , &checked),
                (prop_names::LABEL, &label)
            ]).expect("failed to create row data")
        }
    }
}

mod model {
    use glib::subclass::prelude::*;
    use gtk::{gio, glib, prelude::*};
    use super::row_data::RowData;

    mod imp {
        use gio::subclass::prelude::*;
        use gtk::{gio, glib, prelude::*};
        use std::cell::RefCell;
        use super::RowData;

        #[derive(Debug, Default)]
        pub struct Model(pub RefCell<Vec<RowData>>);

        #[glib::object_subclass]
        impl ObjectSubclass for Model {
            const NAME: &'static str = "ControllerListModel";
            type Type = super::Model;
            type ParentType = glib::Object;
            type Interfaces = (gio::ListModel,);
        }

        impl ObjectImpl for Model {}

        impl ListModelImpl for Model {
            fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
                RowData::static_type()
            }
            fn n_items(&self, _list_model: &Self::Type) -> u32 {
                self.0.borrow().len() as u32
            }
            fn item(&self, _list_model: &Self::Type, position: u32) -> Option<glib::Object> {
                self.0
                    .borrow()
                    .get(position as usize)
                    .map(|o| o.clone().upcast::<glib::Object>())
            }
        }
    }

    glib::wrapper! {
        pub struct Model(ObjectSubclass<imp::Model>) @implements gio::ListModel;
    }

    impl Model {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Model {
            glib::Object::new(&[]).expect("Failed to create Model")
        }

        pub fn append(&self, obj: &RowData) {
            let self_ = imp::Model::from_instance(self);
            let index = {
                let mut data = self_.0.borrow_mut();
                data.push(obj.clone());
                data.len() - 1
            };
            self.items_changed(index as u32, 0, 1);
        }

        pub fn remove(&self, index: u32) {
            let self_ = imp::Model::from_instance(self);
            self_.0.borrow_mut().remove(index as usize);
            self.items_changed(index, 1, 0);
        }
    }
}

#[derive(Clone, glib::Downgrade)]
pub struct CheckedListBox {
    listbox: gtk::ListBox,
    model: model::Model,
    item_toggled_handler: Rc<RefCell<Option<Box<dyn Fn(u64, bool) + 'static>>>>
}

impl CheckedListBox {
    pub fn widget(&self) -> gtk::Widget {
        self.listbox.clone().upcast::<gtk::Widget>()
    }

    pub fn new() -> CheckedListBox {
        let listbox = gtk::ListBox::new();
        let model = model::Model::new();
        let item_toggled_handler: Rc<RefCell<Option<Box<dyn Fn(u64, bool) + 'static>>>> = Rc::new(RefCell::new(None));

        listbox.bind_model(Some(&model), clone!(@weak item_toggled_handler => @default-panic, move |item| {
            use row_data::RowData;
            let box_ = gtk::ListBoxRow::new();
            let item = item.downcast_ref::<RowData>().expect("row data is of wrong type");
            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 5);

            let cbox = gtk::CheckButton::new();
            cbox.connect_clicked(clone!(@weak item_toggled_handler, @weak item => @default-panic, move |cbox| {
                let handler = item_toggled_handler.borrow();
                if let Some(handler) = handler.as_ref() {
                    handler(item.property(prop_names::ID).unwrap().get::<u64>().unwrap(), cbox.is_active());
                }
            }));
            item.bind_property(prop_names::CHECKED, &cbox, "active")
                .flags(gtk::glib::BindingFlags::BIDIRECTIONAL | gtk::glib::BindingFlags::SYNC_CREATE)
                .build();
            hbox.pack_start(&cbox, false, false, 0);

            let label = gtk::Label::new(None);
            item.bind_property(prop_names::LABEL, &label, "label")
                .flags(gtk::glib::BindingFlags::DEFAULT | gtk::glib::BindingFlags::SYNC_CREATE)
                .build();
            hbox.pack_start(&label, false, true, 0);

            box_.add(&hbox);
            box_.show_all();
            box_.upcast::<gtk::Widget>()
        }));

        // listbox.set_header_func(Some(Box::new(clone!(@weak listbox => @default-panic, move |row, _row_before| {
        //     row.set_header(Some(&gtk::Label::new(Some("header!"))));
        // }))));

        CheckedListBox{ listbox, model, item_toggled_handler }
    }

    pub fn add_item(&self, id: u64, checked: bool, label: &str) {
        self.model.append(&row_data::RowData::new(id, checked, label));
    }

    pub fn remove_item(&self, id: u64) {
        for i in 0..self.model.n_items() {
            if self.model.item(i).as_ref().unwrap().property(prop_names::ID).unwrap().get::<u64>().unwrap() == id {
                self.model.remove(i);
                break;
            }
        }
    }

    pub fn is_item_checked(&self, id: u64) -> bool {
        for i in 0..self.model.n_items() {
            macro_rules! item { () => { self.model.item(i).as_ref().unwrap() } }

            if item!().property(prop_names::ID).unwrap().get::<u64>().unwrap() == id {
                return item!().property(prop_names::CHECKED).unwrap().get::<bool>().unwrap();
            }
        }

        panic!("item it not found");
    }

    pub fn on_item_toggled<F: Fn(u64, bool) + 'static>(&self, handler: F) {
        self.item_toggled_handler.replace(Some(Box::new(handler)));
    }
}
