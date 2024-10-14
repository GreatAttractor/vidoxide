//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Limited-frequency action.
//!

use crate::timer::Timer;
use glib::clone;
use std::{cell::RefCell, rc::Rc};

struct State<E> {
    handler: Box<dyn Fn(&E) + 'static>,
    latest_event: Option<E>,
    timer_triggered: bool
}

pub struct LimitedFreqAction<E> {
    interval: std::time::Duration,
    timer: Timer,
    state: Rc<RefCell<State<E>>>

}

impl<E: 'static> LimitedFreqAction<E> {
    pub fn new(interval: std::time::Duration, handler: Box<dyn Fn(&E) + 'static>) -> LimitedFreqAction<E> {
        LimitedFreqAction{
            interval,
            timer: Timer::new(),
            state: Rc::new(RefCell::new(State{
                handler,
                latest_event: None,
                timer_triggered: false
            }))
        }
    }

    pub fn process(&mut self, event: E) {
        if self.state.borrow().timer_triggered {
            self.state.borrow_mut().latest_event = Some(event);
        } else {
            (self.state.borrow().handler)(&event);
            self.timer.run(
                self.interval,
                false,
                clone!(@strong self.state as state => @default-panic, move || {
                    let latest_evt = state.borrow_mut().latest_event.take();
                    if let Some(latest_evt) = latest_evt {
                        (state.borrow().handler)(&latest_evt);
                    } else {
                        state.borrow_mut().timer_triggered = false;
                        // no need to stop the timer; just setting `timer_triggered` to `false` will cause
                        // us to restart the timer on the next event
                    }
                })
            );
            self.state.borrow_mut().timer_triggered = true;
        }
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
        let timer_quit = Timer::new();
        timer_quit.run(
            std::time::Duration::from_millis(500),
            true,
            clone!(@strong main_loop => @default-panic, move || { main_loop.quit(); })
        );

        let handled_events = Rc::new(RefCell::new(vec![]));
        let lfa = Rc::new(RefCell::new(LimitedFreqAction::new(
            std::time::Duration::from_millis(100),
            Box::new(clone!(@strong handled_events => @default-panic, move |event: &i32| {
                handled_events.borrow_mut().push(*event);
            })),
        )));

        macro_rules! inject_at_t { ($t:expr, $event:expr) => {
            let t_event_src = Timer::new();
            t_event_src.run(
                $t,
                true,
                clone!(@strong lfa => @default-panic, move || {
                    for e in $event {
                        lfa.borrow_mut().process(*e);
                    }
                })
            );
        } }

        //
        // Events' timeline (LimitedFreqAction's interval set to 100 ms):
        //
        // t (ms):     0         100       200       300  310    410
        //             |         |         |         |    |      |
        // event:      0 1 2          3 4            |    5  6
        // handled:    *   *            *            |    *  *
        //                                           |
        //                           nothing received nor handled
        //                           during the last interval,
        //                           so the next event (5) begins the timing
        //                           of a new interval
        //

        inject_at_t!(std::time::Duration::ZERO, &[0, 1, 2]);
        inject_at_t!(std::time::Duration::from_millis(150), &[3, 4]);
        inject_at_t!(std::time::Duration::from_millis(310), &[5, 6]);

        main_loop.run();

        assert_eq!(vec![0, 2, 4, 5, 6], *handled_events.borrow());
    }
}
