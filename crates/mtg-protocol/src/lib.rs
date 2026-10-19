//! Client/server wire protocol.
//!
//! [`ClientMessage`] and [`ServerMessage`] are the two message families. The server never sends
//! raw engine state: [`StateView::for_player`] and [`project_events`] strip everything the viewer
//! may not know. Each [`EventBatch`] carries the table and an object diff, so a client holds a
//! [`StateView`] and advances it with [`StateView::apply`] instead of folding engine events.
//! The account endpoints of the server have their bodies in [`rest`]. TypeScript bindings for all
//! of it come from [`bindings`].

mod apply;
mod diff;

pub mod bindings;
pub mod error;
pub mod events;
pub mod message;
pub mod project;
pub mod rest;
pub mod view;

pub use error::ProtocolError;
pub use events::{EventBatch, EventView, HiddenEvent, project_events};
pub use message::{
    ClientMessage, DecisionId, ErrorCode, FullControlMode, LobbySeat, PROTOCOL_VERSION,
    ServerMessage, Stop, StopSide, YieldUntil,
};
pub use project::zone_is_visible;
pub use rest::{
    AuthErrorCode, CardDetail, CardHit, CardSearchPage, Credentials, MyGame, MyGameSeat,
    MyGameStatus, MyGames, UserView,
};
pub use view::{
    AttackerView, CombatView, ObjectView, PermanentView, PlayerView, PriorityView, StateView,
    TableView, TurnView, ZoneView,
};
