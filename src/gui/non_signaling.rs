use gtk::prelude::*;

/// Automates calling a widget's method with blocking the corresponding signal handler.
///
/// This struct purposefully does not support cloning (as it contains a signal handler ID).
///
pub struct NonSignalingWrapper<T: ObjectType> {
    widget: T,
    signal: Option<glib::SignalHandlerId>
}

impl<T: ObjectType> NonSignalingWrapper<T> {
    pub fn new(widget: T, signal: Option<glib::SignalHandlerId>) -> NonSignalingWrapper<T> {
        NonSignalingWrapper{ widget, signal }
    }

    pub fn do_without_signaling<F: Fn(&T)>(&self, action: F) {
        if let Some(signal) = &self.signal { self.widget.block_signal(signal); }
        action(&self.widget);
        if let Some(signal) = &self.signal { self.widget.unblock_signal(signal); }
    }

    pub fn get(&self) -> &T { &self.widget }

    pub fn set_signal(&mut self, signal: glib::SignalHandlerId) {
        self.signal = Some(signal);
    }
}

impl <T: ObjectType> Drop for NonSignalingWrapper<T> {
    fn drop(&mut self) {
        // disable the signal to avoid unwanted side effects during widget removal (e.g., if a spin button's text box
        // has focus and the widget is removed, the changed signal handler gets called)
        if let Some(signal) = &self.signal { self.widget.block_signal(signal) };
    }
}


// struct Ble<T: ObjectType> {
//     widget: T,
//     signal: glib::SignalHandlerId
// }

// impl<T: ObjectType> Ble<T> {
//     fn do_without_signaling<F: Fn(&T)>(&self, action: F) {
//         self.widget.block_signal(&self.signal);
//         action(&self.widget);
//         self.widget.unblock_signal(&self.signal);
//     }
// }

// fn foo() {
//     let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 10.0, 0.1);
//     let signal = slider.connect_value_changed(|_| {});

//     let b = Ble::<gtk::Scale>{
//         widget: slider,
//         signal
//     };

//     b.do_without_signaling(|slider| slider.set_value(2.3));
// }
