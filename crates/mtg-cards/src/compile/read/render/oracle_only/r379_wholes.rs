//! Whole abilities of the r37-9 card pack whose steps the renderers do not join: each is matched whole as data and
//! worded as the card prints it, so another amount, side of the vote or kind of spell never reads back.

use std::sync::LazyLock;

use crate::ability::Ability;

/// The abilities, as JSON, each with its wording (the card's name as `~`).
const R379_WHOLES: [(&str, &str); 5] = [
    // timeline_culler
    (
        r#"{"AltCast":{"way":"Warp","cost":[{"Mana":"{B}"},{"PayLife":2}],"from":"Graveyard"}}"#,
        r"You may cast this card from your graveyard using its warp ability.",
    ),
    // grudge_keeper
    (
        r#"{"Triggered":{"when":{"Did":{"action":"FinishedVoting"}},"effects":[{"LoseLife":{"who":{"PlayersWhere":{"who":"Opponents","test":{"VotedAs":{"with":"Asker","same":false}}}},"amount":2}}]}}"#,
        r"Whenever players finish voting, each opponent who voted for a choice you didn't vote for loses 2 life.",
    ),
    // erestor_of_the_council
    (
        r#"{"Triggered":{"when":{"Did":{"action":"FinishedVoting"}},"effects":[{"CreateToken":{"who":{"PlayersWhere":{"who":"Opponents","test":{"VotedAs":{"with":"Asker","same":true}}}},"token":{"Named":"treasure"}}},{"Scry":{"count":{"Players":{"PlayersWhere":{"who":"Opponents","test":{"VotedAs":{"with":"Asker","same":false}}}}}}},{"Draw":{}}]}}"#,
        r"Whenever players finish voting, each opponent who voted for a choice you voted for creates a Treasure token. You scry X, where X is the number of opponents who voted for a choice you didn't vote for. Draw a card.",
    ),
    // model_of_unity
    (
        r#"{"Triggered":{"when":{"Did":{"action":"FinishedVoting"}},"effects":[{"RepeatEach":{"over":{"Union":["You",{"PlayersWhere":{"who":"Opponents","test":{"VotedAs":{"with":"Asker","same":true}}}}]},"as":"scrier","effects":[{"May":{"who":{"Bound":"scrier"},"effects":[{"Scry":{"who":{"Bound":"scrier"},"count":2}}]}}]}}]}}"#,
        r"Whenever players finish voting, you and each opponent who voted for a choice you voted for may scry 2.",
    ),
    // rootha_mastering_the_moment
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"InstantsAndSorceriesCast"}},"op":"Ge","right":1}},"effects":[{"CreateToken":{"token":{"Sized":{"size":{"GreatestCast":{"who":"You","of":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"stat":"ManaValue"}},"token":{"name":"Elemental","colors":["Blue","Red"],"types":["Creature"],"subtypes":["Elemental"],"abilities":[{"Keyword":"Flying"},{"Keyword":"Haste"}]}}}}}]}}"#,
        r"At the beginning of combat on your turn, if you've cast an instant or sorcery spell this turn, create an X/X blue and red Elemental creature token with flying and haste, where X is the greatest mana value among instant and sorcery spells you've cast this turn.",
    ),
];

/// The entries parsed once.
static R379_SHAPES: LazyLock<Vec<(Ability, &'static str)>> = LazyLock::new(|| {
    R379_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R379_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r37-9 table parses as an ability, so none is silently dropped.
    use super::{R379_SHAPES, R379_WHOLES};

    #[test]
    fn r379_every_whole_parses() {
        assert_eq!(R379_SHAPES.len(), R379_WHOLES.len());
    }
}
