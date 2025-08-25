use mavlink::Message;
use mavlink::common::{MISSION_ITEM_INT_DATA, MavMessage, PARAM_VALUE_DATA};
use ratatui::text::Text;
use ratatui::widgets::{Cell, Row, Table};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, List, Padding, Paragraph, Tabs, Widget, Wrap},
};

use crate::{AppState, Screen, utils::mavlink::decode_param_id};

use strum::IntoEnumIterator;

pub fn draw_status_screen(app_state: &mut AppState, frame: &mut Frame) {
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
    } else if app_state.vehicle.is_armed {
        Span::from(" Armed ").green()
    } else {
        Span::from(" Disarmed ").red()
    })
    .block(Block::bordered().title(" Arm status ".bold()))
    .centered()
    .render(armed_area, frame.buffer_mut());
}

pub fn draw_messages_screen(app_state: &mut AppState, frame: &mut Frame) {
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
        &mut app_state.messages_table_state,
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

pub fn draw_parameters_screen(app_state: &mut AppState, frame: &mut Frame) {
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
    let [
        list_parameters_brief_area,
        details_parameters_statistics_area,
    ] = Layout::vertical([Constraint::Fill(1), Constraint::Length(6)]).areas(list_parameters_area);
    let list_parameters_widget =
        create_list_parameters_widget(&app_state.vehicle.parameter_messages).block(
            Block::bordered()
                .padding(Padding::horizontal(1))
                .title(" Parameters ".bold()),
        );
    frame.render_stateful_widget(
        list_parameters_widget,
        list_parameters_brief_area,
        &mut app_state.parameters_table_state,
    );

    List::new(vec![
        Line::from(format!(
            "Loaded at: {}",
            &app_state
                .vehicle
                .last_parameters_request
                .map(|t| t.format("%H:%M:%S").to_string())
                .unwrap_or("Not loaded".to_string())
        )),
        Line::from(format!(
            "Total:     {}",
            &app_state.vehicle.parameter_messages.len()
        )),
        Line::from(""),
        Line::from("Press (r) to refresh"),
    ])
    .block(Block::bordered().padding(Padding::horizontal(1)))
    .render(details_parameters_statistics_area, frame.buffer_mut());

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

pub fn draw_mission_screen(app_state: &mut AppState, frame: &mut Frame) {
    let area = frame.area();
    let [tab_header, tab_content] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    draw_tabs(tab_header, app_state, frame);

    Block::bordered()
        .border_type(ratatui::widgets::BorderType::Thick)
        .render(tab_content, frame.buffer_mut());

    let [mission_area, help_area] = Layout::vertical([Constraint::Fill(1), Constraint::Length(3)])
        .margin(1)
        .areas(tab_content);

    let [list_mission_items_area, details_mission_statistics_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(6)]).areas(mission_area);
    let list_mission_items_widget =
        create_list_mission_items_widget(&app_state.vehicle.mission_messages).block(
            Block::bordered()
                .padding(Padding::horizontal(1))
                .title(" Mission items ".bold()),
        );
    frame.render_stateful_widget(
        list_mission_items_widget,
        list_mission_items_area,
        &mut app_state.mission_table_state,
    );

    List::new(vec![
        Line::from(format!(
            "Loaded at: {}",
            &app_state
                .vehicle
                .last_mission_request
                .map(|t| t.format("%H:%M:%S").to_string())
                .unwrap_or("Not loaded".to_string())
        )),
        Line::from(format!(
            "Total:     {}",
            &app_state.vehicle.mission_messages.len()
        )),
        Line::from(""),
        Line::from("Press (r) to refresh"),
    ])
    .block(Block::bordered().padding(Padding::horizontal(1)))
    .render(details_mission_statistics_area, frame.buffer_mut());

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
            Line::from(""),
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
                let base_mode = data
                    .base_mode
                    .iter()
                    .map(|x| format!("{:?}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(Line::from(format!("base_mode:       {}", base_mode)));
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
        lines.push(Line::from(""));
        lines.push(Line::from("---------------------------------"));
        lines.push(Line::from("Raw Message:"));
        lines.push(Line::from(format!("{:?} ", m)));
        Paragraph::new(lines).wrap(Wrap { trim: false })
    } else {
        Paragraph::new(Line::from(" Select message "))
    }
}

fn create_parameter_details_paragraph(parameter: Option<PARAM_VALUE_DATA>) -> Paragraph<'static> {
    if let Some(param) = parameter {
        let lines = vec![
            Line::from(format!("Id:       {} ", decode_param_id(&param.param_id))),
            Line::from(format!("Value:    {} ", param.param_value)),
            Line::from(""),
            Line::from("---------------------------------"),
            Line::from("Raw parameter:"),
            Line::from(format!("{:?} ", param)),
        ];
        Paragraph::new(lines).wrap(Wrap { trim: false })
    } else {
        Paragraph::new(Line::from(" Select parameter "))
    }
}

fn try_parse_message(message: &MavMessage) -> Vec<(String, String)> {
    let original = format!("{:?}", message);
    if let Some(brackets_start) = original.find("{") {
        if let Some(brackets_end) = original.find("}") {
            let details = original[brackets_start + 1..brackets_end]
                .trim()
                .split(",")
                .filter(|val| val.contains(":"))
                .map(|val| val.split(":").map(|el| el.trim()).collect::<Vec<_>>())
                .map(|val| {
                    (
                        val.first().unwrap().to_string(),
                        val.last().unwrap().to_string(),
                    )
                })
                .collect::<Vec<_>>();
            return details
                .iter()
                .map(|val| (format!("{:<20}", &val.0), val.1.clone()))
                .collect::<Vec<_>>();
        }
    }
    vec![]
}

fn create_list_events_widget(messages: &[MavMessage]) -> Table<'static> {
    let rows = messages.iter().enumerate().map(|(i, m)| {
        let cell = Cell::default().content(Line::from(vec![
            Span::from(format!("{:>4}  ", i)).style(Color::Magenta),
            Span::from(m.message_name().to_string()),
        ]));
        Row::new(vec![cell])
    });

    Table::new(rows, [Constraint::Fill(1)]).row_highlight_style(Style::default().bg(Color::Yellow))
}

