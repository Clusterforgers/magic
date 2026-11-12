//! Whole abilities of the r28-9 card pack whose steps the renderers do not join: each is matched whole as data and
//! worded as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`).
const R289_WHOLES: [(&str, &str); 23] = [
    // gideon_martial_paragon
    (
        r#"{"Loyalty":{"cost":"Zero","effects":[{"Continuous":{"affected":"This","mods":[{"AddType":"Creature"},{"SetCreatureTypes":["Human","Soldier"]},{"SetPT":{"power":5,"toughness":5}},{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}},{"Shield":{"when":{"Damage":{"to":"This"}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#,
        r"0: Until end of turn, ~ becomes a 5/5 Human Soldier creature with indestructible that's still a planeswalker. Prevent all damage that would be dealt to him this turn.",
    ),
    // rowan_fearless_sparkmage
    (
        r#"{"Loyalty":{"cost":{"Minus":2},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}},{"Continuous":{"affected":{"Target":0},"mods":["CantBlock"],"duration":"UntilEndOfTurn"}}]}}"#,
        r"−2: ~ deals 1 damage to each of up to two target creatures. Those creatures can't block this turn.",
    ),
    // elspeth_undaunted_hero
    (
        r#"{"Loyalty":{"cost":{"Minus":2},"effects":[{"Search":{"also":["Graveyard"],"filter":[{"OwnedBy":"You"},{"Named":"Sunlit Hoplite"}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{"under":"You"}}}},{"If":{"condition":{"Searched":{"zone":"Library"}},"then":[{"Shuffle":{}}]}}]}}"#,
        r"−2: Search your library and/or graveyard for a card named Sunlit Hoplite and put it onto the battlefield. If you search your library this way, shuffle.",
    ),
    // chandra_flames_catalyst
    (
        r#"{"Loyalty":{"cost":{"Minus":8},"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Draw":{"count":7}},{"Rule":{"rule":{"MayCast":{"from":"Hand","free":true}},"duration":"UntilEndOfTurn"}}]}}"#,
        r"−8: Discard your hand, then draw seven cards. Until end of turn, you may cast spells from your hand without paying their mana costs.",
    ),
    // transcendence
    (
        r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}}},"effects":[{"LoseGame":{}}]}}"#,
        r"When you have 20 or more life, you lose the game.",
    ),
    // transcendence
    (
        r#"{"Triggered":{"when":{"LifeLost":{"who":"You"}},"effects":[{"GainLife":{"amount":{"Times":{"by":2,"of":"Triggering"}}}}]}}"#,
        r"Whenever you lose life, you gain 2 life for each 1 life you lost.",
    ),
    // lichs_tomb
    (
        r#"{"Triggered":{"when":{"LifeLost":{"who":"You"}},"effects":[{"Sacrifice":{"count":"Triggering","filter":[]}}]}}"#,
        r"Whenever you lose life, sacrifice a permanent for each 1 life you lost.",
    ),
    // opal_avenger
    (
        r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":10}}}},"if":{"Matches":{"subject":"This","selector":[{"Type":"Enchantment"}]}},"effects":[{"Continuous":{"affected":"This","mods":[{"SetTypes":["Creature"]},{"SetCreatureTypes":["Soldier"]},{"SetPT":{"power":3,"toughness":5}}],"duration":"Forever"}}]}}"#,
        r"When you have 10 or less life, if this permanent is an enchantment, it becomes a 3/5 Soldier creature.",
    ),
    // thought_knot_seer
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"Reveal":{"what":{"Each":[{"OwnedBy":{"Target":0}},{"In":"Hand"}]}}},{"Choose":{"from":{"Each":[{"OwnedBy":{"Target":0}},{"In":"Hand"},{"Not":{"Type":"Land"}}]},"as":"chosen"}},{"Exile":{"what":{"Bound":"chosen"}}}]}}"#,
        r"When this creature enters, target opponent reveals their hand. You choose a nonland card from it and exile that card.",
    ),
    // singe_mind_ogre
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"ChooseAtRandom":{"from":{"Each":[{"OwnedBy":{"Target":0}},{"In":"Hand"}]},"as":"revealed"}},{"Reveal":{"what":{"Bound":"revealed"}}},{"LoseLife":{"who":{"Target":0},"amount":{"Stat":{"of":{"Bound":"revealed"},"stat":"ManaValue"}}}}]}}"#,
        r"When this creature enters, target player reveals a card at random from their hand, then loses life equal to that card's mana value.",
    ),
    // orzhov_euthanist
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"HauntedBy":"This"}],"from":"Battlefield","to":"Graveyard"}}]},"targets":[{"what":{"Object":[{"Type":"Creature"},{"DidThisTurn":{"event":"Damaged"}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
        r"When this creature enters or the creature it haunts dies, destroy target creature that was dealt damage this turn.",
    ),
    // witch_maw_nephilim
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"If":{"condition":{"Compare":{"left":{"Stat":{"of":"This","stat":"Power"}},"op":"Ge","right":10}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        r"Whenever this creature attacks, it gains trample until end of turn if its power is 10 or greater.",
    ),
    // kitnap
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Tap":{"what":"Attached"}},{"If":{"condition":{"Compare":{"left":"Promised","op":"Eq","right":0}},"then":[{"PutCounters":{"on":"Attached","counter":{"Other":"stun"},"count":3}}]}}]}}"#,
        r"When this Aura enters, tap enchanted creature. If the gift wasn't promised, put three stun counters on it.",
    ),
    // graceful_antelope
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetLandTypes":["Plains"]}],"duration":"WhileSourceOnBattlefield"}}]}}]}}"#,
        r"Whenever this creature deals combat damage to a player, you may have target land become a Plains until this creature leaves the battlefield.",
    ),
    // amalia_benavides_aguirre
    (
        r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"Explore":{"what":"This"}},{"If":{"condition":{"Compare":{"left":{"Stat":{"of":"This","stat":"Power"}},"op":"Eq","right":20}},"then":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},"Other"]}}}]}}]}}"#,
        r"Whenever you gain life, ~ explores. Then destroy all other creatures if its power is exactly 20.",
    ),
    // master_of_the_veil
    (
        r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"HasKeywordNamed":"Morph"}]}}],"effects":[{"May":{"effects":[{"TurnFaceDown":{"what":{"Target":0}}}]}}]}}"#,
        r"When this creature is turned face up, you may turn target creature with a morph ability face down.",
    ),
    // underworld_cerberus
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Exile":{"what":{"Arrived":{"of":"This","zone":"Graveyard"}}}},{"Move":{"what":{"Each":[{"Type":"Creature"},{"In":"Graveyard"}]},"to":"Hand"}}]}}"#,
        r"When this creature dies, exile it and each player returns all creature cards from their graveyard to their hand.",
    ),
    // preacher_of_the_schism
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":{"HighestLife":"EachPlayer"}}},"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#,
        r"Whenever this creature attacks while you have the most life or are tied for most life, you draw a card and you lose 1 life.",
    ),
    // absolving_lammasu
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Unsuspect":{"what":{"Each":[{"Type":"Creature"},"Suspected"]}}}]}}"#,
        r"When this creature enters, all suspected creatures are no longer suspected.",
    ),
    // surveillance_monitor
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Pay":{"cost":[{"CollectEvidence":4}]}}]}}"#,
        r"When this creature enters, you may collect evidence 4.",
    ),
    // campsite_cuisine
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"Type":"Creature"},{"Supertype":"Legendary"},{"ControlledBy":"You"}],"to":"Battlefield"}}]},"effects":[{"CreateToken":{"token":{"Named":"food"}}}]}}"#,
        r"Whenever this enchantment or a legendary creature you control enters, create a Food token.",
    ),
    // overlord_of_the_hauntwoods
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Everywhere","types":["Land"],"subtypes":["Plains","Island","Swamp","Mountain","Forest"]}},"tapped":true}}]}}"#,
        r"Whenever this permanent enters or attacks, create a tapped colorless land token named Everywhere that is every basic land type.",
    ),
    // nicol_bolas_the_deceiver
    (
        r#"{"Loyalty":{"cost":{"Plus":3},"effects":[{"RepeatEach":{"over":"Opponents","as":"each","effects":[{"OneOf":{"who":{"Bound":"each"},"options":[[{"Pay":{"who":{"Bound":"each"},"cost":[{"Sacrifice":{"filter":[{"Not":{"Type":"Land"}}]}}],"unpaid":[{"LoseLife":{"who":{"Bound":"each"},"amount":3}}]}}],[{"Pay":{"who":{"Bound":"each"},"cost":[{"Discard":{}}],"unpaid":[{"LoseLife":{"who":{"Bound":"each"},"amount":3}}]}}]]}}]}}]}}"#,
        r"+3: Each opponent loses 3 life unless that player sacrifices a nonland permanent of their choice or discards a card.",
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
