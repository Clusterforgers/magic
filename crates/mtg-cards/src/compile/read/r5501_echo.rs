//! Another cost for echo: "You may pay {0} rather than pay the echo cost for permanents you control." (the rule
//! `EchoRather` of its controller, from the battlefield). Read from the wording and written back the same way.

use super::costs;
use super::tokens::Reader;
use crate::modify::GameRule;

/// The words after the cost.
const TAIL: &str = "rather than pay the echo cost for permanents you control";

/// "You may pay {0} rather than pay the echo cost for permanents you control."
pub(super) fn r5501_echo_rather(r: &mut Reader<'_>) -> Option<GameRule> {
    r.need("you may pay")?;
    let cost = costs::cost(r)?;
    r.need(TAIL)?;
    Some(GameRule::EchoRather(cost))
}

/// What `r5501_echo_rather` reads, written back.
pub(super) fn r5501_echo_rather_text(rule: &GameRule) -> Option<String> {
    let GameRule::EchoRather(cost) = rule else {
        return None;
    };
    Some(format!("You may pay {} {TAIL}", costs::text(cost)?))
}
