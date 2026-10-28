//! Whole abilities that act on a target and on the objects that share a name, a color or a card type with it, or on
//! goaded creatures, whose wording no renderer writes: the ability is matched whole as data and worded as the card
//! prints it ("Destroy target nonland permanent and all other permanents with the same name as that permanent.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const W6D_WHOLES: [(&str, &str); 8] = [
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"effects":[{"Destroy":{"what":{"Each":[{"AnyOf":[{"Is":{"Target":0}},{"SharesName":{"Target":0}}]}]}}}]}}"#,
        "Destroy target nonland permanent and all other permanents with the same name as that permanent.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"effects":[{"Move":{"what":{"Each":[{"AnyOf":[{"Is":{"Target":0}},{"SharesName":{"Target":0}}]}]},"to":"Hand"}}]}}"#,
        "Return target nonland permanent and all other permanents with the same name as that permanent to their owners' hands.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"AnyOf":[{"Is":{"Target":0}},{"SharesName":{"Target":0}}]}]},"mods":[{"ModifyPT":{"power":-3,"toughness":-3}}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Target creature and all other creatures with the same name as that creature get -3/-3 until end of turn.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"},{"AnyOf":[{"Is":{"Target":0}},{"SharesName":{"Target":0}}]}]}}}]}}"#,
        "Exile target creature and all other creatures with the same name as that creature.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}},{"ControlledBy":"Opponents"}]}}],"effects":[{"Exile":{"what":{"Each":[{"AnyOf":[{"Is":{"Target":0}},"Token"]},{"ControlledBy":{"ControllerOf":{"Target":0}}},{"AnyOf":[{"Is":{"Target":0}},{"SharesName":{"Target":0}}]}]}}}]}}"#,
        "Exile target nonland permanent an opponent controls and all tokens that player controls with the same name as that permanent.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"source":"This","to":{"Each":[{"Type":"Creature"},{"AnyOf":[{"Is":{"Target":0}},{"SharesColor":{"Target":0}}]}]},"amount":2}}]}}"#,
        "~ deals 2 damage to target creature and each other creature that shares a color with it.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Not":{"Type":"Land"}},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Sacrifice":{"who":"Opponents","filter":[{"SharesType":"Triggering"}]}}]}}"#,
        "Whenever this enchantment or another nonland permanent you control is put into a graveyard from the battlefield, each opponent sacrifices a permanent of their choice that shares a card type with it.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":[{"Type":"Creature"},"Goaded"]}},"effects":[{"DealDamage":{"source":"Triggering","to":{"ControllerOf":"Triggering"},"amount":1}}]}}"#,
        "Whenever a goaded creature attacks, it deals 1 damage to its controller.",
    ),
];

/// The wordings with their abilities as the data writes them, read once.
static W6D_SHAPES: LazyLock<Vec<(serde_json::Value, &str)>> = LazyLock::new(|| {
    W6D_WHOLES
        .iter()
        .filter_map(|(shape, text)| {
            let ability: Ability = serde_json::from_str(shape).ok()?;
            Some((serde_json::to_value(ability).ok()?, *text))
        })
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn w6d_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(data) = serde_json::to_value(ability) else {
        return;
    };
    for (shape, text) in W6D_SHAPES.iter() {
        if *shape == data {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every whole of the table parses as an ability and reads back as its own wording only.
    use super::*;

    #[test]
    fn w6d_every_whole_parses_and_is_worded_once() {
        assert_eq!(W6D_SHAPES.len(), W6D_WHOLES.len());
        for (shape, text) in W6D_WHOLES {
            let ability: Ability = serde_json::from_str(shape).expect("whole parses");
            let mut texts = Vec::new();
            w6d_add_wholes(&ability, &mut texts);
            assert_eq!(texts, vec![text.to_owned()]);
        }
    }
}
