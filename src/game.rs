//! A high(er) level API that utilizes the Rust type system for somewhat better ergonomics.
use std::{borrow::Cow, sync::Arc};

use crate::schema::{self, ClientCommandContents, ServerCommand};

mod glue;

pub use glue::{ActionMetadata, Actions};
use schemars::schema::{Schema, SchemaObject, SingleOrVec};
use thiserror::Error;

/// A trait to be implemented by your game to create an [`Api`] object.
///
/// # Example
///
/// ```rust,ignore
/// use schemars::JsonSchema;
/// use serde::Deserialize;
///
/// // Note that the default schema for this will allow any integers from 0 to 255, which isn't
/// // necessarily what we want. If you want to customize this, you will have to manually implement
/// // the `JsonSchema` trait.
/// #[derive(Debug, JsonSchema, Deserialize)]
/// struct Move {
///     x: u8,
///     y: u8,
/// }
///
/// #[derive(Debug, JsonSchema, Deserialize)]
/// struct Forfeit;
///
/// // All of the actions available to Neuro. **The doc comments will be directly passed to Neuro as
/// // explanation of what the actions do.** The `name` attribute is used to specify the name the
/// // actions should have for Neuro - the API documentation says:
/// //
/// // > This should be a lowercase string, with words separated by underscores or dashes.
/// #[derive(Debug, neuro_sama::derive::Actions)]
/// enum Action {
///     /// Make a move, placing your mark on the field at a specified position.
///     #[name("move")]
///     Move(Move),
///     /// Forfeit
///     #[name("forfeit")]
///     Forfeit(Forfeit),
/// }
///
/// struct TicTacToe { ... }
///
/// impl Game for TicTacToe {
///     const NAME: &'static str = "Tic Tac Toe";
///     type Actions<'a> = Action;
///
///     fn handle_action<'a>(
///        &self,
///        action: Self::Actions<'a>,
///     ) -> Result<
///         Option<impl 'static + Into<Cow<'static, str>>>,
///         Option<impl 'static + Into<Cow<'static, str>>>,
///     > {
///         Err(Some("not yet implemented".into()))
///     }
///
///     fn send_command(&self, _message: tungstenite::Message) {
///         // TODO: send the websocket message
///     }
/// }
///
/// let game = Arc::new(TicTacToe::new());
/// let api = neuro_sama::game::Api::new(game)?;
/// api.context("something something you are playing tic tac toe")?;
/// api.register_actions::<Action>()?;
///
/// for message in websocket_channel {
///     api.notify_message(message)?;
/// }
/// ```
pub trait Game: Sized {
    /// The game's display name, including any spaces and symbols (e.g. `"Buckshot Roulette"`).
    const NAME: &'static str;
    /// A enum with all the action types that Neuro can pass to the game.
    ///
    /// The `json5` crate is used for handling the input, since the JSON is generated by Neuro.
    /// To actually create this enum, make an enum over types that implement the [`Action`] trait,
    /// and make sure the enum tags as seen by `serde` match what [`Action::name()`] returns. This
    /// is a bit annoying, so for convenience, you can use the `derive` module.
    type Actions<'a>: Actions<'a>;
    /// Handle Neuro's action.
    ///
    /// # Parameters
    ///
    /// - `api` - the API this action came from
    /// - `action` - the action that Neuro passed to the game.
    ///
    /// # Returns
    ///
    /// A result with an optional associated message to pass to Neuro. The result should be
    /// returned as soon as possible, usually before actually executing the action in-game.
    ///
    /// # Note
    ///
    /// If you return `Err` on a forced action, Neuro will try again. If you don't want that, just
    /// return `Ok` with an error message.
    fn handle_action<'a>(
        &self,
        api: &Api<Self>,
        action: Self::Actions<'a>,
    ) -> Result<
        Option<impl 'static + Into<Cow<'static, str>>>,
        Option<impl 'static + Into<Cow<'static, str>>>,
    >;
    /// Called when required by the game to reregister all available actions
    fn reregister_actions(&self, api: &Api<Self>);
    #[cfg(feature = "proposals")]
    /// You should create or identify graceful shutdown points where the game can be closed gracefully after saving progress. You should store the latest received wants_shutdown value, and if it is true when a graceful shutdown point is reached, you should save the game and quit to main menu, then send back a shutdown ready message. Don't close the game entirely.
    ///
    /// # Note
    ///
    /// This is part of the game automation API, which will only be used for games that Neuro can launch by herself. As such, most games will not need to implement this.
    fn graceful_shutdown_wanted(&self, api: &Api<Self>, wants_shutdown: bool) {
        let _ = (api, wants_shutdown);
    }
    #[cfg(feature = "proposals")]
    /// This message will be sent when the game needs to be shutdown immediately. You have only a handful of seconds to save as much progress as possible. After you have saved, you can send back a shutdown ready message (don't close the game by yourself).
    ///
    /// # Note
    ///
    /// This is part of the game automation API, which will only be used for games that Neuro can launch by herself. As such, most games will not need to implement this.
    fn immediate_shutdown(&self, api: &Api<Self>) {
        let _ = api;
    }
    /// Send a message to the WebSocket backend. If an error happens, you can handle it by
    /// attempting to reopen the connection and calling `reinitialize` on the API after a
    /// reconnect.
    fn send_command(&self, api: &Api<Self>, message: tungstenite::Message);
}

