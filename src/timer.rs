//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
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
            let handler = handler.take().unwrap();
            handler();
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
        let _guard = main_context.acquire().unwrap();
        let main_loop = glib::MainLoop::new(Some(&main_context), false);

        timer_no_update(&main_loop);
        timer_update_once(&main_loop);
        timer_update_twice(&main_loop);
        timer_stop(&main_loop);
        timer_update_from_handler(&main_loop);
    }

    fn ms(num_millis: u64) -> std::time::Duration {
        std::time::Duration::from_millis(num_millis)
    }

    fn not_run_handler() {
        panic!("This handler should not have run.");
    }

    #[must_use]
    fn loop_for(main_loop: &glib::MainLoop, duration: std::time::Duration) -> OneShotTimer {
        let timer_quit = OneShotTimer::new();
        timer_quit.run_once(duration, clone!(@strong main_loop => @default-panic, move || { main_loop.quit(); }));
        timer_quit
    }

    fn timer_no_update(main_loop: &glib::MainLoop) {
        let timer = OneShotTimer::new();
        let tstart = std::time::Instant::now();
        let handler_run = Rc::new(RefCell::new(false));

        timer.run_once(
            std::time::Duration::from_millis(20),
            clone!(@weak handler_run, @strong main_loop => @default-panic, move || {
                assert!(tstart.elapsed() > ms(19) && tstart.elapsed() < ms(21));
                handler_run.replace(true);
            })
        );

        let _q = loop_for(main_loop, ms(50));
        main_loop.run();

        assert!(*handler_run.borrow() == true);
    }

    fn timer_update_once(main_loop: &glib::MainLoop) {
        let timer = Rc::new(RefCell::new(OneShotTimer::new()));
        let timer_aux = OneShotTimer::new();
        let tstart = std::time::Instant::now();
        let handler_run = Rc::new(RefCell::new(false));

        timer.borrow_mut().run_once(ms(20), move || not_run_handler());

        timer_aux.run_once(ms(10), clone!(@weak timer, @weak handler_run => @default-panic, move || {
            timer.borrow_mut().run_once(
                ms(20),
                clone!(@weak handler_run => @default-panic, move || {
                    assert!(tstart.elapsed() > ms(29) && tstart.elapsed() < ms(31));
                    handler_run.replace(true);
                })
            );
        }));

        let _q = loop_for(main_loop, ms(50));
        main_loop.run();

        assert!(*handler_run.borrow() == true);
    }

    fn timer_update_twice(main_loop: &glib::MainLoop) {
        let timer = Rc::new(RefCell::new(OneShotTimer::new()));
        let timer_aux1 = OneShotTimer::new();
        let timer_aux2 = OneShotTimer::new();
        let tstart = std::time::Instant::now();
        let handler_run = Rc::new(RefCell::new(false));

        timer.borrow_mut().run_once(ms(20), move || not_run_handler());

        timer_aux1.run_once(ms(10), clone!(@weak timer, @weak handler_run => @default-panic, move || {
            timer.borrow_mut().run_once(ms(20), move || not_run_handler());
        }));

        timer_aux2.run_once(ms(20), clone!(@weak timer, @weak handler_run => @default-panic, move || {
            timer.borrow_mut().run_once(
                ms(20),
                clone!(@weak handler_run => @default-panic, move || {
                    assert!(tstart.elapsed() > ms(39) && tstart.elapsed() < ms(41));
                    handler_run.replace(true);
                })
            );
        }));

        let _q = loop_for(main_loop, ms(50));
        main_loop.run();

        assert!(*handler_run.borrow() == true);
    }

    fn timer_stop(main_loop: &glib::MainLoop) {
        let timer = Rc::new(RefCell::new(OneShotTimer::new()));
        timer.borrow_mut().run_once(ms(20), move || not_run_handler());

        let timer_aux = OneShotTimer::new();
        timer_aux.run_once(ms(10), clone!(@weak timer, @strong main_loop => @default-panic, move || {
            let t = timer.borrow();
            t.stop();
        }));

        let _q = loop_for(main_loop, ms(50));
        main_loop.run();
    }

    fn timer_update_from_handler(main_loop: &glib::MainLoop) {
        let handler_run = Rc::new(RefCell::new(false));
        let timer = Rc::new(RefCell::new(OneShotTimer::new()));

        timer.borrow_mut().run_once(ms(10), clone!(@weak timer, @weak handler_run => @default-panic, move || {
            timer.borrow_mut().run_once(
                ms(10),
                clone!(@weak handler_run => @default-panic, move || { handler_run.replace(true); })
            );
        }));

        let _q = loop_for(main_loop, ms(50));
        main_loop.run();

        assert!(*handler_run.borrow() == true);
    }
}
