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

use glib::clone;
use std::cell::RefCell;
use std::rc::Rc;

const INFINITY: std::time::Duration = std::time::Duration::from_secs(9_999_999_999);

pub struct OneShotTimer {
    sender_main: std::sync::mpsc::Sender<std::time::Instant>,
    handler: Rc<RefCell<Option<Box<dyn Fn() + 'static>>>>
}

impl OneShotTimer {
    pub fn new() -> OneShotTimer {
        let handler: Rc<RefCell<Option<Box<dyn Fn() + 'static>>>> = Rc::new(RefCell::new(None));

        let (sender_timer, receiver_main) = glib::MainContext::channel::<()>(glib::PRIORITY_DEFAULT);
        receiver_main.attach(None, clone!(@weak handler => @default-panic, move |_| {
            (*(*handler).borrow().as_ref().unwrap())();
            return glib::Continue(true);
        }));

        let (sender_main, receiver_timer) = std::sync::mpsc::channel::<std::time::Instant>();

        std::thread::spawn(move || {
            let mut target_time: Option<std::time::Instant> = None;

            loop {
                let recv_result = match &target_time {
                    Some(t) => {
                        let now = std::time::Instant::now();
                        if *t > now {
                            receiver_timer.recv_timeout(*t - now)
                        } else {
                            receiver_timer.recv_timeout(INFINITY)
                        }
                    },
                    None => receiver_timer.recv_timeout(INFINITY)
                };

                match recv_result {
                    Ok(new_target_time) => target_time = Some(new_target_time),

                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        sender_timer.send(()).unwrap();
                        target_time = None;
                    },

                    _ => break
                }
            }
        });

        OneShotTimer{ sender_main, handler }
    }

    /// Runs provided `handler` once after `delay`; any previously scheduled runs will be cancelled.
    pub fn run_once<F: Fn() + 'static>(&self, delay: std::time::Duration, handler: F) {
        self.handler.replace(Some(Box::new(handler)));
        self.sender_main.send(std::time::Instant::now() + delay).unwrap();
    }

    pub fn stop(&self) {
        self.handler.replace(None);
        self.sender_main.send(std::time::Instant::now() + INFINITY).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suite() {
        let main_context = glib::MainContext::default();
        let guard = main_context.acquire().unwrap();

        timer_no_update();
        timer_update_once();
        timer_update_twice();
    }

    fn ms(num_millis: u64) -> std::time::Duration {
        std::time::Duration::from_millis(num_millis)
    }

    fn not_run_handler() {
        panic!("This handler should not have run.");
    }

    fn timer_no_update() {
        let timer = OneShotTimer::new();

        let tstart = std::time::Instant::now();

        timer.run_once(
            std::time::Duration::from_millis(200),
            move || { assert!(tstart.elapsed() > ms(190) && tstart.elapsed() < ms(210)) }
        );
    }

    fn timer_update_once() {
        let timer = OneShotTimer::new();

        let tstart = std::time::Instant::now();

        timer.run_once(ms(200), move || not_run_handler());

        std::thread::sleep(ms(100));

        timer.run_once(
            ms(200),
            move || { assert!(tstart.elapsed() > ms(290) && tstart.elapsed() < ms(310)) }
        )
    }

    fn timer_update_twice() {
        let timer = OneShotTimer::new();

        let tstart = std::time::Instant::now();

        timer.run_once(ms(200), move || not_run_handler());

        std::thread::sleep(ms(100));

        timer.run_once(ms(200), move || not_run_handler());

        std::thread::sleep(ms(100));

        timer.run_once(
            ms(200),
            move || { assert!(tstart.elapsed() > ms(390) && tstart.elapsed() < ms(410)) }
        );
    }

    fn timer_stop() {
        let timer = OneShotTimer::new();

        timer.run_once(ms(200), move || not_run_handler());

        std::thread::sleep(ms(100));

        timer.stop();
    }
}
