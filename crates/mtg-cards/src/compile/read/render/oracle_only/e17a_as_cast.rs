//! Spells that count what their caster controlled "as you cast this spell" (`Amount::CountedAsCast`), matched whole as
//! data and worded as the card prints it ("Steer Clear deals 4 damage to that creature instead if you controlled a Mount
//! as you cast this spell.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const E17A_AS_CAST: [(&str, &str); 5] = [
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":["Attacking","Blocking"]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"When":{"condition":{"Compare":{"left":{"CountedAsCast":{"Count":[{"Subtype":"Mount"},{"ControlledBy":"You"}]}},"op":"Ge","right":1}},"then":4,"otherwise":2}}}}]}}"#,
        "~ deals 2 damage to target attacking or blocking creature. ~ deals 4 damage to that creature instead if you controlled a Mount as you cast this spell.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":{"Times":{"by":-1,"of":{"Sum":["X",{"When":{"condition":{"Compare":{"left":{"CountedAsCast":{"Count":[{"Subtype":"Faerie"},{"ControlledBy":"You"}]}},"op":"Ge","right":1}},"then":3,"otherwise":0}}]}}},"toughness":{"Times":{"by":-1,"of":{"Sum":["X",{"When":{"condition":{"Compare":{"left":{"CountedAsCast":{"Count":[{"Subtype":"Faerie"},{"ControlledBy":"You"}]}},"op":"Ge","right":1}},"then":3,"otherwise":0}}]}}}}}]}}]}}"#,
        "Target creature gets -X/-X until end of turn. That creature gets an additional -3/-3 until end of turn if you controlled a Faerie as you cast this spell.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Sum":["X",{"When":{"condition":{"Compare":{"left":{"CountedAsCast":{"Count":[{"Type":"Creature"},"Modified",{"ControlledBy":"You"}]}},"op":"Ge","right":1}},"then":2,"otherwise":0}}]}}}]}}"#,
        "~ deals X damage to target creature or planeswalker. If you controlled a modified creature as you cast this spell, it deals X plus 2 damage instead.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":{"Times":{"by":-1,"of":{"CountedAsCast":{"Sum":[{"Count":[{"Type":"Creature"},"Modified",{"ControlledBy":"You"}]},2]}}}},"toughness":{"Times":{"by":-1,"of":{"CountedAsCast":{"Sum":[{"Count":[{"Type":"Creature"},"Modified",{"ControlledBy":"You"}]},2]}}}}}}]}}]}}"#,
        "Target creature gets -2/-2 until end of turn. It gets an additional -1/-1 until end of turn for each modified creature you controlled as you cast this spell.",
    ),
    (
        r#"{"Spell":{"effects":[{"Draw":{"count":{"CountedAsCast":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}}}},{"GainLife":{"amount":{"Product":[{"CountedAsCast":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}},{"When":{"condition":{"Matches":{"selector":[{"WasCast":{"from":"Exile"}}],"subject":"This"}},"then":2,"otherwise":0}}]}}}]}}"#,
        "Draw X cards, where X is the greatest power among creatures you controlled as you cast this spell. If this spell was cast from exile, you gain twice X life.",
    ),
];

/// The entries parsed once.
static E17A_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    E17A_AS_CAST
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in E17A_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{E17A_AS_CAST, E17A_SHAPES};

    #[test]
    fn e17a_every_as_cast_whole_parses() {
        assert_eq!(E17A_SHAPES.len(), E17A_AS_CAST.len());
    }
}
