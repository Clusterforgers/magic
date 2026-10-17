//! Client/server wire protocol.
//!
//! [`ClientMessage`] and [`ServerMessage`] are the two message families. The server never sends
//! raw engine state: [`StateView::for_player`] and [`project_events`] strip everything the viewer
//! may not know. TypeScript bindings for all of it come from [`bindings`].

pub mod bindings;
pub mod error;
pub mod events;
pub mod message;
pub mod project;
pub mod view;

pub use error::ProtocolError;
pub use events::{EventBatch, EventView, HiddenEvent, project_events};
pub use message::{ClientMessage, DecisionId, ErrorCode, PROTOCOL_VERSION, ServerMessage};
pub use project::zone_is_visible;
pub use view::{
    AttackerView, CombatView, ObjectView, PermanentView, PlayerView, PriorityView, StateView,
    TurnView, ZoneView,
};