fn create_list_parameters_widget(parameter_messages: &[PARAM_VALUE_DATA]) -> Table<'static> {
    let rows = parameter_messages.iter().enumerate().map(|(i, m)| {
        let cell = Cell::default().content(Line::from(vec![
            Span::from(format!("{:>4}  ", i)).style(Color::Magenta),
            Span::from(decode_param_id(&m.param_id)),
        ]));
        Row::new(vec![cell])
    });
    Table::new(rows, [Constraint::Fill(1)]).row_highlight_style(Style::default().bg(Color::Yellow))
}

fn create_list_mission_items_widget(mission_items: &[MISSION_ITEM_INT_DATA]) -> Table<'static> {
    let header = ["Seq", "Command", "Frame", "x", "y", "z"]
        .into_iter()
        .map(Cell::from)
        .collect::<Row>()
        .height(1);
    let rows = mission_items.iter().enumerate().map(|(i, m)| {
        Row::new(vec![
            Cell::from(Text::from(format!("{}  ", i)).style(Color::Magenta)),
            Cell::from(Text::from(format!("{:?} ", m.command))),
            Cell::from(Text::from(format!("{:?} ", m.frame))),
            Cell::from(Text::from(format!("{:?} ", m.x)).red()),
            Cell::from(Text::from(format!("{:?} ", m.y)).green()),
            Cell::from(Text::from(format!("{:?} ", m.z)).blue()),
        ])
    });
    Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(30),
            Constraint::Length(40),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::Yellow))
}
