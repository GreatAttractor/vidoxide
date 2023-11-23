use crate::ProgramData;
use crate::workers::controller::{ControllerToMainThreadMsg};
use std::cell::RefCell;
use std::rc::Rc;

pub fn on_controller_event(msg: ControllerToMainThreadMsg, program_data_rc: &Rc<RefCell<ProgramData>>) {
    println!("received {:?}", msg);
}
