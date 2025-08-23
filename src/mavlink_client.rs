use std::sync::mpsc;
use std::{sync::Arc, thread, time::Duration};

use mavlink::error::MessageReadError;

use crate::cli::Args;
use crate::{AppEvent, Vehicle};

pub fn connect(args: &Args, tx: mpsc::Sender<AppEvent>) -> Vehicle {
    let url = &args.address;

    // It's possible to change the mavlink dialect to be used in the connect call
    let mut vehicle = Vehicle {
        connection: None,
        is_armed: false,
        messages: Vec::new(),
        parameter_messages: Vec::new(),
        last_parameters_request: None,
    };
    let connection = mavlink::connect::<mavlink::common::MavMessage>(&url.to_string()).ok();
    if connection.is_none() {
        return vehicle;
    }
    vehicle.connection = Some(Arc::new(connection.unwrap()));
    subscribe(&mut vehicle, tx);

    vehicle
}
fn subscribe(vehicle: &mut Vehicle, tx: mpsc::Sender<AppEvent>) {
    let connection = vehicle.connection.as_mut().unwrap().clone();
    thread::spawn({
        move || loop {
            match connection.recv() {
                Ok((_, msg)) => tx.send(AppEvent::Mavlink(Box::new(msg))).unwrap(),
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
pub fn request_parameters(vehicle: &mut Vehicle) {
    let connection = vehicle.connection.as_mut().unwrap().clone();
    thread::spawn({
        move || {
            let param_request_list_message = mavlink::common::MavMessage::PARAM_REQUEST_LIST(
                mavlink::common::PARAM_REQUEST_LIST_DATA {
                    target_system: 1,
                    target_component: 1,
                },
            );
            connection
                .send_default(&param_request_list_message)
                .unwrap();
        }
    });
}
