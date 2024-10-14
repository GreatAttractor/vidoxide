//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! DreamFocuser mini Bluetooth executor.
//!

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform as bt_platform;
use crate::devices::{
    focuser::{DegC, dream_focuser_mini::{CmdExecutor, Command, Position, Speed, State, to_raw_speed}},
    utils
};
use std::{convert::TryInto, error::Error, rc::Rc};

const SCAN_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

mod uuids {
    pub mod services {
        use uuid::{uuid, Uuid};

        pub const MAIN: Uuid = uuid!("0000f00d-1212-efde-1523-785fef13d123");
        pub const TEMP_HUM: Uuid = uuid!("0000f001-1212-efde-1523-785fef13d123");
    }

    pub mod characteristics {
        use uuid::{uuid, Uuid};

        pub const MOVE_STOP: Uuid = uuid!("00000003-1212-efde-1523-785fef13d123");
        pub const GET_POS: Uuid = uuid!("00000002-1212-efde-1523-785fef13d123");
        pub const TEMP_HUM: Uuid = uuid!("00000001-1212-efde-1523-785fef13d123");
    }
}

enum Request {
    Move { raw_speed: i16 },
    Stop,
    GetPos,
    GetTempHum,
    Disconnect
}

enum Response {
    None,
    GetPos(Position),
    GetTempHum { temp: DegC, hum_percent: f32 }
}

pub struct BluetoothExecutor {
    sender: tokio::sync::mpsc::Sender<Request>,
    receiver: tokio::sync::mpsc::Receiver<Result<Response, Box<dyn Error + Sync + Send>>>,
}

impl Drop for BluetoothExecutor {
    fn drop(&mut self) {
        let _ = self.sender.blocking_send(Request::Disconnect);
    }
}

impl BluetoothExecutor {
    pub fn new(mac_addr: &str, tokio_rt: Rc<tokio::runtime::Runtime>) -> Result<Box<dyn CmdExecutor>, Box<dyn Error>> {
        let (device, ch_move_stop, ch_get_pos, ch_temp_hum) = tokio_rt.block_on(async {
            let manager = bt_platform::Manager::new().await?;

            // get the first bluetooth adapter
            let adapters = manager.adapters().await?;
            let central = adapters.into_iter().nth(0).ok_or::<Box<dyn Error>>("no adapters found".into())?;

            central.start_scan(ScanFilter::default()).await?;

            tokio::time::sleep(SCAN_DURATION).await;

            let mut device = None;

            for p in central.peripherals().await? {
                if p.properties()
                    .await?
                    .ok_or::<Box<dyn Error>>("no properties found".into())?
                    .address.to_string() ==  mac_addr.to_string() {
                    device = Some(p);
                    break;
                }
            }

            let device = device.ok_or(format!("device {} not found", mac_addr))?;

            let t_start = std::time::Instant::now();
            device.connect().await?;
            log::info!("connected in {:.01} ms", t_start.elapsed().as_secs_f64() * 1000.0);

            device.discover_services().await?;
            let chs = device.characteristics();

            let ch_move_stop = chs.iter().find(
                |ch| ch.service_uuid == uuids::services::MAIN && ch.uuid == uuids::characteristics::MOVE_STOP
            ).ok_or::<Box<dyn Error>>("move/stop characteristic not found".into())?.clone();

            let ch_get_pos = chs.iter().find(
                |ch| ch.service_uuid == uuids::services::MAIN && ch.uuid == uuids::characteristics::GET_POS
            ).ok_or::<Box<dyn Error>>("get_pos characteristic not found".into())?.clone();

            let ch_temp_hum = chs.iter().find(
                |ch| ch.service_uuid == uuids::services::TEMP_HUM && ch.uuid == uuids::characteristics::TEMP_HUM
            ).ok_or::<Box<dyn Error>>("temp./hum. characteristic not found".into())?.clone();

            Result::<_, Box<dyn Error>>::Ok((device, ch_move_stop, ch_get_pos, ch_temp_hum))
        })?;


        let (req_send, req_recv) = tokio::sync::mpsc::channel::<Request>(1);
        let (resp_send, resp_recv) = tokio::sync::mpsc::channel::<Result<Response, Box<dyn Error + Sync + Send>>>(1);

        tokio_rt.spawn(communication_task(device, ch_move_stop, ch_get_pos, ch_temp_hum, req_recv, resp_send));

        Ok(Box::new(BluetoothExecutor{ sender: req_send, receiver: resp_recv }))
    }
}

