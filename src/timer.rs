//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Timer.
//!

use std::cell::RefCell;
use std::rc::Rc;

pub struct OneShotTimer {
    request_id: Rc<RefCell<u64>>
}

impl OneShotTimer {
    pub fn new() -> OneShotTimer {
        OneShotTimer{ request_id: Rc::new(RefCell::new(0)) }
    }

    pub fn run_once<F: Fn() + 'static>(&mut self, delay: std::time::Duration, handler: F) {
        self.stop();

        let current_id: u64 = *self.request_id.borrow();
        let request_id_clone = self.request_id.clone();
        let (sender_timer, receiver_main) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        receiver_main.attach(None, move |_| {
            if current_id == *request_id_clone.borrow() {
                handler();
            }
            return glib::Continue(true);
        });

        std::thread::spawn(move || {
            std::thread::sleep(delay);
            let _ = sender_timer.send(());
        });
    }

    pub fn stop(&mut self) {
        *self.request_id.borrow_mut() += 1;
    }
}
