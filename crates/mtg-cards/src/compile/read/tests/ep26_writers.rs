//! Goal: the writers of ep26 write the card's words and nothing near them: a target "chosen at random" ("deals 2 damage to any target chosen at random", "deals 2 damage to target opponent chosen at random") reads back only when the rules draw it, and a target the controller chooses does not read as one drawn at random. "Exile a creature you control" as a cost reads back and is read by the compiler; a graveyard, another type, another count or a sacrifice does not. Mana of any of the exiled card's colors (an imprint) and of the exiled cards' colors (every card exiled with it), and several targets "from graveyards"; another slot, zone or source does not read back. "Discard your hand, Sacrifice ~: ... Activate only as an instant." in either order of the cost; another timing, one card or another zone does not. "The next (creature, instant or sorcery) spell you cast this turn can't be countered."; another kind, duration or benefit does not. "You may cast ~ from your graveyard or from exile." of the two permissions in that order; another order, zone or card does not.
use super::*;

fn activated(target: &str, effect: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"targets":[{target}],"effects":[{effect}]}}}}"#
    ))
}

const DAMAGE: &str = r#"{"DealDamage":{"amount":2,"to":{"Target":0}}}"#;
const RANDOM_ANY: &str = r#"{"what":{"AtRandom":"AnyTarget"}}"#;
const ANY: &str = r#"{"what":"AnyTarget"}"#;
const RANDOM_FOE: &str = r#"{"what":{"AtRandom":"Opponent"}}"#;
const FOE: &str = r#"{"what":"Opponent"}"#;
const PILOT: &str = "{T}: ~ deals 2 damage to any target chosen at random.";
const RAMPAGER: &str = "{T}: ~ deals 2 damage to target opponent chosen at random.";

#[rstest]
#[case::pilot(activated(RANDOM_ANY, DAMAGE), PILOT, true)]
#[case::pilot_choosing(activated(ANY, DAMAGE), PILOT, false)]
#[case::pilot_plain_text(
    activated(RANDOM_ANY, DAMAGE),
    "{T}: ~ deals 2 damage to any target.",
    false
)]
#[case::opponent(activated(RANDOM_FOE, DAMAGE), RAMPAGER, true)]
#[case::opponent_choosing(activated(FOE, DAMAGE), RAMPAGER, false)]
#[case::any_target_for_an_opponent(activated(RANDOM_ANY, DAMAGE), RAMPAGER, false)]
fn ep26_targets_chosen_at_random_write_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn draws_for(cost: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{B}}"}},{cost}],"effects":[{{"Draw":{{}}}}]}}}}"#
    ))
}

const EXILE_CREATURE: &str = r#"{"ExilePermanents":{"filter":[{"Type":"Creature"}]}}"#;
const FUMES: &str = "{B}, Exile a creature you control: Draw a card.";

#[rstest]
#[case::a_creature_you_control(draws_for(EXILE_CREATURE), FUMES, true)]
#[case::from_the_graveyard(
    draws_for(r#"{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}]}}"#),
    FUMES,
    false
)]
#[case::an_artifact(
    draws_for(r#"{"ExilePermanents":{"filter":[{"Type":"Artifact"}]}}"#),
    FUMES,
    false
)]
#[case::two_creatures(
    draws_for(r#"{"ExilePermanents":{"filter":[{"Type":"Creature"}],"count":2}}"#),
    FUMES,
    false
)]
#[case::sacrificed(
    draws_for(r#"{"Sacrifice":{"filter":[{"Type":"Creature"}]}}"#),
    FUMES,
    false
)]
fn ep26_exiling_a_permanent_you_control_writes_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[rstest]
#[case::read("{B}, Exile a creature you control: Draw a card.", true)]
#[case::graveyard_stays_graveyard(
    "{B}, Exile a creature card from your graveyard: Draw a card.",
    false
)]
fn ep26_the_compiler_reads_the_exile_cost(#[case] text: &str, #[case] permanent: bool) {
    let read = clause(text, ClauseKind::Activated, false).expect(text);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert_eq!(json.contains(EXILE_CREATURE), permanent, "{json}");
}

fn tap_for(among: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"AnyColorAmong":{among}}}}}}}]}}}}"#
    ))
}

const IMPRINTED: &str =
    r#"[{"Is":{"Remembered":{"on":"This","slot":"imprinted"}}},{"In":"Exile"}]"#;
const EXILED_WITH: &str = r#"[{"ExiledWith":"This"},{"In":"Exile"}]"#;
const MOX: &str = "{T}: Add one mana of any of the exiled card's colors.";
const PIT: &str = "{T}: Add one mana of any of the exiled cards' colors.";

