//! Whole abilities of the r55-23 card pack that no other writer words: each is matched whole as data and worded as the
//! card prints it, and the compiler reads the same wording back as the same data (`r5523_read_whole`).

use std::sync::LazyLock;

use crate::ability::Ability;

/// The abilities, as JSON, each with its wording (the card's name as `~`).
const R5523_WHOLES: [(&str, &str); 4] = [
    // weathered_sentinels
    (
        r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":"AttackLastTurnAttackersDespiteDefender"}}}}"#,
        r"This creature can attack players who attacked you during their last turn as though it didn't have defender.",
    ),
    // wolverine_best_there_is
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"if":{"Matches":{"subject":"This","selector":[{"DealtDamageThisTurn":[{"Type":"Creature"},"Other"]}]}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        r"At the beginning of each end step, if ~ dealt damage to another creature this turn, put a +1/+1 counter on him.",
    ),
    // hundred_battle_veteran
    (
        r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"from":"Graveyard","riders":[{"EntersWithCounter":{"Other":"finality"}}]}},"from":"Graveyard"}}"#,
        r"You may cast this card from your graveyard. If you do, it enters with a finality counter on it.",
    ),
    // yuna_grand_summoner
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"if":{"Compare":{"left":{"AnyCounters":"Triggering"},"op":"Ge","right":1}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"May":{"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"AnyCounters":"Triggering"}}}]}}]}}"#,
        r"Whenever another permanent you control is put into a graveyard from the battlefield, if it had one or more counters on it, you may put that number of +1/+1 counters on target creature.",
    ),
];

/// The entries parsed once.
static R5523_SHAPES: LazyLock<Vec<(Ability, &'static str)>> = LazyLock::new(|| {
    R5523_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(in crate::compile::read) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5523_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

/// The ability of the entry worded exactly as `text`.
pub(in crate::compile::read) fn r5523_read_whole(text: &str) -> Option<Ability> {
    R5523_SHAPES
        .iter()
        .find(|(_, wording)| *wording == text)
        .map(|(shape, _)| shape.clone())
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r55-23 table parses, and reads back as its own data only from its exact wording.
    use super::{R5523_SHAPES, R5523_WHOLES, r5523_read_whole};

    #[test]
    fn r5523_every_whole_parses() {
        assert_eq!(R5523_SHAPES.len(), R5523_WHOLES.len());
    }

    #[test]
    fn r5523_only_the_exact_wording_reads() {
        for (shape, text) in R5523_SHAPES.iter() {
            assert_eq!(r5523_read_whole(text).as_ref(), Some(shape));
            assert_eq!(r5523_read_whole(&text.replacen(" ", "  ", 1)), None);
        }
    }
}
