//! Goal: an artifact animated by its mana value, the optional blow that takes the creature's combat damage, a bonus to any red source and a group taken whose permanents are untapped and hasted read back as the oracle says them; two triggers or two shields for either side of a fight read as one sentence ("blocks or becomes blocked by a creature", "attacks or blocks", "dealt to and dealt by"); another stat, amount, kind, reference, duration, controller or shield never reads back, nor a group read by control named again after it changed control.
use super::*;
use crate::compile::read::either_way_pair_texts;

/// Whether some wording the checker accepts for the ability is the clause.
fn f25_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// Whether some wording of the two abilities as one sentence is the clause.
fn f25_pair(first: &str, second: &str, clause: &str) -> bool {
    either_way_pair_texts(&ability(first), &ability(second))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const F25_KARNS_TOUCH: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetPT":{"power":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}},"toughness":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}},{"AddType":"Artifact"},{"AddType":"Creature"}]}}],"targets":[{"what":{"Object":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}}]}}]}}"#;
const F25_KARN_SILVER_GOLEM: &str = r#"{"Activated":{"cost":"{1}","effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetPT":{"power":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}},"toughness":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}},{"AddType":"Artifact"},{"AddType":"Creature"}]}}],"targets":[{"what":{"Object":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}}]}}]}}"#;
const F25_DWARVEN_VIGILANTES: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"DealDamage":{"amount":{"Stat":{"of":"This","stat":"Power"}},"to":{"Target":0}}},{"Rule":{"duration":"UntilEndOfTurn","rule":{"Combat":{"quirk":"AssignsNoCombatDamage","who":"This"}}}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"when":{"Unblocked":{"what":["This"]}}}}"#;
const F25_LACCOLITH_GRUNT: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"DealDamage":{"amount":{"Stat":{"of":"This","stat":"Power"}},"to":{"Target":0}}},{"Rule":{"duration":"UntilEndOfTurn","rule":{"Combat":{"quirk":"AssignsNoCombatDamage","who":"This"}}}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"when":{"BecomesBlocked":{"what":["This"]}}}}"#;
const F25_EMBERMAW_HELLION: &str = r#"{"Replacement":{"by":{"Add":1},"when":{"Damage":{"source":[{"Color":"Red"},"Other",{"ControlledBy":"You"}]}}}}"#;
const F25_BROADCAST_TAKEOVER: &str = r#"{"Spell":{"effects":[{"Bind":{"what":{"Each":[{"Type":"Artifact"},{"ControlledBy":"Opponents"}]},"as":"taken"}},{"Continuous":{"affected":{"Bound":"taken"},"duration":"UntilEndOfTurn","mods":[{"SetController":"You"}]}},{"Untap":{"what":{"Bound":"taken"}}},{"Continuous":{"affected":{"Bound":"taken"},"duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Haste"}]}}]}}"#;
const F25_ROWAN_ULTIMATE: &str = r#"{"Loyalty":{"cost":{"Minus":9},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"duration":"UntilEndOfTurn","mods":[{"SetController":"You"}]}},{"Untap":{"what":{"Each":[{"Type":"Creature"}]}}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Haste"}]}}]}}"#;
/// Broadcast Takeover as it read before: the opponents' artifacts named again after they changed control.
const F25_OPPONENTS_AGAIN: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Artifact"},{"ControlledBy":"Opponents"}]},"duration":"UntilEndOfTurn","mods":[{"SetController":"You"}]}},{"Untap":{"what":{"Each":[{"Type":"Artifact"},{"ControlledBy":"Opponents"}]}}},{"Continuous":{"affected":{"Each":[{"Type":"Artifact"},{"ControlledBy":"Opponents"}]},"duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Haste"}]}}]}}"#;

#[test]
fn f25_a_group_read_by_control_is_not_them_when_named_again() {
    assert!(!f25_written(
        F25_OPPONENTS_AGAIN,
        "Gain control of all artifacts your opponents control until end of turn. Untap them. They gain haste until end of turn."
    ));
}

#[rstest]
#[case::karns_touch(
    F25_KARNS_TOUCH,
    "Target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value until end of turn."
)]
#[case::karn_silver_golem(
    F25_KARN_SILVER_GOLEM,
    "{1}: Target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value until end of turn."
)]
#[case::dwarven_vigilantes(
    F25_DWARVEN_VIGILANTES,
    "Whenever ~ attacks and isn't blocked, you may have it deal damage equal to its power to target creature. If you do, ~ assigns no combat damage this turn."
)]
#[case::laccolith_grunt(
    F25_LACCOLITH_GRUNT,
    "Whenever ~ becomes blocked, you may have it deal damage equal to its power to target creature. If you do, ~ assigns no combat damage this turn."
)]
#[case::embermaw_hellion(
    F25_EMBERMAW_HELLION,
    "If another red source you control would deal damage to a permanent or player, it deals that much damage plus 1 to that permanent or player instead."
)]
#[case::broadcast_takeover(
    F25_BROADCAST_TAKEOVER,
    "Gain control of all artifacts your opponents control until end of turn. Untap them. They gain haste until end of turn."
)]
#[case::rowan_fearless_sparkmage(
    F25_ROWAN_ULTIMATE,
    "−9: Gain control of all creatures until end of turn. Untap them. They gain haste until end of turn."
)]
fn f25_the_clause_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(f25_written(json, clause), "{:?}", written_variants(&ability(json)));
}

