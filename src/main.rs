mod cli;
mod mavlink_client;
mod utils;
use clap::Parser;
use mavlink::{MavConnection, Message};
use ratatui::DefaultTerminal;
use ratatui::widgets::{List, ListItem, ListState, Padding, Widget, Wrap};
use std::sync::mpsc;
use std::{sync::Arc, thread};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Frame;

use crate::cli::Args;
use color_eyre::Result;
use mavlink::common::MavMessage;
use mavlink::common::MavModeFlag;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

struct Vehicle {
    messages: Vec<MavMessage>,
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

enum Screen {
    Main,
    // Mission,
}
pub struct AppState {
    pub args: crate::cli::Args,

    vehicle: Vehicle,

    pub list_state: ListState,

    is_exit: bool,
    screen: Screen,
}
impl AppState {
    fn default(args: crate::cli::Args, vehicle: Vehicle) -> Self {
        AppState {
            args,
            vehicle,
            is_exit: false,
            list_state: ListState::default().with_selected(Some(0)),
            screen: Screen::Main,
        }
    }
    fn get_selected_message(&self) -> Option<MavMessage> {
        let selected_message_num = self.list_state.selected();
        if let Some(index) = selected_message_num {
            return self.vehicle.messages.get(index).cloned();
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
    let mut fps_limiter = utils::FPSLimiter::default(50);
    while !app_state.is_exit {
        let app_event = rx.recv()?;
        match app_event {
            AppEvent::Input(event) => {
                match app_state.screen {
                    Screen::Main => {
                        handle_input_main_screen(&mut app_state, event);
                        terminal.draw(|frame| draw_main_screen(&mut app_state, frame))?;
                    } // Screen::Mission => {
                      //     // terminal.draw(|frame| draw(&mut app_state, frame))?;
                      // }
                }
            }
            AppEvent::Mavlink(mav_message) => {
                app_state.vehicle.messages.push(mav_message.clone());

                if let mavlink::common::MavMessage::HEARTBEAT(data) = mav_message {
                    let is_armed = data
                        .base_mode
                        .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED);
                    app_state.vehicle.is_armed = is_armed;
                }

                if fps_limiter.check_allowed() {
                    match app_state.screen {
                        Screen::Main => {
                            terminal.draw(|frame| draw_main_screen(&mut app_state, frame))?;
                        } // Screen::Mission => {
                          //    terminal.draw(|frame| draw(&mut app_state, frame))?;
                          // }
                    }
                }
            }
        }
    }
    Ok(())
}

fn draw_main_screen(app_state: &mut AppState, frame: &mut Frame) {
    let area = frame.area();
    let [headear_area, events_area, help_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .areas(area);
    let [connection_area, armed_area] =
        Layout::horizontal([Constraint::Length(50), Constraint::Max(14)]).areas(headear_area);

    let [list_events_area, details_events_area] =
        Layout::horizontal([Constraint::Min(50), Constraint::Percentage(100)]).areas(events_area);
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
        &mut app_state.list_state,
    );

    create_event_details_paragraph(app_state.get_selected_message())
        .block(
            Block::bordered()
                .padding(Padding::uniform(1))
                .title(" Event details ".bold()),
        )
        .render(details_events_area, frame.buffer_mut());

    Paragraph::new(Span::from("(Esc|q) quit | (↑/↓) previous/next | (Home/End) first/last | (Tab) change tab").gray())
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

fn handle_input_main_screen(app_state: &mut AppState, event: Event) {
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
                    app_state.list_state.select_next();
                }
                'k' => {
                    app_state.list_state.select_previous();
                }
                _ => {}
            },

            KeyCode::Esc => {
                app_state.is_exit = true;
            }
            KeyCode::Up => {
                app_state.list_state.select_previous();
            }
            KeyCode::Down => {
                app_state.list_state.select_next();
            }
            KeyCode::Home => {
                app_state.list_state.select_first();
            }
            KeyCode::End => {
                app_state.list_state.select_last();
            }
            KeyCode::PageUp => {
                app_state
                    .list_state
                    .select(app_state.list_state.selected().map(|x| (x - 20).max(0)));
            }
            KeyCode::PageDown => {
                app_state.list_state.select(
                    app_state
                        .list_state
                        .selected()
                        .map(|x| (x + 20).min(app_state.vehicle.messages.len())),
                );
            }
            KeyCode::Tab => {
                // TODO
                app_state.list_state.select(
                    app_state
                        .list_state
                        .selected()
                        .map(|x| (x + 20).min(app_state.vehicle.messages.len())),
                );
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
