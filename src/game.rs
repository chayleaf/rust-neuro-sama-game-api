//! A high(er) level API that utilizes the Rust type system for somewhat better ergonomics.
//!
//! You should implement the [`Game`] trait to use the [`Api`] trait on your object (which will
//! call into [`Game`] methods when it receives messages from the API).
//!
//! If you require mutable access to your game object, you should use the [`GameMut`] trait
//! instead - then you can use the [`ApiMut`] trait, which is exactly the same as [`Api`], except
//! it takes a mutable reference, allowing you to mutate the object. You don't have to implement
//! both.
use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use crate::schema::{self, ClientCommandContents, ServerCommand};

mod glue;

pub use glue::{ActionMetadata, Actions};
use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
use thiserror::Error;

/// A trait to be implemented by your game to create an [`Api`] object.
///
/// You should generally *not* call these methods yourself - instead, call [`Api`] methods, which
/// will call into your code.
///
/// # Example
///
/// ```rust,ignore
/// use schemars::JsonSchema;
/// use serde::Deserialize;
/// use neuro_sama::game::{Api, Game};
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
/// //
/// // By default, for each struct/enum that the command consists of, `title` is set to the struct
/// // name and `description` is set to the doc comment. However, this library currently strips
/// // that to make the schema smaller and potentially less confusing. If you think that this can
/// // actually help make the schema more understandable in some cases, feel free to open an issue.
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
/// let game = TicTacToe::new();
/// // IMPORTANT: call initialize first
/// game.initialize()?;
/// game.context("something something you are playing tic tac toe")?;
/// game.register_actions::<Action>()?;
///
/// for message in websocket_channel {
///     game.notify_message(message)?;
/// }
/// ```
#[neuro_sama_derive::generic_mutability(GameMut)]
pub trait Game: Sized {
    /// The game's display name, including any spaces and symbols (e.g. `"Buckshot Roulette"`).
    const NAME: &'static str;

    /// A enum with all the action types that Neuro can pass to the game.
    ///
    /// The `json5` crate is used for handling the input, since the JSON is generated by Neuro.
    /// To actually create this enum, make an enum over types that implement the [`Action`] trait,
    /// and make sure the enum tags as seen by `serde` match what [`Action::name()`] returns. This
    /// is a bit annoying, so for convenience, you can use the `neuro_sama::derive` module.
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
        action: Self::Actions<'a>,
    ) -> Result<
        Option<impl 'static + Into<Cow<'static, str>>>,
        Option<impl 'static + Into<Cow<'static, str>>>,
    >;

    /// Called when required by the game to reregister all available actions
    fn reregister_actions(&self);

    /// You should create or identify graceful shutdown points where the game can be closed gracefully after saving progress. You should store the latest received wants_shutdown value, and if it is true when a graceful shutdown point is reached, you should save the game and quit to main menu, then send back a shutdown ready message. Don't close the game entirely.
    ///
    /// # Note
    ///
    /// This is part of the game automation API, which will only be used for games that Neuro can launch by herself. As such, most games will not need to implement this.
    #[cfg(feature = "proposals")]
    fn graceful_shutdown_wanted(&self, wants_shutdown: bool) {
        let _ = wants_shutdown;
    }

    /// This message will be sent when the game needs to be shutdown immediately. You have only a handful of seconds to save as much progress as possible. After you have saved, you can send back a shutdown ready message (don't close the game by yourself).
    ///
    /// # Note
    ///
    /// This is part of the game automation API, which will only be used for games that Neuro can launch by herself. As such, most games will not need to implement this.
    #[cfg(feature = "proposals")]
    fn immediate_shutdown(&self) {}

    /// Send a message to the WebSocket backend. If an error happens, you can handle it by
    /// attempting to reopen the connection and calling [`Api::initialize`] on the API after a
    /// reconnect.
    fn send_command(&self, message: tungstenite::Message);
}

