//! Whole abilities of the r28-9 card pack whose steps the renderers do not join: each is matched whole as data and
//! worded as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`).
const R289_WHOLES: [(&str, &str); 8] = [
    // gideon_martial_paragon
    (
        r#"{"Loyalty":{"cost":"Zero","effects":[{"Continuous":{"affected":"This","mods":[{"AddType":"Creature"},{"SetCreatureTypes":["Human","Soldier"]},{"SetPT":{"power":5,"toughness":5}},{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}},{"Shield":{"when":{"Damage":{"to":"This"}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#,
        r#"0: Until end of turn, ~ becomes a 5/5 Human Soldier creature with indestructible that's still a planeswalker. Prevent all damage that would be dealt to him this turn."#,
    ),
    // rowan_fearless_sparkmage
    (
        r#"{"Loyalty":{"cost":{"Minus":2},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}},{"Continuous":{"affected":{"Target":0},"mods":["CantBlock"],"duration":"UntilEndOfTurn"}}]}}"#,
        r#"−2: ~ deals 1 damage to each of up to two target creatures. Those creatures can't block this turn."#,
    ),
    // elspeth_undaunted_hero
    (
        r#"{"Loyalty":{"cost":{"Minus":2},"effects":[{"Search":{"also":["Graveyard"],"filter":[{"OwnedBy":"You"},{"Named":"Sunlit Hoplite"}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{"under":"You"}}}},{"If":{"condition":{"Searched":{"zone":"Library"}},"then":[{"Shuffle":{}}]}}]}}"#,
        r#"−2: Search your library and/or graveyard for a card named Sunlit Hoplite and put it onto the battlefield. If you search your library this way, shuffle."#,
    ),
    // chandra_flames_catalyst
    (
        r#"{"Loyalty":{"cost":{"Minus":8},"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Draw":{"count":7}},{"Rule":{"rule":{"MayCast":{"from":"Hand","free":true}},"duration":"UntilEndOfTurn"}}]}}"#,
        r#"−8: Discard your hand, then draw seven cards. Until end of turn, you may cast spells from your hand without paying their mana costs."#,
    ),
    // transcendence
    (
        r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}}},"effects":[{"LoseGame":{}}]}}"#,
        r#"When you have 20 or more life, you lose the game."#,
    ),
    // transcendence
    (
        r#"{"Triggered":{"when":{"LifeLost":{"who":"You"}},"effects":[{"GainLife":{"amount":{"Times":{"by":2,"of":"Triggering"}}}}]}}"#,
        r#"Whenever you lose life, you gain 2 life for each 1 life you lost."#,
    ),
    // lichs_tomb
    (
        r#"{"Triggered":{"when":{"LifeLost":{"who":"You"}},"effects":[{"Sacrifice":{"count":"Triggering","filter":[]}}]}}"#,
        r#"Whenever you lose life, sacrifice a permanent for each 1 life you lost."#,
    ),
    // opal_avenger
    (
        r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":10}}}},"if":{"Matches":{"subject":"This","selector":[{"Type":"Enchantment"}]}},"effects":[{"Continuous":{"affected":"This","mods":[{"SetTypes":["Creature"]},{"SetCreatureTypes":["Soldier"]},{"SetPT":{"power":3,"toughness":5}}],"duration":"Forever"}}]}}"#,
        r#"When you have 10 or less life, if this permanent is an enchantment, it becomes a 3/5 Soldier creature."#,
    ),
    // END
];

/// The entries parsed once.
static R289_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    R289_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r289_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R289_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r28-9 table parses as an ability, so none is silently dropped.
    use super::{R289_SHAPES, R289_WHOLES};

    #[test]
    fn r289_every_whole_parses() {
        assert_eq!(R289_SHAPES.len(), R289_WHOLES.len());
    }
}