#[rstest]
#[case::karn_by_power(F25_KARNS_TOUCH, "\"stat\":\"ManaValue\"", "\"stat\":\"Power\"")]
#[case::karn_for_good(F25_KARNS_TOUCH, "UntilEndOfTurn", "Forever")]
#[case::vigilantes_fixed_blow(F25_DWARVEN_VIGILANTES, "{\"Stat\":{\"of\":\"This\",\"stat\":\"Power\"}}", "2")]
#[case::vigilantes_keeps_its_damage(F25_DWARVEN_VIGILANTES, "AssignsNoCombatDamage", "DoesntUntap")]
#[case::hellion_of_creatures(F25_EMBERMAW_HELLION, "{\"Color\":\"Red\"}", "{\"Type\":\"Creature\"}")]
#[case::hellion_to_players(F25_EMBERMAW_HELLION, "\"source\"", "\"to\":\"EachPlayer\",\"source\"")]
#[case::broadcast_for_an_opponent(F25_BROADCAST_TAKEOVER, "\"SetController\":\"You\"", "\"SetController\":\"Opponents\"")]
#[case::broadcast_untaps_another_group(F25_BROADCAST_TAKEOVER, "{\"Untap\":{\"what\":{\"Bound\":\"taken\"}}}", "{\"Untap\":{\"what\":{\"Each\":[{\"Type\":\"Artifact\"},{\"ControlledBy\":\"Opponents\"}]}}}")]
#[case::broadcast_haste_for_good(F25_BROADCAST_TAKEOVER, "\"UntilEndOfTurn\",\"mods\":[{\"AddKeyword\"", "\"Forever\",\"mods\":[{\"AddKeyword\"")]
fn f25_another_wording_does_not_read_back(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let clauses = [
        "Target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value until end of turn.",
        "Whenever ~ attacks and isn't blocked, you may have it deal damage equal to its power to target creature. If you do, ~ assigns no combat damage this turn.",
        "If another red source you control would deal damage to a permanent or player, it deals that much damage plus 1 to that permanent or player instead.",
        "Gain control of all artifacts your opponents control until end of turn. Untap them. They gain haste until end of turn.",
    ];
    let changed = json.replacen(from, to, 1);
    assert_ne!(changed, json, "the case changes the rules");
    if serde_json::from_str::<Ability>(&changed).is_err() {
        return;
    }
    for clause in clauses {
        assert!(!f25_written(&changed, clause), "{changed} reads {clause}");
    }
}

const F25_ASHMOUTH_BLOCKS: &str = r#"{"Triggered":{"effects":[{"DealDamage":{"amount":1,"to":"Triggering"}}],"when":{"BecomesBlocked":{"by":["This"],"what":[{"Type":"Creature"}]}}}}"#;
const F25_ASHMOUTH_BLOCKED: &str = r#"{"Triggered":{"effects":[{"DealDamage":{"amount":1,"to":"Dealer"}}],"when":{"BecomesBlocked":{"by":[{"Type":"Creature"}],"what":["This"]}}}}"#;
const F25_ALPHAS_BLOCKS: &str = r#"{"Triggered":{"effects":[{"Together":{"effects":[{"DealDamage":{"amount":3,"to":"Triggering"}},{"DealDamage":{"amount":3,"to":{"ControllerOf":"Triggering"}}}]}}],"when":{"BecomesBlocked":{"by":["This"],"what":[{"Type":"Creature"}]}}}}"#;
const F25_ALPHAS_BLOCKED: &str = r#"{"Triggered":{"effects":[{"Together":{"effects":[{"DealDamage":{"amount":3,"to":"Dealer"}},{"DealDamage":{"amount":3,"to":{"ControllerOf":"Dealer"}}}]}}],"when":{"BecomesBlocked":{"by":[{"Type":"Creature"}],"what":["This"]}}}}"#;
const F25_BOND_ATTACKS: &str = r#"{"Triggered":{"effects":[{"LoseLife":{"amount":3,"who":{"ControllerOf":"Triggering"}}}],"when":{"Attacks":{"what":[{"Is":"Attached"}]}}}}"#;
const F25_BOND_BLOCKS: &str = r#"{"Triggered":{"effects":[{"LoseLife":{"amount":3,"who":{"ControllerOf":"Dealer"}}}],"when":{"Blocks":{"what":[{"Is":"Attached"}]}}}}"#;
const F25_GASEOUS_TO: &str = r#"{"Replacement":{"by":"Prevent","when":{"Damage":{"combat":true,"to":"Attached"}}}}"#;
const F25_GASEOUS_BY: &str = r#"{"Replacement":{"by":"Prevent","when":{"Damage":{"combat":true,"source":[{"Type":"Creature"},{"Is":"Attached"}]}}}}"#;
const F25_HEART_TO: &str = r#"{"Replacement":{"by":"Prevent","when":{"Damage":{"to":"Attached"}}}}"#;
const F25_HEART_BY: &str = r#"{"Replacement":{"by":"Prevent","when":{"Damage":{"source":[{"Type":"Creature"},{"Is":"Attached"}]}}}}"#;

