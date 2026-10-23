//! Goal: what an Aura gives (a keyword beside a quoted ability, all creature types lost), the "When this Aura enters" and "When enchanted creature dies" triggers on its host and the "they" of an enchanted player read into the shapes of the hand-written files and write back as the card's own text.
use super::*;

#[rstest]
#[case::lifelink_and_a_draw(
    "Enchanted creature gets +1/+1 and has lifelink and \"Whenever this creature deals combat damage to a player, draw a card.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Lifelink"},{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{}}]}}}]}}"#
)]
#[case::menace_and_a_treasure(
    "Enchanted creature has menace and \"Whenever this creature deals combat damage to a player, create a Treasure token.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Menace"},{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}}]}}"#
)]
#[case::flying_and_an_activated_ability(
    "Enchanted creature has flying and \"Sacrifice a creature: This creature gets +2/+1 until end of turn.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Flying"},{"GrantAbility":{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}}]}}"#
)]
#[case::a_quoted_ability_alone(
    "Enchanted creature has \"Whenever this creature deals combat damage to a player, create a Treasure token.\"",
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}}]}}"#
)]
#[case::the_types_lost_with_a_size(
    "Nonlegendary creatures enchanted player controls have base power and toughness 3/3 and lose all creature types.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"Supertype":"Legendary"}},{"ControlledBy":"AttachedPlayer"}]},"mods":[{"SetPT":{"power":3,"toughness":3}},{"SetCreatureTypes":[]}]}}"#
)]
#[case::the_types_lost_alone(
    "Creatures enchanted player controls lose all creature types.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"AttachedPlayer"}]},"mods":[{"SetCreatureTypes":[]}]}}"#
)]
fn what_an_aura_gives_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::a_counter_on_the_host(
    "When this Aura enters, put a +1/+1 counter on enchanted creature.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"Attached","counter":"+1/+1"}}]}}"#
)]
#[case::the_host_taps(
    "When this Aura enters, tap enchanted creature.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Tap":{"what":"Attached"}}]}}"#
)]
#[case::the_host_fights_up_to_one(
    "When this Aura enters, enchanted creature fights up to one target creature you don't control.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]},"bounds":{"min":0,"max":1}}],"effects":[{"Fight":{"what":"Attached","with":{"Target":0}}}]}}"#
)]
#[case::the_host_fights_one(
    "When this Aura enters, enchanted creature fights target creature you don't control.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"Fight":{"what":"Attached","with":{"Target":0}}}]}}"#
)]
#[case::the_host_dies_with_when(
    "When enchanted creature dies, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::the_host_dies_with_whenever(
    "Whenever enchanted creature dies, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::they_lose_life(
    "Whenever enchanted player draws a card, they lose 2 life and you gain 2 life.",
    r#"{"Triggered":{"when":{"Drew":{"who":"AttachedPlayer"}},"effects":[{"LoseLife":{"who":"Triggering","amount":2}},{"GainLife":{"amount":2}}]}}"#
)]
#[case::that_player_loses_life(
    "Whenever enchanted player draws a card, that player loses 2 life and you gain 2 life.",
    r#"{"Triggered":{"when":{"Drew":{"who":"AttachedPlayer"}},"effects":[{"LoseLife":{"who":"Triggering","amount":2}},{"GainLife":{"amount":2}}]}}"#
)]
fn a_trigger_on_the_host_or_the_enchanted_player_reads_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::a_spell_has_no_host("Tap enchanted creature.", ClauseKind::Spell)]
#[case::a_group_that_fights(
    "When this Aura enters, enchanted creature fights each creature you don't control.",
    ClauseKind::Triggered
)]
#[case::two_keywords_and_a_quote(
    "Enchanted creature has flying and haste and \"Whenever this creature attacks, draw a card.\"",
    ClauseKind::Static
)]
#[case::they_without_a_player(
    "Whenever a creature enchanted player controls dies, they lose 2 life.",
    ClauseKind::Triggered
)]
fn near_misses_stay_with_the_table(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
