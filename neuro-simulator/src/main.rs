use std::collections::BTreeMap;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use iced::futures::SinkExt;
use iced::futures::StreamExt;
use iced::widget::button;
use iced::widget::combo_box;
use iced::widget::text;
use iced::widget::text_editor;
use iced::widget::Column;
use iced::Element;
use iced::Pixels;
use iced::Subscription;
use iced::Task;
use iced::Theme;
use neuro_sama::schema::ClientCommand;
use neuro_sama::schema::ClientCommandContents;
use tokio::sync::mpsc;

struct State {
    action: combo_box::State<String>,
    selected_action: Option<String>,
    content: text_editor::Content,
    content_valid: bool,
    id_counter: AtomicU32,
    actions: BTreeMap<String, neuro_sama::schema::Action>,
    // contains (sent id, query, action_names, state)
    #[allow(clippy::type_complexity)]
    force_query: Option<(Option<String>, String, Vec<String>, Option<String>)>,
    tx: tokio::sync::mpsc::UnboundedSender<MessageBack>,
    context: (String, bool),
    state: String,
    last_message: String,
}

#[derive(Debug, Clone)]
enum Message {
    Sender(tokio::sync::mpsc::UnboundedSender<MessageBack>),
    Command(ClientCommand),
    ActionEdit(text_editor::Action),
    ActionChanged(String),
    Send,
}

#[derive(Debug, Clone)]
enum MessageBack {
    Action {
        id: String,
        name: String,
        data: Option<String>,
    },
}

fn update(state: &mut State, message: Message) {
    match message {
        Message::Command(cmd) => match cmd.command {
            ClientCommandContents::RegisterActions { actions } => {
                for action in actions {
                    state
                        .actions
                        .insert(action.name.clone().into_owned(), action);
                }
                state.action = combo_box::State::new(state.actions.keys().cloned().collect());
            }
            ClientCommandContents::UnregisterActions { action_names } => {
                for name in action_names {
                    state.actions.remove(name.as_ref());
                    if state.selected_action.as_deref() == Some(name.as_ref()) {
                        state.selected_action = None;
                    }
                }
                state.action = combo_box::State::new(state.actions.keys().cloned().collect());
            }
            ClientCommandContents::ForceActions {
                state: state1,
                query,
                ephemeral_context,
                action_names,
            } => {
                state.action = combo_box::State::new(
                    action_names
                        .iter()
                        .map(|x| x.clone().into_owned())
                        .collect(),
                );
                if let Some(sel) = &state.selected_action {
                    if !action_names.iter().any(|x| x == sel.as_str()) {
                        state.selected_action = None;
                    }
                }
                if !ephemeral_context.unwrap_or_default() {
                    state.state = state1.clone().map(|x| x.into_owned()).unwrap_or_default();
                }
                state.force_query = Some((
                    None,
                    query.to_string(),
                    action_names.into_iter().map(Into::into).collect(),
                    state1.map(Into::into),
                ));
            }
            ClientCommandContents::Startup => {}
            ClientCommandContents::Context { message, silent } => {
                state.context = (message.into_owned(), silent);
            }
            ClientCommandContents::ActionResult {
                id,
                success,
                message,
            } => {
                state.last_message = if success {
                    "success: ".to_owned()
                } else {
                    "failure: ".to_owned()
                } + message.as_deref().unwrap_or_default();
                if success
                    && matches!(state.force_query.as_ref().and_then(|x| x.0.as_ref()), Some(x) if x == &id)
                {
                    state.force_query = None;
                    state.action = combo_box::State::new(state.actions.keys().cloned().collect());
                    if let Some(sel) = &state.selected_action {
                        if !state.actions.keys().any(|x| x == sel.as_str()) {
                            state.selected_action = None;
                        }
                    }
                }
            }
            _ => {}
        },
        Message::Sender(tx) => state.tx = tx,
        Message::ActionChanged(act) => {
            state.content_valid = if let Some(action) = state.actions.get(&act) {
                let schema = serde_json::to_value(&action.schema).unwrap();
                match serde_json::from_str(&state.content.text()) {
                    Ok(value) => jsonschema::is_valid(&schema, &value),
                    Err(_) => false,
                }
            } else {
                false
            };
            state.selected_action = Some(act);
        }
        Message::ActionEdit(action) => {
            state.content.perform(action);
            let text = state.content.text();
            state.content_valid = if let Some(action) = state
                .actions
                .get(state.selected_action.as_deref().unwrap_or(""))
            {
                let schema = serde_json::to_value(&action.schema).unwrap();
                match serde_json::from_str(&text) {
                    Ok(value) => jsonschema::is_valid(&schema, &value),
                    Err(_) => false,
                }
            } else {
                false
            };
        }
        Message::Send => {
            let _ = state.tx.send(MessageBack::Action {
                id: if let Some((ref mut id, ..)) = state.force_query.as_mut() {
                    let id1 = state.id_counter.fetch_add(1, Ordering::SeqCst);
                    *id = Some(id1.to_string());
                    id1
                } else {
                    state.id_counter.fetch_add(1, Ordering::SeqCst)
                }
                .to_string(),
                name: state.selected_action.clone().unwrap_or_default(),
                data: {
                    let text = state.content.text();
                    if text.is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                },
            });
        }
    }
}