#[rstest]
#[case::ashmouth_hound(
    F25_ASHMOUTH_BLOCKS,
    F25_ASHMOUTH_BLOCKED,
    "Whenever ~ blocks or becomes blocked by a creature, ~ deals 1 damage to that creature."
)]
#[case::ashmouth_hound_the_other_order(
    F25_ASHMOUTH_BLOCKED,
    F25_ASHMOUTH_BLOCKS,
    "Whenever ~ blocks or becomes blocked by a creature, ~ deals 1 damage to that creature."
)]
#[case::assembled_alphas(
    F25_ALPHAS_BLOCKS,
    F25_ALPHAS_BLOCKED,
    "Whenever ~ blocks or becomes blocked by a creature, ~ deals 3 damage to that creature and 3 damage to that creature's controller."
)]
#[case::contaminated_bond(
    F25_BOND_ATTACKS,
    F25_BOND_BLOCKS,
    "Whenever enchanted creature attacks or blocks, its controller loses 3 life."
)]
#[case::gaseous_form(
    F25_GASEOUS_TO,
    F25_GASEOUS_BY,
    "Prevent all combat damage that would be dealt to and dealt by enchanted creature."
)]
#[case::heart_of_light(
    F25_HEART_TO,
    F25_HEART_BY,
    "Prevent all damage that would be dealt to and dealt by enchanted creature."
)]
fn f25_the_pair_reads_as_one_sentence(
    #[case] first: &str,
    #[case] second: &str,
    #[case] clause: &str,
) {
    assert!(
        f25_pair(first, second, clause),
        "{:?}",
        either_way_pair_texts(&ability(first), &ability(second))
    );
}

#[rstest]
#[case::another_amount(F25_ASHMOUTH_BLOCKS, F25_ASHMOUTH_BLOCKED, "\"amount\":1", "\"amount\":2")]
#[case::the_blocked_names_itself(F25_ASHMOUTH_BLOCKS, F25_ASHMOUTH_BLOCKED, "\"Dealer\"", "\"This\"")]
#[case::a_kind_for_one_event(F25_ASHMOUTH_BLOCKS, F25_ASHMOUTH_BLOCKED, "[{\"Type\":\"Creature\"}],\"what\"", "[{\"Subtype\":\"Wall\"}],\"what\"")]
#[case::once_for_all_blockers(F25_ASHMOUTH_BLOCKS, F25_ASHMOUTH_BLOCKED, "\"what\":[\"This\"]", "\"what\":[\"This\"],\"once\":true")]
#[case::alphas_hits_the_other_player(F25_ALPHAS_BLOCKS, F25_ALPHAS_BLOCKED, "{\"ControllerOf\":\"Dealer\"}", "\"You\"")]
#[case::bond_hits_you(F25_BOND_ATTACKS, F25_BOND_BLOCKS, "{\"ControllerOf\":\"Dealer\"}", "\"You\"")]
#[case::bond_of_another_creature(F25_BOND_ATTACKS, F25_BOND_BLOCKS, "{\"Is\":\"Attached\"}", "\"This\"")]
#[case::gaseous_by_any_creature(F25_GASEOUS_TO, F25_GASEOUS_BY, ",{\"Is\":\"Attached\"}", "")]
#[case::gaseous_one_shield_of_all_damage(F25_GASEOUS_TO, F25_GASEOUS_BY, "\"combat\":true,\"source\"", "\"source\"")]
#[case::heart_halves_the_damage(F25_HEART_TO, F25_HEART_BY, "\"by\":\"Prevent\"", "\"by\":{\"Add\":1}")]
fn f25_another_pair_does_not_read_as_one(
    #[case] first: &str,
    #[case] second: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let clauses = [
        "Whenever ~ blocks or becomes blocked by a creature, ~ deals 1 damage to that creature.",
        "Whenever ~ blocks or becomes blocked by a creature, ~ deals 3 damage to that creature and 3 damage to that creature's controller.",
        "Whenever enchanted creature attacks or blocks, its controller loses 3 life.",
        "Prevent all combat damage that would be dealt to and dealt by enchanted creature.",
        "Prevent all damage that would be dealt to and dealt by enchanted creature.",
    ];
    let changed = second.replacen(from, to, 1);
    assert_ne!(changed, second, "the case changes the second ability");
    if serde_json::from_str::<Ability>(&changed).is_err() {
        return;
    }
    for clause in clauses {
        assert!(!f25_pair(first, &changed, clause), "{changed} reads {clause}");
        assert!(!f25_pair(&changed, first, clause), "{changed} reads {clause}");
    }
}
