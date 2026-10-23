//! Morph and megamorph (CR 702.37): "You may cast this card face down as a 2/2 creature for {3}. Turn it face up any time
//! for its morph cost." A megamorph permanent gets a +1/+1 counter as it is turned face up.
//!
//! The keywords add no ability. The cast is the way `CastWay::FaceDown` (`ways.rs`), the face-down permanent reads as a
//! 2/2 creature (`face_down.rs`, `object::FaceDown::Morph`) and turning it face up is the special action
//! `PriorityAction::TurnFaceUp` (`announce/turn_up.rs`).

use super::KeywordRules;

/// The rules of morph and megamorph.
#[derive(Debug)]
pub(super) struct Morph;

impl KeywordRules for Morph {}
