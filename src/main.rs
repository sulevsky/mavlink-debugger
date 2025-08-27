mod cli;
mod mavlink_client;
mod utils;
use chrono::DateTime;
use chrono::Local;
use clap::Parser;
use mavlink::MavConnection;
use mavlink::MavFrame;
use ratatui::DefaultTerminal;
use ratatui::widgets::TableState;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;
use std::time::SystemTime;
use strum::Display;
use strum::EnumIter;
use strum::IntoEnumIterator;
use utils::mavlink::decode_param_id;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use crossterm::event::{self};
mod render;

use crate::cli::Args;
use crate::mavlink_client::request_mission_count;
use crate::mavlink_client::request_parameters;
use crate::mavlink_client::synchronise_mission_items;
use color_eyre::Result;
use mavlink::common::MISSION_ITEM_INT_DATA;
use mavlink::common::MavMessage;
use mavlink::common::MavModeFlag;
use mavlink::common::PARAM_VALUE_DATA;

#[derive(Default)]
struct MissionDetails {
    mission_messages: Vec<MISSION_ITEM_INT_DATA>,
    last_mission_request: Option<DateTime<Local>>,
    mission_items_to_load_num: Option<u16>,
}
#[derive(Default, Clone)]
struct TargetDetails {
    target_system_id: u8,
    target_component_id: u8,
}

#[derive(Default)]
struct Vehicle {
    messages: Vec<MavMessage>,
    parameter_messages: Vec<PARAM_VALUE_DATA>,
    is_armed: bool,
    last_parameters_request: Option<DateTime<Local>>,
    connection: Option<Arc<Box<dyn MavConnection<MavMessage> + Send + Sync>>>,
    target_details: Option<TargetDetails>,
    mission_details: Mutex<MissionDetails>,
}

enum AppEvent {
    Input(crossterm::event::Event),
    Mavlink(Box<MavFrame<MavMessage>>),
}

fn handle_input(tx: mpsc::Sender<AppEvent>) {
    thread::spawn(move || {
        loop {
            if let Ok(key_event) = event::read() {
                tx.send(AppEvent::Input(key_event)).unwrap();
            }
        }
    });
}

#[derive(Default, Display, EnumIter, PartialEq)]
enum Screen {
    #[default]
    Status,
    Messages,
    Parameters,
    Mission,
}
pub struct AppState {
    args: crate::cli::Args,

    vehicle: Vehicle,

    messages_table_state: TableState,
    parameters_table_state: TableState,
    mission_table_state: TableState,

