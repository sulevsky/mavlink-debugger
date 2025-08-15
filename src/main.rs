mod cli;
mod mavlink_client;
use crate::mavlink_client::connect;
use clap::Parser;
use mavlink::{MavConnection, Message};
use mavlink::{common::ATTITUDE_DATA, error::MessageReadError};
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;
use ratatui::symbols::{block, border, line};
use ratatui::widgets::{List, ListItem, ListState, Widget};
use std::sync::Mutex;
use std::{env, sync::Arc, thread, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, ModifierKeyCode};
use ratatui::{Frame, text::Text};

use crate::cli::Args;
use color_eyre::Result;
use mavlink::common::MavMessage;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use std::io::{self, Error};

struct Vehicle {
    messages: Arc<Mutex<Vec<MavMessage>>>,
    connection: Option<Arc<Box<dyn MavConnection<MavMessage> + Send + Sync>>>,
    is_armed: Arc<Mutex<bool>>,
}
fn main() -> Result<()> {
    let args = Args::parse();

    color_eyre::install()?;
    let mut terminal = ratatui::init();

    let vehicle = mavlink_client::connect(&args);
    let mut app_state = AppState::default(args, vehicle);

    let app_result = run(&mut app_state, &mut terminal);
    ratatui::restore();
    app_result
}

enum Screen {
    Main,
    AddNew,
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
}

fn run(mut app_state: &mut AppState, terminal: &mut DefaultTerminal) -> Result<()> {
    loop {
        if app_state.is_exit {
            break;
        }
        match app_state.screen {
            Screen::Main => {
                terminal.draw(|frame| draw_main_screen(&mut app_state, frame))?;
                if let Ok(true) = event::poll(Duration::from_millis(10)) {
                    if let Ok(key_event) = event::read() {
                        handle_input_main_screen(&mut app_state, key_event);
                    }
                }
            }
            Screen::AddNew => {
                // terminal.draw(|frame| draw(&mut app_state, frame))?;
            }
        }
    }
    Ok(())
}

fn draw_main_screen(app_state: &mut AppState, frame: &mut Frame) {
    let area = frame.area();
    let [headear_area, events_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);
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
        let is_armed: bool = *(app_state.vehicle.is_armed.lock().unwrap());
        if is_armed {
            Span::from(" Armed ").green()
        } else {
            Span::from(" Disarmed ").red()
        }
    })
    .block(Block::bordered().title(" Arm status ".bold()))
    .centered()
    .render(armed_area, frame.buffer_mut());

    let events_block = Block::bordered()
        .title(" Events ".bold())
        .border_set(border::THICK);
    draw_event(app_state, frame, events_area, events_block);
}

fn draw_event(app_state: &mut AppState, frame: &mut Frame, chunk: Rect, block: Block) {
    let info_style = Style::default().fg(Color::Blue);
    let warning_style = Style::default().fg(Color::Yellow);
    let error_style = Style::default().fg(Color::Magenta);
    let critical_style = Style::default().fg(Color::Red);
    let logs: Vec<ListItem> = app_state
        .vehicle
        .messages
        .lock()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let s = error_style;
            let content = vec![Line::from(vec![
                Span::styled(format!("{:>4} ", i), s),
                Span::raw(format!("{:?}", m)),
            ])];
            ListItem::new(content)
        })
        .collect();
    let logs = List::new(logs).highlight_style(warning_style).block(block);
    frame.render_stateful_widget(logs, chunk, &mut app_state.list_state);
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
                // 'D' => {
                //     if let Some(index) = app_state.list_state.selected() {
                //         app_state.items.remove(index);
                //     }
                // }
                // 'A' => {
                //     app_state.is_add_new = true;
                // }
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

            KeyCode::Enter => {
                todo!();
            }
            _ => {
                println!("{:?}\r", key.code.as_char());
            }
        }
    }
}
