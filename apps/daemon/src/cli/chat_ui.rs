// SPDX-License-Identifier: MIT
// Sprint II CH.2 — `clawd chat` ratatui terminal UI.
//
// Full-screen interactive TUI for the chat command:
//   - Header: session ID + provider
//   - Scrollable message history (user / assistant / tool calls)
//   - Input line at the bottom (Enter to send, Ctrl+C to pause, `exit` to quit)
//   - Tool call results shown as collapsed blocks (press `t` to toggle)

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::{SinkExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use serde_json::{json, Value};
use std::io;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::config::DaemonConfig;

/// A single message in the chat history.
#[derive(Debug, Clone)]
struct ChatMessage {
    role: String, // "user" | "assistant" | "tool"
    content: String,
    is_tool_call: bool,
    expanded: bool,
}

/// ratatui-based interactive chat TUI.
pub struct ChatUi {
    session_id: String,
    config: DaemonConfig,
}

impl ChatUi {
    pub fn new(session_id: String, config: &DaemonConfig) -> Self {
        Self {
            session_id,
            config: config.clone(),
        }
    }

    /// Start the interactive TUI loop.
    pub async fn run(self) -> Result<()> {
        // Set up terminal.
        enable_raw_mode().context("enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).context("enter alternate screen")?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("create terminal")?;

        let result = self.event_loop(&mut terminal).await;

        // Restore terminal regardless of result.
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn event_loop(
        &self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let token = crate::cli::client::read_auth_token(&self.config.data_dir)?;
        let url = format!("ws://127.0.0.1:{}", self.config.port);

        let (mut ws, _) =
            tokio::time::timeout(std::time::Duration::from_secs(5), connect_async(&url))
                .await
                .context("timed out connecting to daemon")?
                .context("failed to connect")?;

        // Authenticate.
        ws.send(Message::Text(serde_json::to_string(
            &json!({"jsonrpc":"2.0","id":1,"method":"daemon.auth","params":{"token":token}}),
        )?))
        .await?;

        let mut messages: Vec<ChatMessage> = Vec::new();
        let mut input_buf = String::new();
        let mut status_line = format!("Session: {}", self.session_id);
        let mut is_streaming = false;
        let mut rpc_id: u64 = 10;

        loop {
            // Draw UI.
            terminal.draw(|f| {
                draw_ui(f, &messages, &input_buf, &status_line, is_streaming);
            })?;

            // Poll for terminal events (non-blocking, 50ms timeout).
            if event::poll(std::time::Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match (key.code, key.modifiers) {
                        // Ctrl+C — pause session and exit.
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            let _ = ws
                                .send(Message::Text(serde_json::to_string(&json!({
                                    "jsonrpc":"2.0","id":99,
                                    "method":"session.pause",
                                    "params":{"sessionId":self.session_id}
                                }))?))
                                .await;
                            break;
                        }
                        // Enter — send message (or exit on "exit").
                        (KeyCode::Enter, _) => {
                            let text = input_buf.trim().to_owned();
                            input_buf.clear();

                            if text == "exit" || text == "quit" {
                                break;
                            }
                            if text.is_empty() {
                                continue;
                            }

                            messages.push(ChatMessage {
                                role: "user".to_owned(),
                                content: text.clone(),
                                is_tool_call: false,
                                expanded: true,
                            });

                            rpc_id += 1;
                            let _ = ws
                                .send(Message::Text(serde_json::to_string(&json!({
                                    "jsonrpc":"2.0","id":rpc_id,
                                    "method":"session.send",
                                    "params":{"sessionId":self.session_id,"content":text}
                                }))?))
                                .await;
                            is_streaming = true;
                            status_line = "Thinking…".to_owned();
                        }
                        // Backspace.
                        (KeyCode::Backspace, _) => {
                            input_buf.pop();
                        }
                        // Toggle tool call expansion with `t`.
                        (KeyCode::Char('t'), _) => {
                            if let Some(last_tool) =
                                messages.iter_mut().rev().find(|m| m.is_tool_call)
                            {
                                last_tool.expanded = !last_tool.expanded;
                            }
                        }
                        // Regular character input.
                        (KeyCode::Char(c), _) => {
                            input_buf.push(c);
                        }
                        _ => {}
                    }
                }
            }

            // Poll for WebSocket messages (non-blocking).
            if let Ok(Some(Ok(Message::Text(text)))) =
                tokio::time::timeout(std::time::Duration::from_millis(5), ws.next()).await
            {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    match v.get("method").and_then(|m| m.as_str()) {
                        Some("session.message.delta") => {
                            let delta = v["params"]["delta"].as_str().unwrap_or("");
                            // Append to last assistant message or create one.
                            if let Some(last) =
                                messages.iter_mut().rev().find(|m| m.role == "assistant")
                            {
                                last.content.push_str(delta);
                            } else {
                                messages.push(ChatMessage {
                                    role: "assistant".to_owned(),
                                    content: delta.to_owned(),
                                    is_tool_call: false,
                                    expanded: true,
                                });
                            }
                        }
                        Some("session.message.complete") => {
                            is_streaming = false;
                            status_line = format!("Session: {}", self.session_id);
                        }
                        Some("session.tool_call") => {
                            let name = v["params"]["name"].as_str().unwrap_or("tool");
                            messages.push(ChatMessage {
                                role: "tool".to_owned(),
                                content: format!("Tool call: {name}\n{}", v["params"]["arguments"]),
                                is_tool_call: true,
                                expanded: false,
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
}

// ─── UI rendering ─────────────────────────────────────────────────────────────

fn draw_ui(
    f: &mut ratatui::Frame,
    messages: &[ChatMessage],
    input: &str,
    status: &str,
    is_streaming: bool,
) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(3),    // message list
            Constraint::Length(3), // input area
            Constraint::Length(1), // help line
        ])
        .split(area);

    render_header(f, chunks[0], status, is_streaming);
    render_messages(f, chunks[1], messages);
    render_input(f, chunks[2], input);
    render_help(f, chunks[3]);
}

fn render_header(f: &mut ratatui::Frame, area: Rect, status: &str, streaming: bool) {
    let indicator = if streaming { " ⠋" } else { "" };
    let header = Paragraph::new(format!(" ClawDE Chat  {status}{indicator}"))
        .style(Style::default().bg(Color::Rgb(28, 28, 40)).fg(Color::White));
    f.render_widget(header, area);
}

fn render_messages(f: &mut ratatui::Frame, area: Rect, messages: &[ChatMessage]) {
    let items: Vec<ListItem> = messages
        .iter()
        .flat_map(|m| {
            let (label, style) = match m.role.as_str() {
                "user" => (
                    "You",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                "tool" => ("Tool", Style::default().fg(Color::Yellow)),
                _ => (
                    "Assistant",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            };

            let mut lines = vec![Line::from(Span::styled(label, style))];

            if m.is_tool_call && !m.expanded {
                lines.push(Line::from(Span::styled(
                    "  [collapsed — press t to expand]",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for line in m.content.lines() {
                    lines.push(Line::from(format!("  {line}")));
                }
            }
            lines.push(Line::from(""));
            lines.into_iter().map(ListItem::new).collect::<Vec<_>>()
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Messages"))
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

fn render_input(f: &mut ratatui::Frame, area: Rect, input: &str) {
    let text = Paragraph::new(format!("> {input}▌"))
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White));
    f.render_widget(text, area);
}

fn render_help(f: &mut ratatui::Frame, area: Rect) {
    let help = Paragraph::new(
        " Enter: send  |  Ctrl+C: pause & exit  |  t: toggle tool  |  type 'exit' to quit",
    )
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, area);
}
