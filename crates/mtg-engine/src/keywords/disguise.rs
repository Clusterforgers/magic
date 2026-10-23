//! Disguise (CR 702.168): morph, and the face-down 2/2 creature has ward {2}.
//!
//! The keyword adds no ability; it is cast and turned face up as morph is (`keywords/morph.rs`), and the ward belongs to
//! the face-down characteristics (`face_down.rs`, `object::FaceDown::Disguise`).

use super::KeywordRules;

/// The rules of disguise.
#[derive(Debug)]
pub(super) struct Disguise;

impl KeywordRules for Disguise {}
