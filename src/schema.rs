//! The schema as described in [the specification](https://github.com/VedalAI/neuro-game-sdk/blob/31e36c1a479faa256896a3e172c8d5a96bd462c6/API/SPECIFICATION.md).
use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// A registerable command that Neuro can execute whenever she wants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Action {
    /// The name of the action, which is its *unique identifier*. This should be a lowercase string, with words separated by underscores or dashes (e.g. `"join_friend_lobby"`, `"use_item"`).
    pub name: Cow<'static, str>,
    /// A plaintext description of what this action does. **This information will be directly received by Neuro.**
    pub description: Cow<'static, str>,
    /// A **valid** simple JSON schema object that describes how the response data should look like. If your action does not have any parameters, you can omit this field or set it to `{}`.
    #[serde(default)]
    pub schema: schemars::schema::RootSchema,
}

/// Client command contents (everything except the `game` field). See `ClientCommand` docs for more
/// info.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(tag = "command", content = "data")]
pub enum ClientCommandContents {
    /// This message should be sent as soon as the game starts, to let Neuro know that the game is running.
    ///
    /// This message clears all previously registered actions for this game and does initial setup, and as such should be the very first message that you send.
    #[serde(rename = "startup")]
    Startup,
    /// This message can be sent to let Neuro know about something that is happening in game.
    #[serde(rename = "context")]
    Context {
        /// A plaintext message that describes what is happening in the game. **This information will be directly received by Neuro.**
        message: Cow<'static, str>,
        /// If `true`, the message will be added to Neuro's context without prompting her to respond to it. If `false`, Neuro might respond to the message directly, unless she is busy talking to someone else or to chat.
        silent: bool,
    },
    /// This message registers one or more actions for Neuro to use.
    #[serde(rename = "actions/register")]
    RegisterActions {
        /// An array of actions to be registered. If you try to register an action that is already registered, it will be ignored.
        actions: Vec<Action>,
    },
    /// This message unregisters one or more actions, preventing Neuro from using them anymore.
    #[serde(rename = "actions/unregister")]
    UnregisterActions {
        /// The names of the actions to unregister. If you try to unregister an action that isn't registered, there will be no problem.
        action_names: Vec<Cow<'static, str>>,
    },
    /// This message forces Neuro to execute one of the listed actions as soon as possible. Note that this might take a bit if she is already talking.
    #[serde(rename = "actions/force")]
    ForceActions {
        /// An arbitrary string that describes the current state of the game. This can be plaintext, JSON, Markdown, or any other format. **This information will be directly received by Neuro.**
        state: Option<Cow<'static, str>>,
        /// A plaintext message that tells Neuro what she is currently supposed to be doing (e.g. `"It is now your turn. Please perform an action. If you want to use any items, you should use them before picking up the shotgun."`). **This information will be directly received by Neuro.**
        query: Cow<'static, str>,
        /// If `false`, the context provided in the `state` and `query` parameters will be remembered by Neuro after the actions force is compelted. If `true`, Neuro will only remember it for the duration of the actions force.
        ephemeral_context: Option<bool>,
        /// The names of the actions that Neuro should choose from.
        action_names: Vec<Cow<'static, str>>,
    },
    /// This message needs to be sent as soon as possible after an action is validated, to allow Neuro to continue.
    ///
    /// # Important
    ///
    /// Until you send an action result, Neuro will just be waiting for the result of her action!
    /// Please make sure to send this as soon as possible.
    /// It should usually be sent after validating the action parameters, before it is actually executed in-game.
    ///
    /// # Tip
    ///
    /// Since setting `success` to false will retry the action force if there was one, if the action was not successful but you don't want it to be retried, you should set `success` to `true` and provide an error message in the `message` field.
    #[serde(rename = "actions/result")]
    ActionResult {
        /// The id of the action that this result is for. This is grabbed from the action message directly.
        id: String,
        /// Whether or not the action was successful. *If this is `false` and this action is part of an actions force, the whole actions force will be immediately retried by Neuro.*
        success: bool,
        /// A plaintext message that describes what happened when the action was executed. If not successful, this should be an error message. If successful, this can either be empty, or provide a *small* context to Neuro regarding the action she just took (e.g. `"Remember to not share this with anyone."`). **This information will be directly received by Neuro.**
        message: Option<Cow<'static, str>>,
    },
    /// This message should be sent as a response to a graceful or an imminent shutdown request, after progress has been saved. After this is sent, Neuro will close the game herself by terminating the process, so to reiterate you must definitely ensure that progress has already been saved.
    ///
    /// # Note
    ///
    /// This is part of the game automation API, which will only be used for games that Neuro can launch by herself.
    /// As such, most games will not need to implement this.
    #[cfg(feature = "proposals")]
    #[serde(rename = "shutdown/ready")]
    ShutdownReady,
}

/// A client to server (game to Neuro) message.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ClientCommand {
    /// The command itself.
    #[serde(flatten)]
    pub command: ClientCommandContents,
    /// The game name. This is used to identify the game. It should *always* be the same and should not change. You should use the game's display name, including any spaces and symbols (e.g. `"Buckshot Roulette"`). The server will not include this field.
    pub game: Cow<'static, str>,
}

