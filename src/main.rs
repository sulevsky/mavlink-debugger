mod cli;
mod mavlink_client;
mod utils;
use chrono::{DateTime, Local};
use clap::Parser;
use mavlink::{MavConnection, Message};
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem, ListState, Padding, Tabs, Widget, Wrap};
use std::sync::mpsc;
use std::{sync::Arc, thread};
use strum::{Display, EnumIter, IntoEnumIterator};
use utils::mavlink::decode_param_id;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Frame;

use crate::cli::Args;
use crate::mavlink_client::request_parameters;
use color_eyre::Result;
use mavlink::common::MavModeFlag;
use mavlink::common::{MavMessage, PARAM_VALUE_DATA};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

struct Vehicle {
    messages: Vec<MavMessage>,
    parameter_messages: Vec<PARAM_VALUE_DATA>,
    last_parameters_request: Option<DateTime<Local>>,
    connection: Option<Arc<Box<dyn MavConnection<MavMessage> + Send + Sync>>>,
    is_armed: bool,
}
fn main() -> Result<()> {
    let args = Args::parse();

    color_eyre::install()?;
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
    handle_input(event_tx.clone());
    let mut terminal = ratatui::init();

    let vehicle = mavlink_client::connect(&args, event_tx.clone());
    let mut app_state = AppState::default(args, vehicle);

    let app_result = run(&mut app_state, &mut terminal, event_rx);
    ratatui::restore();
    app_result
}

enum AppEvent {
    Input(crossterm::event::Event),
    Mavlink(MavMessage),
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

    messages_list_state: ListState,
    parameters_list_state: ListState,

    is_exit: bool,
    screen: Screen,
}
impl AppState {
    fn default(args: crate::cli::Args, vehicle: Vehicle) -> Self {
        AppState {
            args,
            vehicle,
            is_exit: false,
            messages_list_state: ListState::default().with_selected(Some(0)),
            parameters_list_state: ListState::default().with_selected(Some(0)),
            screen: Screen::Status,
        }
    }
    fn get_selected_message(&self) -> Option<MavMessage> {
        let selected_message_num = self.messages_list_state.selected();
        if let Some(index) = selected_message_num {
            return self.vehicle.messages.get(index).cloned();
        } else {
            return None;
        }
    }
    fn get_selected_parameter(&self) -> Option<PARAM_VALUE_DATA> {
        let selected_parameter_num = self.parameters_list_state.selected();
        if let Some(index) = selected_parameter_num {
            return self.vehicle.parameter_messages.get(index).cloned();
        } else {
            return None;
        }
    }
}