fn view(state: &State) -> Element<Message> {
    let mut ret = Column::new().padding(20).spacing(20);
    if let Some((_id, query, _action_names, state)) = &state.force_query {
        ret = ret.push(text(format!("force action awaiting reply: {query}")));
        ret = ret.push_maybe(
            state
                .as_ref()
                .map(|state| text(format!("force action state: {state:?}"))),
        );
    } else {
        ret = ret.push(text(format!("current state: {:?}", state.state)));
    }
    if !state.context.0.is_empty() {
        if state.context.1 {
            ret = ret.push(text(format!(
                "{}current context: {}",
                if state.context.1 {
                    "(this is secret, don't share it with chat!) "
                } else {
                    ""
                },
                state.context.0
            )));
        } else {
            ret = ret.push(text(&state.context.0));
        }
    }
    if !state.last_message.is_empty() {
        ret = ret.push(text(format!("last message: {}", state.last_message)));
    }
    ret = ret.push(combo_box(
        &state.action,
        "Action to send",
        state.selected_action.as_ref(),
        Message::ActionChanged,
    ));
    let mut editor = text_editor(&state.content).on_action(Message::ActionEdit);
    if !state.content_valid {
        editor = editor.style(|theme: &Theme, status| text_editor::Style {
            value: iced::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            background: iced::Background::Color(iced::Color {
                r: 1.0,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            }),
            ..text_editor::Catalog::style(
                theme,
                &<Theme as text_editor::Catalog>::default(),
                status,
            )
        });
    }
    ret = ret.push(editor);
    ret = ret.push(button("send").on_press_with(|| Message::Send));
    if let Some(act) = state
        .actions
        .get(&state.selected_action.clone().unwrap_or_default())
    {
        ret = ret.push(text(serde_json::to_string_pretty(&act.schema).unwrap()).size(Pixels(16.0)));
    }
    ret.into()
}

pub fn main() -> iced::Result {
    let (tx, _rx) = mpsc::unbounded_channel();
    let state = State {
        action: combo_box::State::default(),
        force_query: None,
        last_message: "".to_owned(),
        selected_action: None,
        state: "".to_owned(),
        content: Default::default(),
        content_valid: false,
        id_counter: AtomicU32::new(0),
        actions: BTreeMap::new(),
        tx,
        context: (String::new(), false),
    };
    iced::application("Neuro Simulator", update, view)
        .settings(iced::Settings {
            default_text_size: iced::Pixels(24.0),
            ..Default::default()
        })
        .subscription(|_state| {
            Subscription::run(|| {
                iced::stream::channel(32, |mut tx| async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000")
                        .await
                        .unwrap();
                    let (server_tx, mut rx) = mpsc::unbounded_channel();
                    tx.send(Message::Sender(server_tx)).await.unwrap();
                    while let Ok((stream, _)) = listener.accept().await {
                        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                        loop {
                            tokio::select! {
                                msg = rx.recv() => {
                                    println!("game2ws {msg:#?}");
                                    let Some(msg) = msg else {
                                        break;
                                    };
                                    if ws.send(
                                        tokio_tungstenite::tungstenite::Message::Text(serde_json::to_string(&match msg {
                                            MessageBack::Action { id, name, data } => neuro_sama::schema::ServerCommand::Action {
                                                id,
                                                name,
                                                data,
                                            },
                                        }).unwrap())
                                    ).await.is_err() {
                                        println!("websocket send failed");
                                        break;
                                    }
                                }
                                msg = ws.next() => {
                                    println!("ws2neuro {msg:#?}");
                                    let Some(msg) = msg else {
                                        break;
                                    };
                                    let Ok(msg) = msg else {
                                        continue;
                                    };
                                    if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                                        let data = serde_json::from_str::<neuro_sama::schema::ClientCommand>(&text).unwrap();
                                        tx.send(Message::Command(data)).await.unwrap();
                                    }
                                    // tx.send(msg);
                                }

                            }
                        }
                    }
                })
            })
        })
        .theme(theme)
        .run_with(|| (state, Task::none()))
}

fn theme(_state: &State) -> Theme {
    Theme::TokyoNight
}
