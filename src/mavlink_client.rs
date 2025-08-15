use mavlink::MavConnection;
use mavlink::common::{MavMessage, MavModeFlag};
use mavlink::{common::ATTITUDE_DATA, error::MessageReadError};
use std::sync::Mutex;
use std::{env, sync::Arc, thread, time::Duration};

use crossterm::event::{self, Event};
use ratatui::{Frame, text::Text};

use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::Vehicle;
use crate::cli::Args;

pub fn connect(args: &Args) -> Vehicle {
    let url = &args.address;

    // It's possible to change the mavlink dialect to be used in the connect call
    let mut vehicle = Vehicle {
        connection: None,
        is_armed: Arc::new(Mutex::new(false)),
        messages: Arc::new(Mutex::new(Vec::new())),
    };
    let connection = mavlink::connect::<mavlink::common::MavMessage>(&url.to_string()).ok();
    if connection.is_none() {
        return vehicle;
    }
    vehicle.connection = Some(Arc::new(connection.unwrap()));
    subscribe(&mut vehicle);

    vehicle
}
fn subscribe(vehicle: &mut Vehicle) {
    let connection = vehicle.connection.as_mut().unwrap().clone();
    let messages = vehicle.messages.clone();
    let vehicle_is_armed = vehicle.is_armed.clone();
    thread::spawn({
        move || loop {
            match connection.recv() {
                Ok((header, msg)) => {
                    messages.lock().unwrap().push(msg.clone());
                    if let mavlink::common::MavMessage::HEARTBEAT(data) = msg {
                        let is_armed = data
                            .base_mode
                            .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED);
                        let mut vehicle_is_armed = vehicle_is_armed.lock().unwrap();
                        *vehicle_is_armed = is_armed;
                    }
                }
                Err(MessageReadError::Io(e)) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        // println!("No messages");
                        //no messages currently available to receive -- wait a while
                        thread::sleep(Duration::from_secs(1));
                        continue;
                    } else {
                        println!("recv error: {e:?}");
                        break;
                    }
                }
                // messages that didn't get through due to parser errors are ignored
                _ => {}
            }
        }
    });
}