/// An error that occured somewhere while sending/receiving a message.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    /// A JSON error
    #[error("json error: {0}")]
    Json(
        #[from]
        #[source]
        serde_json::Error,
    ),
}

/// API object for... accessing the API. Note that this can be cheaply cloned.
#[derive(Debug)]
pub struct Api<G: Game> {
    game: Arc<G>,
}

impl<G: Game> Clone for Api<G> {
    fn clone(&self) -> Self {
        Self {
            game: self.game.clone(),
        }
    }
}

/// A trait that has to be implemented by actions. It is automatically implemented when you create
/// an enum for all actions with `#[derive(neuro_sama::derive::Actions)]`.
///
/// Note that while there aren't any hard limitations on how complex the JSON schema can be, Neuro
/// might get confused if it's too complex.
pub trait Action: schemars::JsonSchema {
    /// The name of the action, which is its *unique identifier*. This should be a lowercase string, with words separated by underscores or dashes (e.g. `"join_friend_lobby"`, `"use_item"`).
    fn name() -> &'static str;
    /// A plaintext description of what this action does. **This information will be directly received by Neuro.**
    fn description() -> &'static str;
}

fn cleanup_action(action: &mut schema::Action) {
    fn visit_schema(schema: &mut Schema) {
        match schema {
            Schema::Object(obj) => visit_schema_obj(obj),
            Schema::Bool(_) => {}
        }
    }
    fn visit_schema_obj(schema: &mut SchemaObject) {
        if let Some(meta) = schema.metadata.as_mut() {
            meta.description = None;
            meta.title = None;
        }
        if let Some(arr) = schema.array.as_mut() {
            for x in arr.items.iter_mut() {
                match x {
                    SingleOrVec::Single(schema) => visit_schema(schema),
                    SingleOrVec::Vec(schemas) => {
                        for schema in schemas {
                            visit_schema(schema);
                        }
                    }
                }
            }
            for x in arr
                .contains
                .iter_mut()
                .chain(arr.additional_items.iter_mut())
            {
                visit_schema(x);
            }
        }
        if let Some(obj) = schema.object.as_mut() {
            for schema in obj
                .properties
                .values_mut()
                .chain(obj.pattern_properties.values_mut())
                .chain(
                    obj.additional_properties
                        .iter_mut()
                        .chain(obj.property_names.iter_mut())
                        .map(|x| &mut **x),
                )
            {
                visit_schema(schema);
            }
        }
        if let Some(sub) = schema.subschemas.as_mut() {
            for schema in sub
                .all_of
                .iter_mut()
                .chain(sub.any_of.iter_mut())
                .chain(sub.one_of.iter_mut())
                .flat_map(|x| x.iter_mut())
                .chain(
                    sub.not
                        .iter_mut()
                        .chain(sub.if_schema.iter_mut())
                        .chain(sub.then_schema.iter_mut())
                        .chain(sub.else_schema.iter_mut())
                        .map(|x| &mut **x),
                )
            {
                visit_schema(schema);
            }
        }
    }
    action.schema.meta_schema = None;
    visit_schema_obj(&mut action.schema.schema);
}

