//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
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

pub struct Timer {
    sender_main: std::sync::mpsc::Sender<Request>,
    handler: Rc<RefCell<Option<Box<dyn Fn() + 'static>>>>
}

struct Request {
    once: bool,
    delay: std::time::Duration
}

impl Timer {
    pub fn new() -> Timer {
        let handler: Rc<RefCell<Option<Box<dyn Fn() + 'static>>>> = Rc::new(RefCell::new(None));

        let (sender_timer, receiver_main) = glib::MainContext::channel::<()>(glib::PRIORITY_DEFAULT);
        receiver_main.attach(None, clone!(@weak handler => @default-panic, move |_| {
            let loc_handler = handler.take().unwrap();
            loc_handler();
            // restore `handler` only if `None`; otherwise it means the user has already reassigned `handler`
            // during `loc_handler` execution
            if handler.borrow().is_none() { *handler.borrow_mut() = Some(loc_handler); }
            return glib::Continue(true);
        }));

        let (sender_main, receiver_timer) = std::sync::mpsc::channel::<Request>();

        std::thread::spawn(move || {
            let mut request: Option<Request> = None;

            loop {
                let recv_result = match &request {
                    Some(req) => receiver_timer.recv_timeout(req.delay),
                    None => receiver_timer.recv_timeout(INFINITY)
                };

                match recv_result {
                    Ok(req) => request = Some(req),

                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        sender_timer.send(()).unwrap();
                        if let Some(req) = &request {
                            if req.once { request = None; }
                        }
                    },

                    _ => break
                }
            }
        });

        Timer{ sender_main, handler }
    }

    /// Runs provided `handler`; any previously scheduled runs will be cancelled.
    pub fn run<F: Fn() + 'static>(&self, delay: std::time::Duration, once: bool, handler: F) {
        self.handler.replace(Some(Box::new(handler)));
        self.sender_main.send(Request{ once, delay }).unwrap();
    }

    pub fn stop(&self) {
        self.handler.replace(None);
        self.sender_main.send(Request{ once: true, delay: INFINITY }).unwrap();
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
        timer_run_several_times(&main_loop);
    }

    fn ms(num_millis: u64) -> std::time::Duration {
        std::time::Duration::from_millis(num_millis)
    }

    fn not_run_handler() {
        panic!("This handler should not have run.");
    }

    #[must_use]
    fn loop_for(main_loop: &glib::MainLoop, duration: std::time::Duration) -> Timer {
        let timer_quit = Timer::new();
        timer_quit.run(duration, true, clone!(@strong main_loop => @default-panic, move || { main_loop.quit(); }));
        timer_quit
    }

    fn timer_no_update(main_loop: &glib::MainLoop) {
        let timer = Timer::new();
        let tstart = std::time::Instant::now();
        let handler_run = Rc::new(RefCell::new(false));

        timer.run(
            std::time::Duration::from_millis(20),
            true,
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
        let timer = Rc::new(Timer::new());
        let timer_aux = Timer::new();
        let tstart = std::time::Instant::now();
        let handler_run = Rc::new(RefCell::new(false));

        timer.run(ms(20), true, move || not_run_handler());

        timer_aux.run(ms(10), true, clone!(@weak timer, @weak handler_run => @default-panic, move || {
            timer.run(
                ms(20),
                true,
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
        let timer = Rc::new(Timer::new());
        let timer_aux1 = Timer::new();
        let timer_aux2 = Timer::new();
        let tstart = std::time::Instant::now();
        let handler_run = Rc::new(RefCell::new(false));

        timer.run(ms(20), true, move || not_run_handler());

        timer_aux1.run(ms(10), true, clone!(@weak timer, @weak handler_run => @default-panic, move || {
            timer.run(ms(20), true, move || not_run_handler());
        }));

        timer_aux2.run(ms(20), true, clone!(@weak timer, @weak handler_run => @default-panic, move || {
            timer.run(
                ms(20),
                true,
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
        let timer = Rc::new(Timer::new());
        timer.run(ms(20), true, move || not_run_handler());

        let timer_aux = Timer::new();
        timer_aux.run(ms(10), true, clone!(@weak timer, @strong main_loop => @default-panic, move || {
            timer.stop();
        }));

        let _q = loop_for(main_loop, ms(50));
        main_loop.run();
    }

    fn timer_update_from_handler(main_loop: &glib::MainLoop) {
        let handler_run = Rc::new(RefCell::new(false));
        let timer = Rc::new(Timer::new());

        timer.run(ms(10), true, clone!(@weak timer, @weak handler_run => @default-panic, move || {
            timer.run(
                ms(10),
                true,
                clone!(@weak handler_run => @default-panic, move || { handler_run.replace(true); })
            );
        }));

        let _q = loop_for(main_loop, ms(50));
        main_loop.run();

        assert!(*handler_run.borrow() == true);
    }

    fn timer_run_several_times(main_loop: &glib::MainLoop) {
        let handler_run = Rc::new(RefCell::new(Vec::<std::time::Instant>::new()));
        let timer = Rc::new(Timer::new());

        let t_start = std::time::Instant::now();

        timer.run(ms(10), false, clone!(@weak timer, @weak handler_run => @default-panic, move || {
            handler_run.borrow_mut().push(std::time::Instant::now());
        }));

        let _q = loop_for(main_loop, ms(60));
        main_loop.run();

        let handler_run = handler_run.borrow();
        assert_eq!(5, handler_run.len());
        let check_if_around = |exp, act, range| { assert!(act >= exp - range && act <= exp + range); };
        for i in 0..5 {
            check_if_around(t_start + (i + 1) * ms(10), handler_run[i as usize], ms(2));
        }
    }
}