    is_exit: bool,
    screen: Screen,
}
impl AppState {
    fn default(args: crate::cli::Args, vehicle: Vehicle) -> Self {
        AppState {
            args,
            vehicle,
            is_exit: false,
            messages_table_state: TableState::default().with_selected(Some(0)),
            parameters_table_state: TableState::default().with_selected(Some(0)),
            mission_table_state: TableState::default().with_selected(Some(0)),
            screen: Screen::Status,
        }
    }
    fn get_selected_message(&self) -> Option<MavMessage> {
        let selected_message_num = self.messages_table_state.selected();
        if let Some(index) = selected_message_num {
            self.vehicle.messages.get(index).cloned()
        } else {
            None
        }
    }
    fn get_selected_parameter(&self) -> Option<PARAM_VALUE_DATA> {
        let selected_parameter_num = self.parameters_table_state.selected();
        if let Some(index) = selected_parameter_num {
            self.vehicle.parameter_messages.get(index).cloned()
        } else {
            None
        }
    }
    fn clear_parameters(&mut self) {
        self.vehicle.parameter_messages.clear();
        self.vehicle.last_parameters_request = None;
        self.parameters_table_state.select_first();
    }
    fn clear_mission(&mut self) {
        let mut details = self.vehicle.mission_details.lock().unwrap();
        details.mission_messages.clear();
        details.last_mission_request = None;
        details.mission_items_to_load_num = None;
        self.mission_table_state.select_first();
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    color_eyre::install()?;
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
    handle_input(event_tx.clone());
    let mut terminal = ratatui::init();

    let vehicle = mavlink_client::connect(args.address.as_str(), event_tx.clone());
    let mut app_state = AppState::default(args, vehicle);

    let app_result = run(&mut app_state, &mut terminal, event_rx);
    ratatui::restore();
    app_result
}

fn run(
    app_state: &mut AppState,
    terminal: &mut DefaultTerminal,
    rx: mpsc::Receiver<AppEvent>,
) -> Result<()> {
    let mut fps_limiter = utils::tui::FPSLimiter::default(5);
    while !app_state.is_exit {
        let app_event = rx.recv()?;
        match app_event {
            AppEvent::Input(event) => {
                handle_input_event(app_state, event);
                match app_state.screen {
                    Screen::Status => {
                        terminal.draw(|frame| render::draw_status_screen(app_state, frame))?;
                    }
                    Screen::Messages => {
                        terminal.draw(|frame| render::draw_messages_screen(app_state, frame))?;
                    }
                    Screen::Parameters => {
                        if app_state.vehicle.last_parameters_request.is_none() {
                            request_parameters(&mut app_state.vehicle);
                            app_state.vehicle.last_parameters_request = Some(Local::now());
                        }
                        terminal.draw(|frame| render::draw_parameters_screen(app_state, frame))?;
                    }
                    Screen::Mission => {
                        if app_state
                            .vehicle
                            .mission_details
                            .lock()
                            .unwrap()
                            .last_mission_request
                            .is_none()
                        {
                            request_mission_count(&mut app_state.vehicle);
                            app_state
                                .vehicle
                                .mission_details
                                .lock()
                                .unwrap()
                                .last_mission_request = Some(Local::now());
                        }
                        terminal.draw(|frame| render::draw_mission_screen(app_state, frame))?;
                    }
                }
            }
            AppEvent::Mavlink(mav_frame) => {
                app_state.vehicle.messages.push(mav_frame.msg.clone());
                match mav_frame.msg {
                    mavlink::common::MavMessage::HEARTBEAT(data) => {
                        let is_armed = data
                            .base_mode
                            .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED);
                        app_state.vehicle.target_details = Some(TargetDetails {
                            target_system_id: mav_frame.header.system_id,
                            target_component_id: mav_frame.header.component_id,
                        });
                        app_state.vehicle.is_armed = is_armed;
                    }
                    mavlink::common::MavMessage::PARAM_VALUE(data) => {
                        app_state.vehicle.parameter_messages.push(data);
                        app_state
                            .vehicle
                            .parameter_messages
                            .sort_by_key(|d| decode_param_id(&d.param_id));
                    }
                    mavlink::common::MavMessage::MISSION_COUNT(data) => {
                        app_state
                            .vehicle
                            .mission_details
                            .lock()
                            .unwrap()
                            .mission_items_to_load_num = Some(data.count);
                        synchronise_mission_items(&app_state.vehicle);
                    }
                    mavlink::common::MavMessage::MISSION_ITEM_INT(data) => {
                        let mut mission_details = app_state.vehicle.mission_details.lock().unwrap();
                        mission_details.mission_messages.push(data);
                        mission_details.mission_messages.sort_by_key(|d| d.seq);
                    }

                    _ => {}
                }

                if fps_limiter.check_allowed(SystemTime::now()) {
                    match app_state.screen {
                        Screen::Status => {
                            terminal.draw(|frame| render::draw_status_screen(app_state, frame))?;
                        }
                        Screen::Messages => {
                            terminal
                                .draw(|frame| render::draw_messages_screen(app_state, frame))?;
                        }
                        Screen::Parameters => {
                            terminal
                                .draw(|frame| render::draw_parameters_screen(app_state, frame))?;
                        }
                        Screen::Mission => {
                            terminal.draw(|frame| render::draw_mission_screen(app_state, frame))?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_input_event(app_state: &mut AppState, event: Event) {
    if let Event::Key(key) = event {
        match key.code {
            KeyCode::Char(char) => match char {
                'q' => {
                    app_state.is_exit = true;
                }
                'c' => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        app_state.is_exit = true;
                    }
                }
                'j' => {
                    if let Some(s) = choose_list_state(app_state) {
                        s.select_next();
                    }
                }
                'k' => {
                    if let Some(s) = choose_list_state(app_state) {
                        s.select_previous();
                    }
                }
                'r' => match app_state.screen {
                    Screen::Parameters => {
                        app_state.clear_parameters();
                    }
                    Screen::Mission => {
                        app_state.clear_mission();
                    }
                    _ => {}
                },
                _ => {}
            },

            KeyCode::Esc => {
                app_state.is_exit = true;
            }
            KeyCode::Up => {
                if let Some(s) = choose_list_state(app_state) {
                    s.select_previous();
                }
            }
            KeyCode::Down => {
                if let Some(s) = choose_list_state(app_state) {
                    s.select_next();
                }
            }
            KeyCode::Home => {
                if let Some(s) = choose_list_state(app_state) {
                    s.select_first();
                }
            }
            KeyCode::End => {
                if let Some(s) = choose_list_state(app_state) {
                    s.select_last();
                }
            }
            KeyCode::PageUp => {
                if let Some(s) = choose_list_state(app_state) {
                    s.select(s.selected().map(|x| (x - 20).max(0)));
                }
            }

            KeyCode::PageDown => {
                let max_len_option = match app_state.screen {
                    Screen::Status => None,
                    Screen::Messages => Some(app_state.vehicle.messages.len()),
                    Screen::Parameters => Some(app_state.vehicle.parameter_messages.len()),
                    Screen::Mission => Some(
                        app_state
                            .vehicle
                            .mission_details
                            .lock()
                            .unwrap()
                            .mission_messages
                            .len(),
                    ),
                };
                if let Some(max_len) = max_len_option {
                    if let Some(s) = choose_list_state(app_state) {
                        s.select(s.selected().map(|x| (x + 20).min(max_len)));
                    }
                }
            }

            KeyCode::Tab => {
                let mut flag = false;
                for item in Screen::iter().cycle() {
                    if flag {
                        app_state.screen = item;
                        break;
                    }
                    if item == app_state.screen {
                        flag = true;
                    }
                }
            }
            _ => {}
        }
    }
}

fn choose_list_state(app_state: &mut AppState) -> Option<&mut TableState> {
    match app_state.screen {
        Screen::Status => None,
        Screen::Messages => Some(&mut app_state.messages_table_state),
        Screen::Parameters => Some(&mut app_state.parameters_table_state),
        Screen::Mission => Some(&mut app_state.mission_table_state),
    }
}
