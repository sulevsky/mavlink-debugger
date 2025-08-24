use std::sync::mpsc;
use std::{sync::Arc, thread, time::Duration};

use mavlink::common::MavMessage;
use mavlink::error::MessageReadError;

use crate::{AppEvent, Vehicle};

pub fn connect(address: &str, tx: mpsc::Sender<AppEvent>) -> Vehicle {
    let mut vehicle = Vehicle {
        messages: Vec::new(),
        connection: None,
        is_armed: false,
        parameter_messages: Vec::new(),
        last_parameters_request: None,
        mission_messages: Vec::new(),
        last_mission_request: None,
    };
    let connection = mavlink::connect::<mavlink::common::MavMessage>(address).ok();
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
    let param_request_list_message =
        mavlink::common::MavMessage::PARAM_REQUEST_LIST(mavlink::common::PARAM_REQUEST_LIST_DATA {
            target_system: 1,
            target_component: 1,
        });
    send_message(vehicle, param_request_list_message);
}

pub fn request_mission(vehicle: &mut Vehicle) {
    let mission_request_list_message = mavlink::common::MavMessage::MISSION_REQUEST_LIST(
        mavlink::common::MISSION_REQUEST_LIST_DATA {
            target_system: 1,
            target_component: 1,
        },
    );
    // TODO vova sedn for count, save count, request tiems
    send_message(vehicle, mission_request_list_message);
    for i in 0..7 {
        let mission_request_int_message = mavlink::common::MavMessage::MISSION_REQUEST_INT(
            mavlink::common::MISSION_REQUEST_INT_DATA {
                target_system: 1,
                target_component: 1,
                seq: i,
            },
        );
        send_message(vehicle, mission_request_int_message);
    }
}

fn send_message(vehicle: &mut Vehicle, message: MavMessage) {
    let connection = vehicle.connection.as_mut().unwrap().clone();
    connection.send_default(&message).unwrap();
}
