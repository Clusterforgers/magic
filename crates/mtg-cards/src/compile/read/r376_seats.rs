//! Players named by their seat (CR 101.4): "the player to your right", "the opponent to your left", and "the player to
//! your right gains control of this artifact", and "Reverse the game's turn order."

use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule, Modification};
use crate::refs::{Ref, Side};

/// "the player to your left", "the opponent to your right": the neighbour of the controller on that side.
pub(super) fn r376_seated(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        r.need("the")?;
        let opponent = if r.word("opponent") {
            true
        } else {
            r.need("player")?;
            false
        };
        r.need("to your")?;
        let side = match r.take_word()? {
            "left" => Side::Left,
            "right" => Side::Right,
            _ => return None,
        };
        Some(Ref::Seated {
            of: Box::new(Ref::You),
            side,
            opponent,
        })
    })
}

/// The words of a seat the reader reads: "the player to your right".
pub(super) fn r376_seated_text(who: &Ref) -> Option<String> {
    let Ref::Seated {
        of,
        side,
        opponent,
    } = who
    else {
        return None;
    };
    (**of == Ref::You).then_some(())?;
    let noun = if *opponent { "opponent" } else { "player" };
    let side = match side {
        Side::Left => "left",
        Side::Right => "right",
    };
    Some(format!("the {noun} to your {side}"))
}

/// "the player to your right gains control of ~": the card changes hands for good.
pub(super) fn r376_seat_control(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let who = r376_seated(r)?;
    r.need("gains control of")?;
    r.source().then_some(())?;
    Some(vec![Effect::Continuous {
        affected: Ref::This,
        mods: vec![Modification::SetController(who)],
        duration: Duration::Forever,
    }])
}

/// The rule "reverse the game's turn order" makes, for the rest of the game.
fn r376_reverse() -> Effect {
    Effect::Rule {
        rule: GameRule::ReverseTurnOrder,
        duration: Duration::Forever,
    }
}

/// "reverse the game's turn order".
pub(super) fn r376_reverse_turn_order(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("reverse the game's turn order")?;
    Some(vec![r376_reverse()])
}

/// The words of the rule [`r376_reverse_turn_order`] reads.
pub(super) fn r376_reverse_text(effect: &Effect) -> Option<String> {
    (*effect == r376_reverse()).then(|| "reverse the game's turn order".to_owned())
}

/// The roll of "roll a dN and create a number of Treasure tokens equal to the result": one row over every result.
fn r376_treasure_roll(sides: u32) -> Option<Effect> {
    serde_json::from_str(&format!(
        r#"{{"RollDice":{{"sides":{sides},"results":[{{"from":1,"effects":[{{"CreateToken":{{"token":{{"Named":"treasure"}},"count":{{"DieResult":"Only"}}}}}}]}}]}}}}"#
    ))
    .ok()
}

/// "roll a d4 and create a number of Treasure tokens equal to the result", read as one sentence that other sentences
/// may follow.
pub(super) fn r376_roll_treasures(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.attempt(|r| {
        r.need("roll a")?;
        let sides = r.take_word()?.strip_prefix('d')?.parse::<u32>().ok()?;
        r.r376_text("and create a number of Treasure tokens equal to the result")
            .then_some(())?;
        Some(vec![r376_treasure_roll(sides)?])
    })
}

/// The sentence of a roll [`r376_roll_treasures`] reads at the head of `effects`, and the one effect it covers.
pub(super) fn r376_roll_treasures_text(effects: &[Effect]) -> Option<(Vec<String>, usize)> {
    let Some(Effect::RollDice { sides, .. }) = effects.first() else {
        return None;
    };
    (effects.first()? == &r376_treasure_roll(*sides)?).then(|| {
        (
            vec![format!(
                "roll a d{sides} and create a number of Treasure tokens equal to the result"
            )],
            1,
        )
    })
}