impl<G: Game, T: Deref<Target = G>> Game for T {
    const NAME: &'static str = G::NAME;
    type Actions<'a> = G::Actions<'a>;

    fn handle_action<'a>(
        &self,
        action: Self::Actions<'a>,
    ) -> Result<
        Option<impl 'static + Into<Cow<'static, str>>>,
        Option<impl 'static + Into<Cow<'static, str>>>,
    > {
        self.deref()
            .handle_action(action)
            .map(|x| x.map(Into::into))
            .map_err(|x| x.map(Into::into))
    }
    fn reregister_actions(&self) {
        self.deref().reregister_actions();
    }
    #[cfg(feature = "proposals")]
    fn graceful_shutdown_wanted(&self, wants_shutdown: bool) {
        self.deref().graceful_shutdown_wanted(wants_shutdown);
    }
    #[cfg(feature = "proposals")]
    fn immediate_shutdown(&self) {
        self.deref().immediate_shutdown();
    }
    fn send_command(&self, message: tungstenite::Message) {
        self.deref().send_command(message);
    }
}

impl<G: GameMut, T: DerefMut<Target = G>> GameMut for T {
    const NAME: &'static str = G::NAME;
    type Actions<'a> = G::Actions<'a>;

    fn handle_action<'a>(
        &mut self,
        action: Self::Actions<'a>,
    ) -> Result<
        Option<impl 'static + Into<Cow<'static, str>>>,
        Option<impl 'static + Into<Cow<'static, str>>>,
    > {
        self.deref_mut()
            .handle_action(action)
            .map(|x| x.map(Into::into))
            .map_err(|x| x.map(Into::into))
    }
    fn reregister_actions(&mut self) {
        self.deref_mut().reregister_actions();
    }
    #[cfg(feature = "proposals")]
    fn graceful_shutdown_wanted(&mut self, wants_shutdown: bool) {
        self.deref_mut().graceful_shutdown_wanted(wants_shutdown);
    }
    #[cfg(feature = "proposals")]
    fn immediate_shutdown(&mut self) {
        self.deref_mut().immediate_shutdown();
    }
    fn send_command(&mut self, message: tungstenite::Message) {
        self.deref_mut().send_command(message);
    }
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
            for x in &mut arr.items {
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
    match &action.schema.schema.instance_type {
        Some(SingleOrVec::Single(x)) if **x == InstanceType::Null => {
            action.schema.schema.instance_type = None;
        }
        _ => {}
    }
}

fn send_ws_command<G: Game>(game: &G, cmd: schema::ClientCommandContents) -> Result<(), Error> {
    let data = crate::to_string(&schema::ClientCommand {
        command: cmd,
        game: G::NAME.into(),
    })?;
    game.send_command(tungstenite::Message::text(data));
    Ok(())
}

fn send_ws_command_mut<G: GameMut>(
    game: &mut G,
    cmd: schema::ClientCommandContents,
) -> Result<(), Error> {
    let data = crate::to_string(&schema::ClientCommand {
        command: cmd,
        game: G::NAME.into(),
    })?;
    game.send_command(tungstenite::Message::text(data));
    Ok(())
}

impl<T: Game> Api for T {}
impl<T: GameMut> ApiMut for T {}

/// A sealed trait implemented for all objects that implement [`Game`]. You can use these methods for
/// talking to the Neuro API. Main points of interest are [`Api::initialize`] (which must be called first)
/// and [`Api::handle_message`] for handling incoming WebSocket messages.
#[neuro_sama_derive::generic_mutability(ApiMut, GameMut)]
pub trait Api: Game {
    /// Reinitialize the API (sending the `startup` action and reregistering all actions).
    ///
    /// **This *must* be called before using any other method from [`Api`]**, and also whenever the
    /// WebSocket connection is reopened.
    ///
    /// Sadly, this isn't enforced in the type system, because the typestate pattern would be quite
    /// bulky here, and this isn't enforced in runtime because traits don't have any state - so
    /// please just remember to call it.
    ///
    /// A previous version of this crate had a separate struct just for enforcing this being
    /// called, but not enforcing this at all seems to lead to a better API.
    fn initialize(&self) -> Result<(), Error> {
        let ret = send_ws_command(self, ClientCommandContents::Startup);
        if ret.is_ok() {
            self.reregister_actions();
        }
        ret
    }

