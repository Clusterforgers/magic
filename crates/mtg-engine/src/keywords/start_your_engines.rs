//! Start your engines! (CR 702.179): the speed it starts is given by the state-based actions (`speed.rs`), so the
//! keyword answers no hook.

use super::KeywordRules;

/// The rules of start your engines!.
#[derive(Debug)]
pub(super) struct StartYourEngines;

impl KeywordRules for StartYourEngines {}
