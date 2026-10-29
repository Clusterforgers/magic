//! Goal: the writers of the shapes the stored local-model proposals hit (r4d-b, r4b-b) give the oracle wording of the ability, and a changed clause does not read back.
use super::*;

/// The wordings the checker accepts for the rules, for the message of a failed golden.
fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const IRE: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":4,"source":"This","to":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Giant"},{"ControlledBy":"You"}]},"op":"Gt","right":0}},"then":[{"Draw":{}}]}}],"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}]}}"#;
const IRE_TWO: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Giant"},{"ControlledBy":"You"}]},"op":"Gt","right":1}},"then":[{"Draw":{}}]}}]}}"#;
const FLARE: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":1}}]}},{"Untap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}"#;
const FLARE_OTHER: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":1}}]}},{"Untap":{"what":{"Each":[{"Type":"Creature"}]}}}]}}"#;
const IMPACT: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]},"to":{"Target":0}}}],"targets":[{"what":"Player"}]}}"#;
const IMPACT_YOURS: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"to":{"Target":0}}}],"targets":[{"what":"Player"}]}}"#;
const GRASP: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}},{"GainLife":{"amount":1}}],"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"AnyOf":[{"Color":"Green"},{"Color":"White"}]}]}}]}}"#;
const CALAMITY: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"Compare":{"op":"Ge","stat":"Power","to":4}},{"Compare":{"op":"Ge","stat":"Toughness","to":4}}]}]}}]}}"#;
const CALAMITY_APART: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"Compare":{"op":"Ge","stat":"Power","to":4}},{"Compare":{"op":"Ge","stat":"Toughness","to":3}}]}]}}]}}"#;
const WRACK: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}},"source":{"Target":0},"to":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const WRACK_SOURCE: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}},"to":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const CANE: &str = r#"{"Activated":{"cost":["Tap","ExileSelf"],"effects":[{"Move":{"to":{"Library":{"place":"Shuffled"}},"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}}]}}"#;
const CANE_THEIRS: &str = r#"{"Activated":{"cost":["Tap","ExileSelf"],"effects":[{"Move":{"to":{"Library":{"place":"Shuffled"}},"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}}}]}}"#;
const GLOOMLANCE: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":{"Any":[{"Matches":{"selector":[{"Color":"Green"}],"subject":{"Target":0}}},{"Matches":{"selector":[{"Color":"White"}],"subject":{"Target":0}}}]},"then":[{"Discard":{"who":{"ControllerOf":{"Target":0}}}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const SECLUSION: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Eq","right":1}},"mods":[{"ModifyPT":{"power":3,"toughness":1}},{"AddKeyword":"Lifelink"}]}}"#;
const SECLUSION_TWO: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Eq","right":2}},"mods":[{"ModifyPT":{"power":3,"toughness":1}},{"AddKeyword":"Lifelink"}]}}"#;
const YUKORA: &str = r#"{"Triggered":{"effects":[{"Sacrifice":{"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Not":{"Subtype":"Ogre"}}]},"filter":[{"Type":"Creature"},{"ControlledBy":"You"},{"Not":{"Subtype":"Ogre"}}]}}],"when":{"ZoneChange":{"from":"Battlefield","what":["This"]}}}}"#;
const YUKORA_TWO: &str = r#"{"Triggered":{"effects":[{"Sacrifice":{"count":2,"filter":[{"Type":"Creature"},{"ControlledBy":"You"},{"Not":{"Subtype":"Ogre"}}]}}],"when":{"ZoneChange":{"from":"Battlefield","what":["This"]}}}}"#;
const TIGER: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Named":"Rock Lobster"}]},"mods":["CantAttack","CantBlock"]}}"#;

#[rstest]
#[case::greater_than_zero(
    IRE,
    "~ deals 4 damage to target player or planeswalker. If you control a Giant, draw a card."
)]
#[case::untap_those(
    FLARE,
    "Creatures you control get +2/+1 until end of turn. Untap those creatures."
)]
#[case::that_players_hand(
    IMPACT,
    "~ deals damage to target player equal to the number of cards in that player's hand."
)]
#[case::two_colors(
    GRASP,
    "Destroy target creature or planeswalker that's green or white. You gain 1 life."
)]
#[case::two_sizes(
    CALAMITY,
    "Destroy target creature with power or toughness 4 or greater."
)]
#[case::itself(WRACK, "Target creature deals damage to itself equal to its power.")]
#[case::shuffle_graveyard(CANE, "{T}, Exile ~: Shuffle your graveyard into your library.")]
#[case::was_color(
    GLOOMLANCE,
    "Destroy target creature. If that creature was green or white, its controller discards a card."
)]
#[case::exactly_one(
    SECLUSION,
    "As long as you control exactly one creature, that creature gets +3/+1 and has lifelink."
)]
#[case::sacrifice_all(
    YUKORA,
    "When ~ leaves the battlefield, sacrifice all non-Ogre creatures you control."
)]
#[case::named(TIGER, "Creatures named Rock Lobster can't attack or block.")]
fn w13a_writer_gives_the_oracle_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}: {:#?}", wordings(json));
}

#[rstest]
#[case::count_more_than_one(IRE_TWO, "If you control a Giant, draw a card.")]
#[case::another_group(
    FLARE_OTHER,
    "Creatures you control get +2/+1 until end of turn. Untap those creatures."
)]
#[case::your_hand(
    IMPACT_YOURS,
    "~ deals damage to target player equal to the number of cards in that player's hand."
)]
#[case::one_color(
    GRASP,
    "Destroy target creature or planeswalker that's green. You gain 1 life."
)]
#[case::other_colors(
    GRASP,
    "Destroy target creature or planeswalker that's red or white. You gain 1 life."
)]
#[case::sizes_apart(
    CALAMITY_APART,
    "Destroy target creature with power or toughness 4 or greater."
)]
#[case::another_source(
    WRACK_SOURCE,
    "Target creature deals damage to itself equal to its power."
)]
#[case::other_graveyard(CANE_THEIRS, "{T}, Exile ~: Shuffle your graveyard into your library.")]
#[case::shuffle_hand(CANE, "{T}, Exile ~: Shuffle your hand into your library.")]
#[case::was_one_color(
    GLOOMLANCE,
    "Destroy target creature. If that creature was green, its controller discards a card."
)]
#[case::exactly_two(
    SECLUSION_TWO,
    "As long as you control exactly one creature, that creature gets +3/+1 and has lifelink."
)]
#[case::sacrifice_two(
    YUKORA_TWO,
    "When ~ leaves the battlefield, sacrifice all non-Ogre creatures you control."
)]
#[case::other_name(TIGER, "Creatures named Paper Tiger can't attack or block.")]
fn w13a_near_miss_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