fn run(
    mut app_state: &mut AppState,
    terminal: &mut DefaultTerminal,
    rx: mpsc::Receiver<AppEvent>,
) -> Result<()> {
    let mut fps_limiter = utils::tui::FPSLimiter::default(50);
    while !app_state.is_exit {
        let app_event = rx.recv()?;
        match app_event {
            AppEvent::Input(event) => {
                handle_input_event(&mut app_state, event);
                match app_state.screen {
                    Screen::Status => {
                        terminal.draw(|frame| draw_status_screen(&mut app_state, frame))?;
                    }
                    Screen::Messages => {
                        terminal.draw(|frame| draw_messages_screen(&mut app_state, frame))?;
                    }
                    Screen::Parameters => {
                        if app_state.vehicle.last_parameters_request.is_none() {
                            request_parameters(&mut app_state.vehicle);
                            app_state.vehicle.last_parameters_request = Some(Local::now());
                        }
                        terminal.draw(|frame| draw_parameters_screen(&mut app_state, frame))?;
                    }
                    Screen::Mission => {
                        terminal.draw(|frame| draw_messages_screen(&mut app_state, frame))?;
                    }
                }
            }
            AppEvent::Mavlink(mav_message) => {
                app_state.vehicle.messages.push(mav_message.clone());
                match mav_message {
                    mavlink::common::MavMessage::HEARTBEAT(data) => {
                        let is_armed = data
                            .base_mode
                            .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED);
                        app_state.vehicle.is_armed = is_armed;
                    }
                    mavlink::common::MavMessage::PARAM_VALUE(data) => {
                        app_state.vehicle.parameter_messages.push(data);
                        app_state
                            .vehicle
                            .parameter_messages
                            .sort_by_key(|d| decode_param_id(&d.param_id));
                    }
                    _ => {}
                }

                if fps_limiter.check_allowed() {
                    match app_state.screen {
                        Screen::Status => {
                            terminal.draw(|frame| draw_status_screen(&mut app_state, frame))?;
                        }
                        Screen::Messages => {
                            terminal.draw(|frame| draw_messages_screen(&mut app_state, frame))?;
                        }
                        Screen::Parameters => {
                            terminal.draw(|frame| draw_parameters_screen(&mut app_state, frame))?;
                        }
                        Screen::Mission => {
                            terminal.draw(|frame| draw_messages_screen(&mut app_state, frame))?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn draw_status_screen(app_state: &mut AppState, frame: &mut Frame) {
    let area = frame.area();
    let [tab_header, tab_content] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    draw_tabs(tab_header, app_state, frame);

    Block::bordered()
        .border_type(ratatui::widgets::BorderType::Thick)
        .render(tab_content, frame.buffer_mut());

    let [headear_area, _events_area, _help_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .margin(1)
    .areas(tab_content);
    let [connection_area, armed_area] =
        Layout::horizontal([Constraint::Length(50), Constraint::Max(14)]).areas(headear_area);

    Paragraph::new(Line::from(vec![
        Span::from(" Address: "),
        Span::from(app_state.args.address.to_string()),
        if app_state.vehicle.connection.is_some() {
            Span::from(" connected ").green()
        } else {
            Span::from(" not connected ").red()
        },
    ]))
    .block(Block::bordered().title(" Connection ".bold()))
    .render(connection_area, frame.buffer_mut());

    Paragraph::new(if app_state.vehicle.connection.is_none() {
        Span::from("Unknown").gray()
    } else {
        if app_state.vehicle.is_armed {
            Span::from(" Armed ").green()
        } else {
            Span::from(" Disarmed ").red()
        }
    })
    .block(Block::bordered().title(" Arm status ".bold()))
    .centered()
    .render(armed_area, frame.buffer_mut());
}

fn draw_messages_screen(app_state: &mut AppState, frame: &mut Frame) {
    let area = frame.area();
    let [tab_header, tab_content] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    draw_tabs(tab_header, app_state, frame);

    Block::bordered()
        .border_type(ratatui::widgets::BorderType::Thick)
        .render(tab_content, frame.buffer_mut());

    let [events_area, help_area] = Layout::vertical([Constraint::Fill(1), Constraint::Length(3)])
        .margin(1)
        .areas(tab_content);

    let [list_events_area, details_events_area] =
        Layout::horizontal([Constraint::Min(50), Constraint::Percentage(100)]).areas(events_area);

    let list_events_widget = create_list_events_widget(&app_state.vehicle.messages).block(
        Block::bordered()
            .padding(Padding::horizontal(1))
            .title(" Events ".bold())
            .title_bottom(
                Line::from(format!("Total: {}", &app_state.vehicle.messages.len())).right_aligned(),
            ),
    );
    frame.render_stateful_widget(
        list_events_widget,
        list_events_area,
        &mut app_state.messages_list_state,
    );

    create_event_details_paragraph(app_state.get_selected_message())
        .block(
            Block::bordered()
                .padding(Padding::uniform(1))
                .title(" Event details ".bold()),
        )
        .render(details_events_area, frame.buffer_mut());

    Paragraph::new(
        Span::from("(Esc|q) quit | (↑/↓) previous/next | (Home/End) first/last | (Tab) change tab")
            .gray(),
    )
    .block(Block::bordered())
    .centered()
    .render(help_area, frame.buffer_mut());
}

fn draw_tabs(tab_header: Rect, app_state: &mut AppState, frame: &mut Frame) {
    let tab_index = Screen::iter()
        .position(|x| x == app_state.screen)
        .unwrap_or(0);

    let tab_names = Screen::iter()
        .map(|x| format!(" {} ", x))
        .collect::<Vec<String>>();

    Tabs::new(tab_names)
        .highlight_style(Style::default().bg(Color::Yellow))
        .select(tab_index)
        .block(Block::bordered().border_type(ratatui::widgets::BorderType::Thick))
        .render(tab_header, frame.buffer_mut());
}

fn draw_parameters_screen(app_state: &mut AppState, frame: &mut Frame) {
    let area = frame.area();
    let [tab_header, tab_content] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    draw_tabs(tab_header, app_state, frame);

    Block::bordered()
        .border_type(ratatui::widgets::BorderType::Thick)
        .render(tab_content, frame.buffer_mut());

    let [parameters_area, help_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(3)])
            .margin(1)
            .areas(tab_content);

    let [list_parameters_area, details_parameters_area] =
        Layout::horizontal([Constraint::Min(50), Constraint::Percentage(100)])
            .areas(parameters_area);
    let list_parameters_widget =
        create_list_parameters_widget(&app_state.vehicle.parameter_messages).block(
            Block::bordered()
                .padding(Padding::horizontal(1))
                .title(" Parameters ".bold())
                .title_bottom(
                    Line::from(format!(
                        "{}Total: {}",
                        &app_state
                            .vehicle
                            .last_parameters_request
                            .map(|t| format!("Loaded at: {}, ", t.format("%H:%M:%S").to_string()))
                            .unwrap_or("".to_string()),
                        &app_state.vehicle.parameter_messages.len()
                    ))
                    .right_aligned(),
                ),
        );
    frame.render_stateful_widget(
        list_parameters_widget,
        list_parameters_area,
        &mut app_state.parameters_list_state,
    );

    create_parameter_details_paragraph(app_state.get_selected_parameter())
        .block(
            Block::bordered()
                .padding(Padding::uniform(1))
                .title(" Parameter details ".bold()),
        )
        .render(details_parameters_area, frame.buffer_mut());

    Paragraph::new(
        Span::from("(Esc|q) quit | (↑/↓) previous/next | (Home/End) first/last | (Tab) change tab")
            .gray(),
    )
    .block(Block::bordered())
    .centered()
    .render(help_area, frame.buffer_mut());
}

fn create_event_details_paragraph(message: Option<MavMessage>) -> Paragraph<'static> {
    if let Some(m) = message {
        let mut lines = vec![
            Line::from(format!("Name: {} ", m.message_name())),
            Line::from(format!("Id:   {} ", m.message_id())),
            Line::from(format!("")),
        ];
        match &m {
            MavMessage::HEARTBEAT(data) => {
                lines.push(Line::from(format!(
                    "custom_mode:     {:?} ",
                    data.custom_mode
                )));
                lines.push(Line::from(format!("mavtype:         {:?}", data.mavtype)));
                lines.push(Line::from(format!(
                    "autopilot:       {:?} ",
                    data.autopilot
                )));
                let a = data
                    .base_mode
                    .iter()
                    .map(|x| format!("{:?}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(Line::from(format!("base_mode:       {}", a)));
                lines.push(Line::from(format!(
                    "system_status:   {:?} ",
                    data.system_status
                )));
                lines.push(Line::from(format!(
                    "mavlink_version: {:?} ",
                    data.mavlink_version
                )));
            }
            _ => {
                let l = try_parse_message(&m)
                    .iter()
                    .map(|(k, v)| Line::from(format!("{}: {}", k, v)))
                    .collect::<Vec<_>>();
                lines.extend(l);
            }
        };
        lines.push(Line::from(format!("")));
        lines.push(Line::from(format!("---------------------------------")));
        lines.push(Line::from(format!("Raw Message:")));
        lines.push(Line::from(format!("{:?} ", m)));
        Paragraph::new(lines).wrap(Wrap { trim: false })
    } else {
        Paragraph::new(Line::from(" Please select event "))
    }
}

fn create_parameter_details_paragraph(parameter: Option<PARAM_VALUE_DATA>) -> Paragraph<'static> {
    if let Some(param) = parameter {
        let lines = vec![
            Line::from(format!("Id:       {} ", decode_param_id(&param.param_id))),
            Line::from(format!("Value:    {} ", param.param_value)),
            Line::from(format!("")),
            Line::from(format!("---------------------------------")),
            Line::from(format!("Raw parameter:")),
            Line::from(format!("{:?} ", param)),
        ];
        Paragraph::new(lines).wrap(Wrap { trim: false })
    } else {
        Paragraph::new(Line::from(" Please select parameter "))
    }
}

fn try_parse_message(message: &MavMessage) -> Vec<(String, String)> {
    let original = format!("{:?}", message);
    if let Some(brackets_start) = original.find("{") {
        if let Some(brackets_end) = original.find("}") {
            let details = original[brackets_start + 1..brackets_end]
                .trim()
                .to_string()
                .split(",")
                .filter(|val| val.contains(":"))
                .map(|val| val.split(":").map(|el| el.trim()).collect::<Vec<_>>())
                .map(|val| {
                    (
                        val.get(0).unwrap().to_string(),
                        val.get(1).unwrap().to_string(),
                    )
                })
                .collect::<Vec<_>>();
            let padded = details
                .iter()
                .map(|val| (format!("{:<20}", &val.0).to_string(), val.1.clone()))
                .collect::<Vec<_>>();
            return padded;
        }
    }
    return vec![];
}

fn create_list_events_widget(messages: &Vec<MavMessage>) -> List<'static> {
    let logs: Vec<ListItem> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let content = Line::from(vec![
                Span::from(format!("{:>4}  ", i)).style(Color::Magenta),
                Span::raw(format!("{}", m.message_name())),
            ]);
            ListItem::new(content)
        })
        .collect();
    return List::new(logs).highlight_style(Style::default().bg(Color::Yellow));
}
fn create_list_parameters_widget(parameter_messages: &Vec<PARAM_VALUE_DATA>) -> List<'static> {
    let logs: Vec<ListItem> = parameter_messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let content = Line::from(vec![
                Span::from(format!("{:>4}  ", i)).style(Color::Magenta),
                Span::raw(format!("{}", decode_param_id(&m.param_id))),
            ]);
            ListItem::new(content)
        })
        .collect();
    return List::new(logs).highlight_style(Style::default().bg(Color::Yellow));
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
                'j' => match app_state.screen {
                    Screen::Messages => {
                        app_state.messages_list_state.select_next();
                    }
                    Screen::Parameters => {
                        app_state.parameters_list_state.select_next();
                    }
                    _ => {}
                },
                'k' => match app_state.screen {
                    Screen::Messages => {
                        app_state.messages_list_state.select_previous();
                    }
                    Screen::Parameters => {
                        app_state.parameters_list_state.select_previous();
                    }
                    _ => {}
                },
                _ => {}
            },

            KeyCode::Esc => {
                app_state.is_exit = true;
            }
            KeyCode::Up => match app_state.screen {
                Screen::Messages => {
                    app_state.messages_list_state.select_previous();
                }
                Screen::Parameters => {
                    app_state.parameters_list_state.select_previous();
                }
                _ => {}
            },
            KeyCode::Down => match app_state.screen {
                Screen::Messages => {
                    app_state.messages_list_state.select_next();
                }
                Screen::Parameters => {
                    app_state.parameters_list_state.select_next();
                }
                _ => {}
            },
            KeyCode::Home => match app_state.screen {
                Screen::Messages => {
                    app_state.messages_list_state.select_first();
                }
                Screen::Parameters => {
                    app_state.parameters_list_state.select_first();
                }
                _ => {}
            },
            KeyCode::End => match app_state.screen {
                Screen::Messages => {
                    app_state.messages_list_state.select_last();
                }
                Screen::Parameters => {
                    app_state.parameters_list_state.select_last();
                }
                _ => {}
            },
            KeyCode::PageUp => match app_state.screen {
                Screen::Messages => app_state.messages_list_state.select(
                    app_state
                        .messages_list_state
                        .selected()
                        .map(|x| (x - 20).max(0)),
                ),
                Screen::Parameters => app_state.parameters_list_state.select(
                    app_state
                        .parameters_list_state
                        .selected()
                        .map(|x| (x - 20).max(0)),
                ),
                _ => {}
            },

            KeyCode::PageDown => match app_state.screen {
                Screen::Messages => app_state.messages_list_state.select(
                    app_state
                        .messages_list_state
                        .selected()
                        .map(|x| (x + 20).min(app_state.vehicle.messages.len())),
                ),

                Screen::Parameters => app_state.parameters_list_state.select(
                    app_state
                        .parameters_list_state
                        .selected()
                        .map(|x| (x + 20).min(app_state.vehicle.parameter_messages.len())),
                ),
                _ => {}
            },
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

            KeyCode::Enter => {
                todo!();
            }
            _ => {
                println!("{:?}\r", key.code.as_char());
            }
        }
    }
}
