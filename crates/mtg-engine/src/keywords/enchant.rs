//! Enchant (CR 702.5): the restriction of an Aura. It adds no hook: the target of the Aura spell, the host an Aura enters
//! attached to and the state-based action that keeps it legal read it through `attach.rs`.

use super::KeywordRules;

/// The rules of enchant.
#[derive(Debug)]
pub(super) struct Enchant;

impl KeywordRules for Enchant {}
