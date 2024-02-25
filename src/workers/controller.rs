//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Game controller input thread.
//!

#[derive(Debug)]
pub struct NewDevice {
    /// Device-model-specific identifier.
    pub id: u64,
    /// Device index (among the currently connected).
    pub index: usize,
    pub name: String
}

#[derive(Debug)]
pub struct StickEvent {
    pub id: u64,
    pub index: usize,
    pub event: stick::Event
}

#[derive(Debug)]
pub enum ControllerToMainThreadMsg {
    NewDevice(NewDevice),
    StickEvent(StickEvent)
}

struct State {
    listener: stick::Listener,
    controllers: Vec<stick::Controller>,
    sender: glib::Sender<ControllerToMainThreadMsg>
}

type Exit = usize;

impl State {
    fn on_connect(&mut self, controller: stick::Controller) -> std::task::Poll<Exit> {
        self.sender.send(ControllerToMainThreadMsg::NewDevice(NewDevice{
            id: controller.id(),
            index: self.controllers.len(),
            name: controller.name().into()
        })).unwrap();

        self.controllers.push(controller);

        std::task::Poll::Pending
    }

    fn on_stick_event(&mut self, index: usize, event: stick::Event) -> std::task::Poll<Exit> {
        let id = self.controllers[index].id();
        self.sender.send(ControllerToMainThreadMsg::StickEvent(StickEvent{ id, index, event })).unwrap();
        if let stick::Event::Disconnect = event { self.controllers.remove(index); }

        std::task::Poll::Pending
    }
}

async fn event_loop(sender: glib::Sender<ControllerToMainThreadMsg>) {
    let mut state = State{
        sender,
        listener: stick::Listener::default(),
        controllers: Vec::new()
    };

    pasts::Loop::new(&mut state)
        .when(|s| &mut s.listener, State::on_connect)
        .poll(|s| &mut s.controllers, State::on_stick_event)
        .await;
}

pub fn controller_thread(sender: glib::Sender<ControllerToMainThreadMsg>) {
    pasts::block_on(event_loop(sender));
}