impl<G: Game> Api<G> {
    /// Create a new API object. This takes an `Arc` of your game, this forces it to not be mutable
    /// but that's fully intended because asynchronous action handling is theoretically allowed,
    /// since each action has a separate ID which means multiple parallel actions can happen at the
    /// same time.
    pub fn new(game: Arc<G>) -> Result<Self, Error> {
        let ret = Self { game };
        ret.reinitialize()?;
        Ok(ret)
    }
    /// Reinitialize the API (sending the `startup` action).
    pub fn reinitialize(&self) -> Result<(), Error> {
        let ret = self.send_command(ClientCommandContents::Startup);
        if ret.is_ok() {
            self.game.reregister_actions(self);
        }
        ret
    }
    /// This message can be sent to let Neuro know about something that is happening in game.
    pub fn context(
        &self,
        context: impl Into<Cow<'static, str>>,
        silent: bool,
    ) -> Result<(), Error> {
        self.send_command(ClientCommandContents::Context {
            message: context.into(),
            silent,
        })
    }
    /// Register actions.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use schemars::JsonSchema;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct Move {
    ///     x: u32,
    ///     y: u32,
    /// }
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct Shoot;
    ///
    /// #[derive(neuro_sama::game::Actions)]
    /// enum Action {
    ///     /// Move to a different position
    ///     #[name = "move"]
    ///     Move(Move),
    ///     /// Shoot the enemy
    ///     #[name = "shoot"]
    ///     Shoot(Shoot),
    /// }
    ///
    /// api.register_actions::<(Move, Shoot)>();
    /// // or
    /// api.register_actions::<Action>();
    ///
    /// // later
    /// api.unregister_actions::<(Move, Shoot)>();
    /// // or
    /// api.unregister_actions::<Move>();
    /// ```
    pub fn register_actions<A: ActionMetadata>(&self) -> Result<(), Error> {
        let mut actions = A::actions();
        for action in &mut actions {
            cleanup_action(action);
        }
        self.register_actions_raw(actions)
    }
    /// Unregister actions. See `register_actions` for example use.
    pub fn unregister_actions<A: ActionMetadata>(&self) -> Result<(), Error> {
        self.unregister_actions_raw(A::names())
    }
    /// Directly call `actions/unregister`. You should typically use `unregister_actions` instead.
    pub fn unregister_actions_raw(
        &self,
        action_names: Vec<Cow<'static, str>>,
    ) -> Result<(), Error> {
        self.send_command(ClientCommandContents::UnregisterActions { action_names })
    }
    /// Directly call `actions/register`. You should typically use `register_actions` instead.
    pub fn register_actions_raw(&self, actions: Vec<schema::Action>) -> Result<(), Error> {
        self.send_command(ClientCommandContents::RegisterActions { actions })
    }
    fn send_command(&self, cmd: schema::ClientCommandContents) -> Result<(), Error> {
        let data = serde_json::to_string(&schema::ClientCommand {
            command: cmd,
            game: G::NAME.into(),
        })?;
        self.game
            .send_command(self, tungstenite::Message::text(data));
        Ok(())
    }
    /// Notify the API object of a new websocket message. Note that this only handles `Text` and
    /// `Binary` messages, the rest are silently ignored.
    pub fn notify_message(&self, message: tungstenite::Message) -> Result<(), Error> {
        let message = match message {
            tungstenite::Message::Text(s) => serde_json::from_str(&s)?,
            tungstenite::Message::Binary(b) => serde_json::from_slice(&b)?,
            _ => return Ok(()),
        };
        let (id, res) = match message {
            ServerCommand::Action { id, name, data } => {
                let res = if let Some(data) = data.as_ref().filter(|x| !x.is_empty()) {
                    json5::Deserializer::from_str(data)
                        .and_then(|mut de| <G::Actions<'_> as Actions>::deserialize(&name, &mut de))
                } else {
                    <G::Actions<'_> as Actions>::deserialize(
                        &name,
                        serde::de::value::UnitDeserializer::new(),
                    )
                };
                let data = match res {
                    Ok(data) => data,
                    Err(err) => {
                        return self.send_command(ClientCommandContents::ActionResult {
                            id,
                            success: false,
                            message: Some(
                                ("Failed to deserialize Neuro-provided action data: ".to_owned()
                                    + &err.to_string())
                                    .into(),
                            ),
                        });
                    }
                };
                (id, self.game.handle_action(self, data))
            }
            #[cfg(feature = "proposals")]
            ServerCommand::ReregisterAllActions => {
                self.game.reregister_actions(self);
                return Ok(());
            }
            #[cfg(feature = "proposals")]
            ServerCommand::GracefulShutdown { wants_shutdown } => {
                self.game.graceful_shutdown_wanted(self, wants_shutdown);
                return Ok(());
            }
            #[cfg(feature = "proposals")]
            ServerCommand::ImmediateShutdown => {
                self.game.immediate_shutdown(self);
                return Ok(());
            }
        };
        let res = match res {
            Ok(msg) => ClientCommandContents::ActionResult {
                id,
                success: true,
                message: msg.map(Into::into),
            },
            Err(msg) => ClientCommandContents::ActionResult {
                id,
                success: false,
                message: msg.map(Into::into),
            },
        };
        self.send_command(res)
    }
    /// Tell Neuro to execute one of the listed actions as soon as possible. Note that this might take a bit if she is already talking.
    ///
    /// # Parameters
    ///
    /// - `query` - A plaintext message that tells Neuro what she is currently supposed to be doing (e.g. `"It is now your turn. Please perform an action. If you want to use any items, you should use them before picking up the shotgun."`). **This information will be directly received by Neuro.**
    /// - `action_names` - The names of the actions that Neuro should choose from.
    ///
    /// # Returns
    ///
    /// A builder object that can be used to configure the request further. After you've configured
    /// it, please send the request using the `.send()` method on the builder.
    #[must_use]
    pub fn force_actions<T: ActionMetadata>(
        &self,
        query: Cow<'static, str>,
    ) -> ForceActionsBuilder<G> {
        self.force_actions_raw(query, T::names())
    }
    /// A version of `force_actions` that uses raw action names instead of type parameters.
    #[must_use]
    pub fn force_actions_raw(
        &self,
        query: Cow<'static, str>,
        action_names: Vec<Cow<'static, str>>,
    ) -> ForceActionsBuilder<G> {
        ForceActionsBuilder {
            api: self,
            state: None,
            query,
            ephemeral_context: None,
            action_names,
        }
    }
}