    /// This message can be sent to let Neuro know about something that is happening in game.
    ///
    /// # Parameters
    ///
    /// - `context` - a plaintext message that describes what is happening in the game. **This information will be directly received by Neuro.**
    /// - `silent` - if `true`, the message will be added to Neuro's context without prompting her to respond to it. If `false`, Neuro might respond to the message directly, unless she is busy talking to someone else or to chat.
    fn context(&self, context: impl Into<Cow<'static, str>>, silent: bool) -> Result<(), Error> {
        send_ws_command(
            self,
            ClientCommandContents::Context {
                message: context.into(),
                silent,
            },
        )
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
    fn register_actions<A: ActionMetadata>(&self) -> Result<(), Error> {
        self.register_actions_raw(A::actions())
    }

    /// Directly call `actions/register`. You should typically use [`Api::register_actions`] instead.
    fn register_actions_raw(&self, mut actions: Vec<schema::Action>) -> Result<(), Error> {
        for action in &mut actions {
            cleanup_action(action);
        }
        send_ws_command(self, ClientCommandContents::RegisterActions { actions })
    }

    /// Unregister actions. See [`Api::register_actions`] for example use.
    fn unregister_actions<A: ActionMetadata>(&self) -> Result<(), Error> {
        self.unregister_actions_raw(A::names())
    }

    /// Directly call `actions/unregister`. You should typically use [`Api::unregister_actions`] instead.
    fn unregister_actions_raw(&self, action_names: Vec<Cow<'static, str>>) -> Result<(), Error> {
        send_ws_command(
            self,
            ClientCommandContents::UnregisterActions { action_names },
        )
    }

    /// Handle a new websocket message. Note that this only handles `Text` and `Binary` messages,
    /// the rest are silently ignored.
    fn handle_message(&self, message: tungstenite::Message) -> Result<(), Error> {
        let message = match message {
            tungstenite::Message::Text(s) => serde_json::from_str(&s)?,
            tungstenite::Message::Binary(b) => serde_json::from_slice(&b)?,
            _ => return Ok(()),
        };
        let (id, res) = match message {
            ServerCommand::Action { id, name, data } => {
                let res = data.as_ref().filter(|x| !x.trim().is_empty()).map_or_else(
                    || {
                        <Self::Actions<'_> as Actions>::deserialize(
                            &name,
                            serde::de::value::UnitDeserializer::new(),
                        )
                    },
                    |data| match json5::Deserializer::from_str(data) {
                        Ok(mut de) => <Self::Actions<'_> as Actions>::deserialize(&name, &mut de),
                        Err(err) => {
                            let mut data = data.clone();
                            data.retain(|x| !x.is_whitespace());
                            if data.is_empty() || data == "{}" {
                                <Self::Actions<'_> as Actions>::deserialize(
                                    &name,
                                    serde::de::value::UnitDeserializer::new(),
                                )
                                .map_err(|_: serde_json::Error| err)
                            } else {
                                Err(err)
                            }
                        }
                    },
                );
                let data = match res {
                    Ok(data) => data,
                    Err(err) => {
                        return send_ws_command(
                            self,
                            ClientCommandContents::ActionResult {
                                id,
                                success: false,
                                message: Some(
                                    ("Failed to deserialize Neuro-provided action data: "
                                        .to_owned()
                                        + &err.to_string())
                                        .into(),
                                ),
                            },
                        );
                    }
                };
                (id, self.handle_action(data))
            }
            #[cfg(feature = "proposals")]
            ServerCommand::ReregisterAllActions => {
                self.reregister_actions();
                return Ok(());
            }
            #[cfg(feature = "proposals")]
            ServerCommand::GracefulShutdown { wants_shutdown } => {
                self.graceful_shutdown_wanted(wants_shutdown);
                return Ok(());
            }
            #[cfg(feature = "proposals")]
            ServerCommand::ImmediateShutdown => {
                self.immediate_shutdown();
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
        send_ws_command(self, res)
    }

    /// Tell Neuro to execute one of the listed actions as soon as possible. Note that this might take a bit if she is already talking.
    ///
    /// # Parameters
    ///
    /// - `query` - a plaintext message that tells Neuro what she is currently supposed to be doing (e.g. `"It is now your turn. Please perform an action. If you want to use any items, you should use them before picking up the shotgun."`). **This information will be directly received by Neuro.**
    /// - `action_names` - the names of the actions that Neuro should choose from.
    ///
    /// # Returns
    ///
    /// A builder object that can be used to configure the request further. After you've configured
    /// it, please send the request using the `.send()` method on the builder.
    #[must_use]
    fn force_actions<T: ActionMetadata>(
        &self,
        query: Cow<'static, str>,
    ) -> ForceActionsBuilder<Self> {
        self.force_actions_raw(query, T::names())
    }

    /// A version of [`Api::force_actions`] that uses raw action names instead of type parameters.
    #[must_use]
    fn force_actions_raw(
        &self,
        query: Cow<'static, str>,
        action_names: Vec<Cow<'static, str>>,
    ) -> ForceActionsBuilder<Self> {
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
pub struct ForceActionsBuilder<'a, G: Api> {
    api: &'a G,
    state: Option<Cow<'static, str>>,
    query: Cow<'static, str>,
    ephemeral_context: Option<bool>,
    action_names: Vec<Cow<'static, str>>,
}

/// A mutable version of [`ForceActionsBuilder`]. See [`ForceActionsBuilder`] docs for more info.
pub struct ForceActionsBuilderMut<'a, G: ApiMut> {
    api: &'a mut G,
    state: Option<Cow<'static, str>>,
    query: Cow<'static, str>,
    ephemeral_context: Option<bool>,
    action_names: Vec<Cow<'static, str>>,
}

#[neuro_sama_derive::generic_mutability(ForceActionsBuilderMut, ApiMut)]
impl<'a, G: Api> ForceActionsBuilder<'a, G> {
    /// If `false`, the context provided in the `state` and `query` parameters will be remembered by Neuro after the actions force is compelted. If `true`, Neuro will only remember it for the duration of the actions force.
    #[must_use]
    pub fn with_ephemeral_context(mut self, ephemeral_context: bool) -> Self {
        self.ephemeral_context = Some(ephemeral_context);
        self
    }

    /// An arbitrary string that describes the current state of the game. This can be plaintext, JSON, Markdown, or any other format. **This information will be directly received by Neuro.**
    #[must_use]
    pub fn with_state(mut self, state: impl Into<Cow<'static, str>>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Send the WebSocket message to the server.
    pub fn send(self) -> Result<(), Error> {
        send_ws_command(
            self.api,
            schema::ClientCommandContents::ForceActions {
                state: self.state,
                query: self.query,
                ephemeral_context: self.ephemeral_context,
                action_names: self.action_names,
            },
        )
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
        #[cfg(feature = "strip-trailing-zeroes")]
        assert_eq!(
            crate::to_string(&actions).unwrap(),
            r#"[
              {
                "name": "move",
                "description": "test 1",
                "schema": {
                  "type": "object",
                  "required": [ "x", "y" ],
                  "properties": {
                    "x": { "type": "integer", "format": "uint32", "minimum": 0 },
                    "y": { "type": "integer", "format": "uint32", "minimum": 0 }
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
        #[cfg(not(feature = "strip-trailing-zeroes"))]
        assert_eq!(
            crate::to_string(&actions).unwrap(),
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