impl CmdExecutor for BluetoothExecutor {
    // TODO: use proper target position handling
    fn move_(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            self.stop()
        } else {
            let mut raw_speed = to_raw_speed(speed);
            if target.0 < 0 { raw_speed = -raw_speed; }
            self.sender.blocking_send(Request::Move { raw_speed })?;
            self.receiver.blocking_recv().unwrap().map(|_| ()).map_err(|e| e.to_string().into())
        }
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.sender.blocking_send(Request::Stop)?;
        self.receiver.blocking_recv().unwrap().map(|_| ()).map_err(|e| e.to_string().into())
    }

    fn state(&mut self) -> Result<State, Box<dyn Error>> {
        Err("unsupported".into())

        // TODO this needs rethinking, getting pos+temp+hum like below takes hundreds of ms, GUI becomes unresponsive.
        // Check if activating and reading the 1-s temp+hum notifications works better.

        // self.sender.blocking_send(Request::GetPos)?;
        // let pos;
        // if let Response::GetPos(p) = self.receiver.blocking_recv().unwrap().map_err(|e| e.to_string())? {
        //     pos = p;
        // } else {
        //     panic!("unexpected response");
        // }

        // self.sender.blocking_send(Request::GetTempHum)?;
        // let t;
        // if let Response::GetTempHum { temp, hum_percent } = self.receiver.blocking_recv().unwrap().map_err(|e| e.to_string())? {
        //     log::info!("temp. {:.02}°C, hum. {:.02}%", temp.0, hum_percent);
        //     t = Some(temp);
        // } else {
        //     panic!("unexpected response");
        // }

        // Ok(State{ pos, moving: Some(false), temperature: t })
    }
}

fn to_temp_hum(payload: &[u8; 4]) -> Response {
    Response::GetTempHum{
        temp: DegC((((payload[0] as i16) << 8) + payload[1] as i16) as f32 / 100.0),
        hum_percent: (((payload[2] as i16) << 8) + payload[3] as i16) as f32 / 100.0
    }
}

fn to_pos(payload: &[u8; 4]) -> Response {
    Response::GetPos(Position(i32::from_le_bytes(*payload)))
}

async fn communication_task(
    device: btleplug::platform::Peripheral,
    ch_move_stop: btleplug::api::Characteristic,
    ch_get_pos: btleplug::api::Characteristic,
    ch_temp_hum: btleplug::api::Characteristic,
    mut req_recv: tokio::sync::mpsc::Receiver<Request>,
    resp_send: tokio::sync::mpsc::Sender<Result<Response, Box<dyn Error + Sync + Send>>>
) {
    loop {
        match req_recv.recv().await {
            None => break,
            Some(req) => match req {
                Request::Move { raw_speed } => {
                    // TODO extract these to a function
                    let result: Result<Response, Box<dyn Error + Sync + Send>> = device.write(
                        &ch_move_stop,
                        &[Command::Move.opcode(), (raw_speed & 0xFF) as u8, (raw_speed >> 8) as u8],
                        WriteType::WithoutResponse
                    ).await.map(|_| Response::None).map_err(|e| e.into());

                    resp_send.send(result).await.unwrap();
                },

                Request::Stop => {
                    let result: Result<Response, Box<dyn Error + Sync + Send>> = device.write(
                        &ch_move_stop,
                        &[Command::Stop.opcode()],
                        WriteType::WithoutResponse
                    ).await.map(|_| Response::None).map_err(|e| e.into());

                    resp_send.send(result).await.unwrap();
                },

                Request::GetPos => {
                    let result = match device.read(&ch_get_pos).await {
                        Ok(bytes) => match TryInto::<&[u8; 4]>::try_into(bytes.as_slice()) {
                            Ok(payload) => Ok(to_pos(payload)),
                            Err(_) => Err(format!("invalid response: {:?}", bytes).into())
                        },
                        Err(e) => Err(e.to_string().into())
                    };

                   resp_send.send(result).await.unwrap();
                },

                Request::GetTempHum => {
                    let result = match device.read(&ch_temp_hum).await {
                        Ok(bytes) => match TryInto::<&[u8; 4]>::try_into(bytes.as_slice()) {
                            Ok(payload) => Ok(to_temp_hum(payload)),
                            Err(_) => Err(format!("invalid response: {:?}", bytes).into())
                        },
                        Err(e) => Err(e.to_string().into())
                    };

                   resp_send.send(result).await.unwrap();
                },

                Request::Disconnect => break
            }
        }
    }
}