/// A server to client (Neuro to game) message.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "command", content = "data")]
#[non_exhaustive]
pub enum ServerCommand {
    #[serde(rename = "action")]
    Action {
        /// A unique id for the action. You should use it when sending back the action result.
        id: String,
        /// The name of the action that Neuro is trying to execute.
        name: String,
        /// The JSON-stringified data for the action, as sent by Neuro. This *should* be an object that matches the JSON schema you provided when registering the action. If you did not provide a schema, this parameter will usually be undefined.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<String>,
    },
    /// If there is a problem mid-game and Neuro crashes, upon reconnection this message might be sent in order to reregister all actions that were previously registered. You should respond to this with an actions register containing all actions that are currently supposed to be registered.
    #[cfg(feature = "proposals")]
    #[serde(rename = "actions/reregister_all")]
    ReregisterAllActions,
    /// This message will be sent when Neuro decides to stop playing a game, or upon manual intervention from the dashboard. You should create or identify graceful shutdown points where the game can be closed gracefully after saving progress. You should store the latest received `wants_shutdown` value, and if it is `true{} when a graceful shutdown point is reached, you should save the game and quit to main menu, then send back a shutdown ready message.
    ///
    /// # Note
    ///
    /// This is part of the game automation API, which will only be used for games that Neuro can launch by herself.
    /// As such, most games will not need to implement this.
    ///
    /// **Please don't actually close the game, just quit to main menu. Neuro will close the game herself.**
    #[cfg(feature = "proposals")]
    #[serde(rename = "shutdown/graceful")]
    GracefulShutdown {
        /// Whether the game should shutdown at the next graceful shutdown point. `true` means shutdown is requested, `false` means to cancel the previous shutdown request.
        wants_shutdown: bool,
    },
    /// This message will be sent when the game needs to be shutdown immediately. You have only a handful of seconds to save as much progress as possible. After you have saved, you can send back a shutdown ready message.
    ///
    /// # Note
    ///
    /// This is part of the game automation API, which will only be used for games that Neuro can launch by herself.
    /// As such, most games will not need to implement this.
    ///
    /// **Please don't actually close the game, just save the current progress that can be saved. Neuro will close the game herself.**
    #[cfg(feature = "proposals")]
    #[serde(rename = "shutdown/immediate")]
    ImmediateShutdown,
}

#[cfg(test)]
mod tests {
    use schemars::schema::{InstanceType, Schema, SingleOrVec};

    use super::*;

    fn parse<'a, T: serde::Deserialize<'a>>(data: &'a str) -> T {
        serde_json::from_str(data).unwrap()
    }

    fn ser<T: serde::Serialize>(x: &T) -> String {
        // its easier to work with string slices and this is tests dont judge ok?
        crate::to_string(x).unwrap()
    }

    #[test]
    fn test_action_roundtrip() {
        // no schema
        const SAMPLE1: &str = r#"{"name":"test","description":"abcd","schema":{}}"#;
        const SAMPLE2: &str = r#"{"name":"test","description":"abcd"}"#;
        let a: Action = parse(SAMPLE1);
        let b: Action = parse(SAMPLE2);
        assert_eq!(&a.name, "test");
        assert_eq!(&a.description, "abcd");
        assert_eq!(a, b);
        assert_eq!(&ser(&a), SAMPLE1);
        assert_eq!(ser(&a), ser(&b));
        // yes schema
        const SAMPLE3: &str = r#"{"name":"test","description":"abcd","schema":{"type":"object","properties":{"test":{"type":"string"}},"required":["test"]}}"#;
        let c: Action = parse(SAMPLE3);
        let schema = c.schema.schema;
        assert!(
            matches!(schema.instance_type.as_ref().unwrap(), SingleOrVec::Single(x) if **x == InstanceType::Object)
        );
        let object_schema = schema.object.unwrap();
        assert!(object_schema.required.contains("test"));
        let Schema::Object(prop_schema) = object_schema.properties.get("test").unwrap() else {
            panic!()
        };
        assert!(
            matches!(prop_schema.instance_type.as_ref().unwrap(), SingleOrVec::Single(x) if **x == InstanceType::String)
        );
        assert!(object_schema.required.contains("test"));
    }

    #[test]
    fn test_command_roundtrip() {
        let neuro_cmd = ServerCommand::Action {
            id: "abcd".to_owned(),
            name: "efgh".to_owned(),
            data: None,
        };
        const SAMPLE_ACTION: &str = r#"{"command":"action","data":{"id":"abcd","name":"efgh"}}"#;
        assert_eq!(parse::<ServerCommand>(SAMPLE_ACTION), neuro_cmd);
        assert_eq!(SAMPLE_ACTION, ser(&neuro_cmd));

        let startup = ClientCommand {
            game: "game".into(),
            command: ClientCommandContents::Startup,
        };
        const STARTUP: &str = r#"{"command":"startup","game":"game"}"#;
        assert_eq!(parse::<ClientCommand>(STARTUP), startup);
        assert_eq!(STARTUP, ser(&startup));

        let context = ClientCommand {
            game: "game".into(),
            command: ClientCommandContents::Context {
                message: "test".into(),
                silent: false,
            },
        };
        const CONTEXT: &str =
            r#"{"command":"context","data":{"message":"test","silent":false},"game":"game"}"#;
        assert_eq!(parse::<ClientCommand>(CONTEXT), context);
        assert_eq!(CONTEXT, ser(&context));
    }
}
