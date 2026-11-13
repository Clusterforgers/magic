//! Rules about searches and extra turns: "Your opponents can't search libraries." (`GameRule::CantSearch`) and "If an
//! opponent would begin an extra turn, that player skips that turn instead." (`GameRule::SkipExtraTurns`).

use super::tokens::Reader;
use crate::modify::GameRule;
use crate::refs::Ref;

/// The event of Ichor Elixir's rule.
const R4707_PLANAR_DICE: &str = "you would roll one or more planar dice";
/// What Ichor Elixir's rule does instead.
const R4707_MORE_PLANAR_DICE: &str = "instead roll that many planar dice plus one and ignore one";
/// The event of Susan Foreman's rule.
const R4707_PLANESWALK: &str = "you would planeswalk";
/// What Susan Foreman's rule does instead.
const R4707_LOOK_TWO: &str = "instead look at the top two cards of your planar deck, put one on the bottom of your \
                              planar deck and the other on top, then planeswalk";

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
    if r.words("spells and abilities your opponents control can't cause their controller to search their library") {
        return Some(GameRule::OwnSpellsCantSearch(Ref::Opponents));
    }
    r.need("if")?;
    if let Some(rule) = r.attempt(r6013_search_top) {
        return Some(rule);
    }
    if r.words("you would flip a coin") {
        r.punct(',').then_some(())?;
        r.need("instead flip two coins and ignore one")?;
        return Some(GameRule::FlipTwoCoins(Ref::You));
    }
    if r.words("you would roll one or more dice") {
        r.punct(',').then_some(())?;
        r.need("instead roll that many dice plus one and ignore the lowest roll")?;
        return Some(GameRule::ExtraDie(Ref::You));
    }
    if r.words(R4707_PLANAR_DICE) {
        r.punct(',').then_some(())?;
        r.need(R4707_MORE_PLANAR_DICE)?;
        return Some(GameRule::ExtraPlanarDie(Ref::You));
    }
    if r.words(R4707_PLANESWALK) {
        r.punct(',').then_some(())?;
        for (at, part) in R4707_LOOK_TWO.split(", ").enumerate() {
            if at > 0 {
                r.punct(',').then_some(())?;
            }
            r.need(part)?;
        }
        return Some(GameRule::PlaneswalkLookTwo(Ref::You));
    }
    let who = group(r, "an opponent", "a player")?;
    r.need("would begin an extra turn")?;
    r.punct(',').then_some(())?;
    r.need("that player skips that turn instead")?;
    Some(GameRule::SkipExtraTurns(who))
}

/// "an opponent would search a library, that player searches the top four cards of that library instead", after "If".
fn r6013_search_top(r: &mut Reader<'_>) -> Option<GameRule> {
    let who = group(r, "an opponent", "a player")?;
    r.need("would search a library")?;
    r.punct(',').then_some(())?;
    r.need("that player searches the top")?;
    let count = super::amounts::number_word(r.take_word()?)?;
    r.need("cards of that library instead")?;
    Some(GameRule::SearchTop { who, count })
}

/// What `read` reads, written back.
pub(super) fn texts(rule: &GameRule) -> Vec<String> {
    let (who, search) = match rule {
        GameRule::SearchTop { who, count } => {
            let who = match who {
                Ref::Opponents => "an opponent",
                Ref::EachPlayer => "a player",
                _ => return Vec::new(),
            };
            let Some(count) = super::amounts::count_word(*count) else {
                return Vec::new();
            };
            return vec![format!(
                "If {who} would search a library, that player searches the top {count} cards of that library instead"
            )];
        }
        GameRule::FlipTwoCoins(Ref::You) => {
            return vec![
                "If you would flip a coin, instead flip two coins and ignore one".to_owned(),
            ];
        }
        GameRule::ExtraPlanarDie(Ref::You) => {
            return vec![format!("If {R4707_PLANAR_DICE}, {R4707_MORE_PLANAR_DICE}")];
        }
        GameRule::PlaneswalkLookTwo(Ref::You) => {
            return vec![format!("If {R4707_PLANESWALK}, {R4707_LOOK_TWO}")];
        }
        GameRule::ExtraDie(Ref::You) => {
            return vec![
                "If you would roll one or more dice, instead roll that many dice plus one and ignore the lowest roll"
                    .to_owned(),
            ];
        }
        GameRule::OwnSpellsCantSearch(Ref::Opponents) => {
            return vec![
                "Spells and abilities your opponents control can't cause their controller to search their library"
                    .to_owned(),
            ];
        }
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
