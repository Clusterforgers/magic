//! Whole texts, secret choices most of them: texts whose sentences only make sense together ("each player secretly chooses a number.
//! Then those numbers are revealed. Each player with the highest number loses that much life."), each read as a fixed
//! list of effects and written back from it word for word.

use super::tokens::Reader;
use crate::effect::Effect;

/// One text and the effects it reads as, in canonical JSON.
struct Whole {
    /// The sentences, each without its final period, as the card prints them after the trigger's head.
    sentences: &'static [&'static str],
    /// The effects of the sentences.
    rules: &'static str,
}

/// Menacing Ogre: the players with the highest secret number lose that much life.
const HIGHEST_NUMBER: Whole = Whole {
    sentences: &[
        "each player secretly chooses a number",
        "then those numbers are revealed",
        "each player with the highest number loses that much life",
        "if you are one of those players, put two +1/+1 counters on this creature",
    ],
    rules: r#"[{"ChooseNumbers":{"secret":true,"as":"number","highest":"highest","lowest":"lowest"}},{"LoseLife":{"who":{"Bound":"highest"},"amount":{"ChosenAs":"number"}}},{"If":{"condition":{"Among":{"Bound":"highest"}},"then":[{"PutCounters":{"on":"This","counter":"+1/+1","count":2}}]}}]"#,
};

/// Círdan the Shipwright: each player draws for the secret votes they got, and those who got none may put a permanent
/// card onto the battlefield.
const VOTES_DRAW: Whole = Whole {
    sentences: &[
        "each player secretly votes for a player, then those votes are revealed",
        "each player draws a card for each vote they received",
        "each player who received no votes may put a permanent card from their hand onto the battlefield",
    ],
    rules: r#"[{"Vote":{"options":{"Of":"EachPlayer"},"secret":true,"tally":"votes"}},{"RepeatEach":{"over":"EachPlayer","as":"voter","effects":[{"Draw":{"who":{"Bound":"voter"},"count":{"Players":{"PlayersWhere":{"who":{"Bound":"votes"},"test":{"Among":{"Bound":"voter"}}}}}}}]}},{"RepeatEach":{"over":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":{"Bound":"votes"}}}}},"as":"unvoted","effects":[{"Choose":{"who":{"Bound":"unvoted"},"from":{"Each":[{"OwnedBy":{"Bound":"unvoted"}},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"In":"Hand"}]},"count":{"min":0,"max":1},"as":"picked"}},{"Move":{"what":{"Bound":"picked"},"to":{"Battlefield":{}}}}]}}]"#,
};

/// Mob Verdict: each opponent is dealt 2 damage, with each creature they control, for each secret vote they got, and
/// the caster draws for each of theirs.
const VOTES_DAMAGE: Whole = Whole {
    sentences: &[
        "each player secretly votes for another player, then those votes are revealed",
        "for each vote an opponent received, ~ deals 2 damage to that player and each creature that player controls",
        "for each vote you received, draw a card",
    ],
    rules: r#"[{"Vote":{"options":"AnotherPlayer","secret":true,"tally":"votes"}},{"RepeatEach":{"over":"Opponents","as":"voter","effects":[{"DealDamage":{"to":{"Union":[{"Bound":"voter"},{"Each":[{"Type":"Creature"},{"ControlledBy":{"Bound":"voter"}}]}]},"amount":{"Times":{"by":2,"of":{"Players":{"PlayersWhere":{"who":{"Bound":"votes"},"test":{"Among":{"Bound":"voter"}}}}}}}}}]}},{"Draw":{"count":{"Players":{"PlayersWhere":{"who":{"Bound":"votes"},"test":{"Among":"Asker"}}}}}}]"#,
};

/// Prisoner's Dilemma: the opponents' secret choices of silence or snitch decide the damage.
const SILENCE_OR_SNITCH: Whole = Whole {
    sentences: &[
        "each opponent secretly chooses silence or snitch, then the choices are revealed",
        "if each opponent chose silence, ~ deals 4 damage to each of them",
        "if each opponent chose snitch, ~ deals 8 damage to each of them",
        "otherwise, ~ deals 12 damage to each opponent who chose silence",
    ],
    rules: r#"[{"ChooseWords":{"who":"Opponents","words":["silence","snitch"],"secret":true}},{"If":{"condition":{"Compare":{"left":{"Players":{"Bound":"silence"}},"op":"Eq","right":{"Players":"Opponents"}}},"then":[{"DealDamage":{"to":"Opponents","amount":4}}],"otherwise":[{"If":{"condition":{"Compare":{"left":{"Players":{"Bound":"snitch"}},"op":"Eq","right":{"Players":"Opponents"}}},"then":[{"DealDamage":{"to":"Opponents","amount":8}}],"otherwise":[{"DealDamage":{"to":{"Bound":"silence"},"amount":12}}]}}]}}]"#,
};

/// Cryptic Gateway: a creature card that shares a creature type with each creature tapped to pay the cost.
const SHARED_TYPE: Whole = Whole {
    sentences: &[
        "you may put a creature card from your hand that shares a creature type with each creature tapped this way onto the battlefield",
    ],
    rules: r#"[{"Choose":{"from":{"Each":[{"OwnedBy":"You"},{"Type":"Creature"},{"In":"Hand"},{"SharesCreatureTypeWithEach":"Paid"}]},"count":{"min":0,"max":1},"as":"picked"}},{"Move":{"what":{"Bound":"picked"},"to":{"Battlefield":{}}}}]"#,
};

/// Teyo, Geometric Tactician: until the controller's next turn, attacks go only to the nearest opponent on the side
/// chosen.
const NEAREST_UNTIL_NEXT_TURN: Whole = Whole {
    sentences: &[
        "choose left or right",
        "until your next turn, each player may attack only the nearest opponent in the last chosen direction and planeswalkers controlled by that opponent",
    ],
    rules: r#"[{"ChooseValue":{"kind":{"Options":["left","right"]},"as":"direction"}},{"Rule":{"rule":{"AttackOnlyNearest":{"ChosenAs":"direction"}},"duration":"UntilYourNextTurn"}}]"#,
};

/// The texts, tried in this order.
const WHOLES: [Whole; 6] = [
    HIGHEST_NUMBER,
    VOTES_DRAW,
    VOTES_DAMAGE,
    SILENCE_OR_SNITCH,
    SHARED_TYPE,
    NEAREST_UNTIL_NEXT_TURN,
];

/// The effects of a text.
fn effects(whole: &Whole) -> Option<Vec<Effect>> {
    serde_json::from_str(whole.rules).ok()
}

/// Reads one of the texts whole, leaving its final period.
pub(super) fn r376_secret(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    WHOLES.iter().find_map(|whole| {
        r.attempt(|r| {
            let (last, head) = whole.sentences.split_last()?;
            for sentence in head {
                r.r376_text(sentence).then_some(())?;
                r.punct('.').then_some(())?;
            }
            r.r376_text(last).then_some(())?;
            effects(whole)
        })
    })
}

/// The sentences of the effects at the head of `effects` when they are those of one of the texts, and how many they
/// cover.
pub(super) fn r376_secret_text(effects_written: &[Effect]) -> Option<(Vec<String>, usize)> {
    WHOLES.iter().find_map(|whole| {
        let wanted = effects(whole)?;
        (effects_written.get(..wanted.len())? == wanted.as_slice()).then(|| {
            (
                whole
                    .sentences
                    .iter()
                    .map(|sentence| (*sentence).to_owned())
                    .collect(),
                wanted.len(),
            )
        })
    })
}