#[rstest]
#[case::mox(IMPRINTED, MOX, true)]
#[case::mox_of_every_exiled_card(EXILED_WITH, MOX, false)]
#[case::mox_in_the_graveyard(
    r#"[{"Is":{"Remembered":{"on":"This","slot":"imprinted"}}},{"In":"Graveyard"}]"#,
    MOX,
    false
)]
#[case::mox_of_another_slot(
    r#"[{"Is":{"Remembered":{"on":"This","slot":"chosen"}}},{"In":"Exile"}]"#,
    MOX,
    false
)]
#[case::pit(EXILED_WITH, PIT, true)]
#[case::pit_of_the_imprint(IMPRINTED, PIT, false)]
#[case::pit_of_another_source(r#"[{"ExiledWith":"Attached"},{"In":"Exile"}]"#, PIT, false)]
fn ep26_mana_of_the_exiled_cards_colors_writes_as_printed(
    #[case] among: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&tap_for(among));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const PIT_ENTERS: &str = r#"{"Triggered":{"effects":[{"Exile":{"what":{"Target":0}}}],"targets":[{"bounds":{"max":3,"min":0},"what":{"Object":[{"In":"Graveyard"}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;

#[rstest]
#[case::several_from_graveyards(
    "When ~ enters, exile up to three target cards from graveyards.",
    true
)]
#[case::several_from_a_graveyard(
    "When ~ enters, exile up to three target cards from a graveyard.",
    false
)]
fn ep26_several_targets_come_from_graveyards(#[case] text: &str, #[case] reads: bool) {
    let found = written_variants(&ability(PIT_ENTERS));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const HAND: &str = r#"{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}"#;

fn diamond(cost: &str, timing: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":{cost},"effects":[{{"AddMana":{{"mana":{{"AnyColor":3}}}}}}]{timing}}}}}"#
    ))
}

const AS_INSTANT: &str = r#","timing":"AsInstant""#;
const LED: &str =
    "Discard your hand, Sacrifice ~: Add three mana of any one color. Activate only as an instant.";

#[rstest]
#[case::diamond(diamond(&format!(r#"["SacrificeSelf",{HAND}]"#), AS_INSTANT), LED, true)]
#[case::diamond_in_print_order(diamond(&format!(r#"[{HAND},"SacrificeSelf"]"#), AS_INSTANT), LED, true)]
#[case::at_any_time(diamond(&format!(r#"["SacrificeSelf",{HAND}]"#), ""), LED, false)]
#[case::as_a_sorcery(diamond(&format!(r#"["SacrificeSelf",{HAND}]"#), r#","timing":"Sorcery""#), LED, false)]
#[case::one_card(diamond(r#"["SacrificeSelf",{"Discard":{}}]"#, AS_INSTANT), LED, false)]
#[case::the_graveyard(diamond(r#"["SacrificeSelf",{"Discard":{"count":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}}]"#, AS_INSTANT), LED, false)]
fn ep26_discard_your_hand_as_an_instant_writes_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn next_spell(what: &str, benefit: &str, duration: &str) -> Ability {
    ability(&format!(
        r#"{{"Spell":{{"effects":[{{"Rule":{{"duration":"{duration}","rule":{{"NextSpell":{{"benefit":{benefit}{what}}}}}}}}}]}}}}"#
    ))
}

const UNCOUNTERABLE: &str = r#""Uncounterable""#;
const CREATURES: &str = r#","what":[{"Type":"Creature"}]"#;
const SPELLS: &str = r#","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]"#;
const INSIST: &str = "The next creature spell you cast this turn can't be countered.";
const OVERMASTER: &str = "The next instant or sorcery spell you cast this turn can't be countered.";
const VILLAGE: &str = "The next spell you cast this turn can't be countered.";

#[rstest]
#[case::insist(next_spell(CREATURES, UNCOUNTERABLE, "UntilEndOfTurn"), INSIST, true)]
#[case::overmaster(next_spell(SPELLS, UNCOUNTERABLE, "UntilEndOfTurn"), OVERMASTER, true)]
#[case::village(next_spell("", UNCOUNTERABLE, "UntilEndOfTurn"), VILLAGE, true)]
#[case::insist_of_every_spell(next_spell("", UNCOUNTERABLE, "UntilEndOfTurn"), INSIST, false)]
#[case::village_of_creatures(
    next_spell(CREATURES, UNCOUNTERABLE, "UntilEndOfTurn"),
    VILLAGE,
    false
)]
#[case::village_forever(next_spell("", UNCOUNTERABLE, "Forever"), VILLAGE, false)]
#[case::village_with_flash(next_spell("", r#""Flash""#, "UntilEndOfTurn"), VILLAGE, false)]
fn ep26_a_next_spell_that_cannot_be_countered_writes_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn cast_from(zone: &str, what: &str) -> Ability {
    ability(&format!(
        r#"{{"Rule":{{"from":"{zone}","rule":{{"MayCast":{{"from":"{zone}","what":{what}}}}}}}}}"#
    ))
}

const SQUEE: &str = "You may cast ~ from your graveyard or from exile.";
const ITSELF: &str = r#"["This"]"#;

#[rstest]
#[case::squee(cast_from("Graveyard", ITSELF), cast_from("Exile", ITSELF), true)]
#[case::exile_first(cast_from("Exile", ITSELF), cast_from("Graveyard", ITSELF), false)]
#[case::graveyard_twice(cast_from("Graveyard", ITSELF), cast_from("Graveyard", ITSELF), false)]
#[case::another_card_from_exile(
    cast_from("Graveyard", ITSELF),
    cast_from("Exile", r#"[{"Type":"Creature"}]"#),
    false
)]
fn ep26_a_card_cast_from_two_zones_writes_as_printed(
    #[case] first: Ability,
    #[case] second: Ability,
    #[case] reads: bool,
) {
    let found = crate::compile::read::static_pair_texts(&first, &second);
    assert_eq!(
        found.iter().any(|w| same_text(w, SQUEE)),
        reads,
        "{found:?}"
    );
}
