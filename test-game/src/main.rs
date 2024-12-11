use std::{sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use neuro_sama::game::Api;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::mpsc;

struct TestGame(mpsc::UnboundedSender<tungstenite::Message>);

#[allow(unused)]
#[derive(Debug, Deserialize, JsonSchema)]
struct Action1 {
    a: String,
    b: u32,
    c: u16,
    d: bool,
}

#[allow(unused)]
#[derive(Debug, Deserialize, JsonSchema)]
struct Action2 {
    a: u32,
    b: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct Action3;

#[derive(Debug, neuro_sama::derive::Actions)]
enum Action {
    /// Action 1 description
    #[allow(unused)]
    #[name = "action1"]
    Action1(Action1),
    /// Action 2 description
    #[name = "action2"]
    Action2(Action2),
    /// Action 3 description
    #[name = "action3"]
    Action3(Action3),
}

impl neuro_sama::game::Game for TestGame {
    const NAME: &'static str = "Test Game";
    type Actions<'a> = Action;
    fn send_command(&self, _api: &Api<Self>, message: tungstenite::Message) {
        let _ = self.0.send(message);
    }
    fn reregister_actions(&self, api: &Api<Self>) {
        // your game could have some complicated logic here i guess
        api.register_actions::<Action>().unwrap();
    }
    fn handle_action<'a>(
        &self,
        _api: &Api<Self>,
        action: Self::Actions<'a>,
    ) -> Result<
        Option<impl 'static + Into<std::borrow::Cow<'static, str>>>,
        Option<impl 'static + Into<std::borrow::Cow<'static, str>>>,
    > {
        match action {
            Action::Action3(_) => Err(Some("try again")),
            Action::Action1(_) => Ok(None),
            Action::Action2(act) => {
                if act.b {
                    Ok(Some("ok"))
                } else {
                    Err(Some("err"))
                }
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (game2ws_tx, mut game2ws_rx) = mpsc::unbounded_channel();
    let game = Arc::new(TestGame(game2ws_tx));
    let api = Api::new(game.clone()).unwrap();
    let api1 = api.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(20)).await;
            api1.force_actions::<Action>("do your thing".into())
                .with_state("some state idk")
                .send()
                .unwrap();
        }
    });
    let mut ws = tokio_tungstenite::connect_async("ws://127.0.0.1:8000")
        .await
        .unwrap()
        .0;
    loop {
        tokio::select! {
            msg = game2ws_rx.recv() => {
                println!("game2ws {msg:?}");
                let Some(msg) = msg else {
                    break;
                };
                if ws.send(msg).await.is_err() {
                    println!("websocket send failed");
                    break;
                }
            }
            msg = ws.next() => {
                println!("ws2game {msg:?}");
                let Some(msg) = msg else {
                    break;
                };
                let Ok(msg) = msg else {
                    continue;
                };
                if let Err(err) = api.notify_message(msg) {
                    // this could happen because we don't know what this message means (e.g. added
                    // in a new version of the API)
                    println!("notify_message failed: {err}");
                    continue;
                }
            }

        }
    }
}
