//! Rules about searches and extra turns: "Your opponents can't search libraries." (`GameRule::CantSearch`) and "If an
//! opponent would begin an extra turn, that player skips that turn instead." (`GameRule::SkipExtraTurns`).

use super::tokens::Reader;
use crate::modify::GameRule;
use crate::refs::Ref;

/// The group a rule names: "your opponents" and "an opponent", "players" and "a player".
fn group(r: &mut Reader<'_>, opponents: &str, everyone: &str) -> Option<Ref> {
    if r.words(opponents) {
        Some(Ref::Opponents)
    } else if r.words(everyone) {
        Some(Ref::EachPlayer)
    } else {
        None
    }
}

/// "Your opponents can't search libraries.", "Players can't search libraries." and "If an opponent would begin an
/// extra turn, that player skips that turn instead.", with "a player" for everyone.
pub(super) fn read(r: &mut Reader<'_>) -> Option<GameRule> {
    if let Some(who) = r.attempt(|r| {
        let who = group(r, "your opponents", "players")?;
        r.need("can't search libraries")?;
        Some(who)
    }) {
        return Some(GameRule::CantSearch(who));
    }
    r.need("if")?;
    let who = group(r, "an opponent", "a player")?;
    r.need("would begin an extra turn")?;
    r.punct(',').then_some(())?;
    r.need("that player skips that turn instead")?;
    Some(GameRule::SkipExtraTurns(who))
}

/// What `read` reads, written back.
pub(super) fn texts(rule: &GameRule) -> Vec<String> {
    let (who, search) = match rule {
        GameRule::CantSearch(who) => (who, true),
        GameRule::SkipExtraTurns(who) => (who, false),
        _ => return Vec::new(),
    };
    let text = match (who, search) {
        (Ref::Opponents, true) => "Your opponents can't search libraries",
        (Ref::EachPlayer, true) => "Players can't search libraries",
        (Ref::Opponents, false) => {
            "If an opponent would begin an extra turn, that player skips that turn instead"
        }
        (Ref::EachPlayer, false) => {
            "If a player would begin an extra turn, that player skips that turn instead"
        }
        _ => return Vec::new(),
    };
    vec![text.to_owned()]
}
