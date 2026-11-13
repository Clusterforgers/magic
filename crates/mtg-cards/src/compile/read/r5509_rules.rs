//! Rules of the r55-09 pack read whole: "While an opponent is choosing targets as part of casting a spell they control or
//! activating an ability they control, that player must choose at least one Flagbearer on the battlefield if able." and
//! "During your turn, as long as you haven't activated an exhaust ability this turn, you may activate exhaust abilities as
//! though they haven't been activated."

use super::tokens::Reader;
use crate::modify::GameRule;

/// The sentence of `GameRule::Flagbearers`, without its period.
const R5509_FLAGBEARERS: &str = "While an opponent is choosing targets as part of casting a spell they control or \
                                 activating an ability they control, that player must choose at least one Flagbearer \
                                 on the battlefield if able";

/// The sentence of `GameRule::ExhaustAgain`, without its period.
const R5509_EXHAUST_AGAIN: &str = "During your turn, as long as you haven't activated an exhaust ability this turn, you \
                                   may activate exhaust abilities as though they haven't been activated";

/// One of the rules this module reads, the whole sentence but its period.
pub(super) fn r5509_rule_read(r: &mut Reader<'_>) -> Option<GameRule> {
    if r.words(&R5509_FLAGBEARERS.to_lowercase()) {
        return Some(GameRule::Flagbearers);
    }
    r.words(&R5509_EXHAUST_AGAIN.to_lowercase())
        .then_some(GameRule::ExhaustAgain)
}

/// What `r5509_rule_read` reads, written back.
pub(super) fn r5509_rule_texts(rule: &GameRule) -> Vec<String> {
    match rule {
        GameRule::Flagbearers => vec![R5509_FLAGBEARERS.to_owned()],
        GameRule::ExhaustAgain => vec![R5509_EXHAUST_AGAIN.to_owned()],
        _ => Vec::new(),
    }
}
