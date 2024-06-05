//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Device driver utilities.
//!

use std::error::Error;

#[derive(Debug)]
pub enum ResponseType {
    None,
    EndsWith(char),
    NumCharsReceived(usize),
    CharsReceived(String)
}

pub enum InvalidResponseTreatment {
    Fail,
    Ignore{ log_warning: bool }
}

pub fn send_cmd_and_get_reply<T: std::io::Read + std::io::Write>(
    device: &mut T,
    cmd: String,
    response_type: ResponseType,
    on_invalid_resp: InvalidResponseTreatment
) -> Result<Vec<u8>, Box<dyn Error>> {
    device.write_all(&cmd.clone().into_bytes())?;

    match &response_type {
        ResponseType::CharsReceived(chars) => { if chars.is_empty() { return Ok(vec![]); } },
        ResponseType::NumCharsReceived(0) | ResponseType::None => { return Ok(vec![]); }
        _ => ()
    }

    let mut reply_error = false;

    let mut buf = vec![];
    let mut reply_received = false;
    while !reply_received {
        buf.push(0);
        if buf.len() > 1024 { return Err("response has too many characters".into()); }
        let blen = buf.len();
        if device.read_exact(&mut buf[blen - 1..blen]).is_err() {
            reply_error = true;
            break;
        }
        reply_received = match response_type {
            ResponseType::EndsWith(ch) => buf[blen - 1] == ch as u8,
            ResponseType::NumCharsReceived(num) => buf.len() == num,
            ResponseType::CharsReceived(ref chars) => buf.len() == chars.len(),
            ResponseType::None => unreachable!()
        };
    }

    if let ResponseType::CharsReceived(chars) = &response_type {
        if &buf != chars.as_bytes() { reply_error = true; }
    }

    if reply_error {
        let message = format!("cmd \"{}\" failed to get expected response: {:?}", cmd, response_type);
        match on_invalid_resp {
            InvalidResponseTreatment::Fail => return Err(message.into()),
            InvalidResponseTreatment::Ignore{ log_warning } => if log_warning { log::warn!("{}", message); }
        }
    }

    Ok(buf)
}