/// A builder object for sending an `actions/force` message.
pub struct ForceActionsBuilder<'a, G: Game> {
    api: &'a Api<G>,
    state: Option<Cow<'static, str>>,
    query: Cow<'static, str>,
    ephemeral_context: Option<bool>,
    action_names: Vec<Cow<'static, str>>,
}

impl<'a, G: Game> ForceActionsBuilder<'a, G> {
    /// If `false`, the context provided in the `state` and `query` parameters will be remembered by Neuro after the actions force is compelted. If `true`, Neuro will only remember it for the duration of the actions force.
    pub fn with_ephemeral_context(mut self, ephemeral_context: bool) -> Self {
        self.ephemeral_context = Some(ephemeral_context);
        self
    }
    /// An arbitrary string that describes the current state of the game. This can be plaintext, JSON, Markdown, or any other format. **This information will be directly received by Neuro.**
    pub fn with_state(mut self, state: impl Into<Cow<'static, str>>) -> Self {
        self.state = Some(state.into());
        self
    }
    /// Send the WebSocket message to the server.
    pub fn send(self) -> Result<(), Error> {
        self.api
            .send_command(schema::ClientCommandContents::ForceActions {
                state: self.state,
                query: self.query,
                ephemeral_context: self.ephemeral_context,
                action_names: self.action_names,
            })
    }
}

#[cfg(test)]
mod test {
    use serde::Deserialize;

    use crate::{
        self as neuro_sama,
        game::{cleanup_action, ActionMetadata},
    };

    /// Move action
    #[derive(Debug, schemars::JsonSchema, Deserialize, PartialEq)]
    struct Move {
        x: u32,
        y: u32,
    }

    /// Shoot action
    #[derive(Debug, schemars::JsonSchema, Deserialize, PartialEq)]
    struct Shoot;

    #[derive(crate::derive::Actions, Debug, PartialEq)]
    enum Action {
        /// test1
        #[name = "move"]
        Move(Move),
        /// test2
        #[name = "shoot"]
        Shoot(Shoot),
    }

    #[test]
    fn test() {
        use super::Actions;
        let mut deser = serde_json::Deserializer::from_str(r#"{"x":5,"y":6}"#);
        let action = <Action as Actions>::deserialize("move", &mut deser).unwrap();
        assert_eq!(action, Action::Move(Move { x: 5, y: 6 }));
        let mut deser = json5::Deserializer::from_str(r#"null"#).unwrap();
        let action = <Action as Actions>::deserialize("shoot", &mut deser).unwrap();
        assert_eq!(action, Action::Shoot(Shoot));
        let mut actions = <Action as ActionMetadata>::actions();
        for action in &mut actions {
            cleanup_action(action);
        }
        assert_eq!(
            serde_json::to_string(&actions).unwrap(),
            r#"[
              {
                "name": "move",
                "description": "test 1",
                "schema": {
                  "type": "object",
                  "required": [ "x", "y" ],
                  "properties": {
                    "x": { "type": "integer", "format": "uint32", "minimum": 0.0 },
                    "y": { "type": "integer", "format": "uint32", "minimum": 0.0 }
                  }
                }
              },
              {
                "name": "shoot",
                "description": "test 2",
                "schema": {
                  "type": "null"
                }
              }
            ]"#
            .to_string()
            .replace(|x| x == ' ' || x == '\n', "")
        );
    }
}
