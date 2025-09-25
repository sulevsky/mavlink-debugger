use std::thread;
use std::time::Duration;

use crossterm::style::Stylize;
use mavlink::common::MavMessage;
use mavlink::error::MessageReadError;
use mavlink_debugger::cli::Args;
use mavlink_debugger::utils::mavlink::parse_status_text;

use clap::Parser;

fn main() {
    let args = Args::parse();
    let connection = mavlink::connect::<mavlink::common::MavMessage>(&args.address).unwrap();
    loop {
        match connection.recv_frame() {
            Ok(frame) => {
                if let MavMessage::STATUSTEXT(data) = frame.msg {
                    let severity = parse_severity(data.severity);
                    let text = parse_status_text(&data.text);
                    println!("{} {}", severity, text);
                }
            }
            Err(MessageReadError::Io(e)) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
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
}

fn parse_severity(severity: mavlink::common::MavSeverity) -> String {
    match severity {
        mavlink::common::MavSeverity::MAV_SEVERITY_EMERGENCY => "EMERGENCY".red(),
        mavlink::common::MavSeverity::MAV_SEVERITY_ALERT => "ALERT".red(),
        mavlink::common::MavSeverity::MAV_SEVERITY_CRITICAL => "CRITICAL".red(),
        mavlink::common::MavSeverity::MAV_SEVERITY_ERROR => "ERROR".red(),
        mavlink::common::MavSeverity::MAV_SEVERITY_WARNING => "WARNING".yellow(),
        mavlink::common::MavSeverity::MAV_SEVERITY_NOTICE => "NOTICE".yellow(),
        mavlink::common::MavSeverity::MAV_SEVERITY_INFO => "INFO".green(),
        mavlink::common::MavSeverity::MAV_SEVERITY_DEBUG => "DEBUG".white(),
    }
    .to_string()
}
