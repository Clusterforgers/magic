//! Goal: an activated ability reads from its cost parts in the printed order (`{T}`, mana, life, sacrifice, discard), its sentences and its "Activate only" restrictions in the shapes of the hand-written files and writes back as the same text, and a cost or restriction the templates do not model stays with the table.
use super::*;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::mana_and_tap(
    "{1}, {T}: Add {C}.",
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#
)]
#[case::life_in_the_cost(
    "{T}, Pay 1 life: Add {B}.",
    r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Black"]}}}]}}"#
)]
#[case::treasure(
    "{T}, Sacrifice ~: Add one mana of any color.",
    r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#
)]
#[case::clue(
    "{2}, Sacrifice ~: Draw a card.",
    r#"{"Activated":{"cost":[{"Mana":"{2}"},"SacrificeSelf"],"effects":[{"Draw":{}}]}}"#
)]
#[case::food(
    "{2}, {T}, Sacrifice ~: You gain 3 life.",
    r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap","SacrificeSelf"],"effects":[{"GainLife":{"amount":3}}]}}"#
)]
#[case::warmonger_hellkite_pump(
    "{1}{R}: Attacking creatures get +1/+0 until end of turn.",
    r#"{"Activated":{"cost":"{1}{R}","effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Attacking"]},"mods":[{"ModifyPT":{"power":1,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::a_target(
    "{T}: Tap target creature.",
    r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#
)]
#[case::untap_the_card(
    "{3}: Untap ~.",
    r#"{"Activated":{"cost":"{3}","effects":[{"Untap":{"what":"This"}}]}}"#
)]
#[case::two_steps(
    "{U}, {T}: Draw a card, then discard a card.",
    r#"{"Activated":{"cost":[{"Mana":"{U}"},"Tap"],"effects":[{"Draw":{}},{"Discard":{}}]}}"#
)]
#[case::sacrifice_a_creature(
    "Sacrifice a creature: ~ gets +2/+2 until end of turn.",
    r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::sacrifice_another(
    "{T}, Sacrifice another creature: Put a +1/+1 counter on ~.",
    r#"{"Activated":{"cost":["Tap",{"Sacrifice":{"filter":[{"Type":"Creature"},"Other"]}}],"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::sacrifice_two(
    "Sacrifice two artifacts: Draw a card.",
    r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Artifact"}],"count":2}}],"effects":[{"Draw":{}}]}}"#
)]
#[case::sacrifice_a_subtype(
    "{R}, Sacrifice a Goblin: ~ deals 2 damage to any target.",
    r#"{"Activated":{"cost":[{"Mana":"{R}"},{"Sacrifice":{"filter":[{"Subtype":"Goblin"}]}}],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]}}"#
)]
#[case::discard_a_card(
    "{T}, Discard a card: Draw a card.",
    r#"{"Activated":{"cost":["Tap",{"Discard":{}}],"effects":[{"Draw":{}}]}}"#
)]
#[case::tap_an_untapped_creature(
    "Tap an untapped creature you control: ~ gets +1/+1 until end of turn.",
    r#"{"Activated":{"cost":[{"TapPermanents":{"filter":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::tap_two_untapped_creatures(
    "{1}, Tap two untapped creatures you control: Draw a card.",
    r#"{"Activated":{"cost":[{"Mana":"{1}"},{"TapPermanents":{"filter":[{"Type":"Creature"}],"count":2}}],"effects":[{"Draw":{}}]}}"#
)]
#[case::exile_a_creature_card_from_the_graveyard(
    "{W}, Exile a creature card from your graveyard: You gain 1 life.",
    r#"{"Activated":{"cost":[{"Mana":"{W}"},{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}]}}],"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::exile_any_card_from_the_graveyard(
    "Exile a card from your graveyard: Draw a card.",
    r#"{"Activated":{"cost":[{"ExileFromGraveyard":{"filter":[]}}],"effects":[{"Draw":{}}]}}"#
)]
#[case::exile_two_cards_from_the_graveyard(
    "Exile two creature cards from your graveyard: Draw a card.",
    r#"{"Activated":{"cost":[{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}],"count":2}}],"effects":[{"Draw":{}}]}}"#
)]
#[case::remove_a_counter(
    "{T}, Remove a charge counter from ~: You gain 1 life.",
    r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":"charge"}}],"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::remove_two_counters(
    "Remove two +1/+1 counters from ~: Draw a card.",
    r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"+1/+1","count":2}}],"effects":[{"Draw":{}}]}}"#
)]
#[case::return_a_land(
    "Return a land you control to its owner's hand: Draw a card.",
    r#"{"Activated":{"cost":[{"ReturnToHand":{"filter":[{"Type":"Land"}]}}],"effects":[{"Draw":{}}]}}"#
)]
#[case::drudge_skeletons(
    "{B}: Regenerate ~.",
    r#"{"Activated":{"cost":"{B}","effects":[{"Regenerate":{"what":"This"}}]}}"#
)]
#[case::regenerate_a_target(
    "{1}{G}: Regenerate target creature.",
    r#"{"Activated":{"cost":"{1}{G}","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Regenerate":{"what":{"Target":0}}}]}}"#
)]
#[case::untap_symbol(
    "{Q}, {1}: Scry 1.",
    r#"{"Activated":{"cost":["Untap",{"Mana":"{1}"}],"effects":[{"Scry":{}}]}}"#
)]
fn an_activated_ability_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::sorcery(
    "{2}: Draw a card. Activate only as a sorcery.",
    r#"{"Activated":{"cost":"{2}","timing":"Sorcery","effects":[{"Draw":{}}]}}"#
)]
#[case::once_each_turn(
    "{T}: Draw a card. Activate only once each turn.",
    r#"{"Activated":{"cost":["Tap"],"limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#
)]
#[case::both(
    "{1}: Draw a card. Activate only as a sorcery and only once each turn.",
    r#"{"Activated":{"cost":"{1}","timing":"Sorcery","limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#
)]
#[case::both_the_other_way(
    "{1}: Draw a card. Activate only once each turn and only as a sorcery.",
    r#"{"Activated":{"cost":"{1}","timing":"Sorcery","limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#
)]
#[case::with_a_target(
    "{3}, {T}: Destroy target artifact. Activate only as a sorcery.",
    r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
fn the_restrictions_on_when_an_ability_may_be_activated_read(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[test]
fn an_x_in_the_cost_is_the_payers_to_choose() {
    let read = golden(
        "{X}, {T}: ~ deals X damage to any target.",
        ClauseKind::Activated,
        false,
    );
    assert_eq!(
        read,
        ability(
            r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap"],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#
        )
    );
    // Without the X in the cost nothing says what X is.
    assert_eq!(activated("{T}: ~ deals X damage to any target."), None);
}

#[rstest]
#[case::mana_in_a_spell("Add {B}{B}{B}.")]
#[case::mana_after_an_untap("Untap target land. Add {G}.")]
#[case::mana_and_damage("Add {R}. ~ deals 1 damage to you.")]
fn mana_is_an_effect_like_any_other(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[test]
fn mana_in_a_trigger_reads() {
    let read = clause("When ~ enters, add {R}.", ClauseKind::Triggered, false);
    assert_eq!(
        read,
        Some(golden(
            "When ~ enters, add {R}.",
            ClauseKind::Triggered,
            false
        ))
    );
}

/// Every cost part is read and written in the order it is printed, whatever it follows.
#[test]
fn every_ordered_run_of_cost_parts_with_every_effect_and_restriction_writes_back() {
    let parts = [
        "{T}",
        "{2}",
        "{1}{G}",
        "{X}",
        "Pay 2 life",
        "Sacrifice ~",
        "Sacrifice another creature",
        "Sacrifice a land",
        "Sacrifice three artifacts",
        "Discard a card",
    ];
    let effects = [
        "Draw a card.",
        "~ gets +2/+2 until end of turn.",
        "Add {G}.",
        "Put a +1/+1 counter on target creature.",
    ];
    let restrictions = [
        "",
        " Activate only as a sorcery.",
        " Activate only once each turn.",
        " Activate only as a sorcery and only once each turn.",
    ];
    let is_mana = |part: &str| part.starts_with('{') && part != "{T}";
    let mut runs: Vec<Vec<&str>> = parts.iter().map(|part| vec![*part]).collect();
    for _ in 0..2 {
        let longer: Vec<Vec<&str>> = runs
            .iter()
            .flat_map(|run| {
                parts
                    .iter()
                    .filter(|part| {
                        let second_mana = is_mana(part) && run.iter().any(|p| is_mana(p));
                        !run.contains(part) && !second_mana
                    })
                    .map(|part| [run.as_slice(), &[*part]].concat())
            })
            .collect();
        runs.extend(longer);
    }
    runs.sort();
    runs.dedup();
    let mut read = 0;
    for run in &runs {
        for effect in effects {
            for restriction in restrictions {
                let text = format!("{}: {effect}{restriction}", run.join(", "));
                golden(&text, ClauseKind::Activated, false);
                read += 1;
            }
        }
    }
    assert!(read > 5000, "only {read} wordings were read");
}

#[rstest]
#[case::mana_of_a_color_a_creature_could_produce(
    "{T}: Add one mana of any color that a creature an opponent controls could produce."
)]
#[case::a_condition_and_a_timing(
    "{T}: Add {B}. Activate only if you control a Swamp. Activate only as a sorcery."
)]
#[case::a_condition_that_is_not_a_count("{T}: Add {B}. Activate only if it's your turn.")]
#[case::both_restrictions_apart(
    "{T}: Draw a card. Activate only as a sorcery. Activate only once each turn."
)]
#[case::the_same_restriction_twice(
    "{T}: Draw a card. Activate only once each turn and only once each turn."
)]
#[case::an_unknown_restriction("{T}: Draw a card. Activate only during your turn.")]
#[case::a_restriction_first("{T}: Activate only as a sorcery. Draw a card.")]
#[case::a_creature_you_control("{T}, Sacrifice a creature you control: Draw a card.")]
#[case::a_creature_card("{T}, Sacrifice a creature card: Draw a card.")]
#[case::a_count_that_is_a_stat("Sacrifice creatures equal to its power: Draw a card.")]
#[case::one_of_several("Sacrifice one creature: Draw a card.")]
#[case::a_plural_for_one("Sacrifice a creatures: Draw a card.")]
#[case::a_singular_for_many("Sacrifice two creature: Draw a card.")]
#[case::digits("Sacrifice 2 creatures: Draw a card.")]
#[case::a_count_of_the_card("Sacrifice two ~: Draw a card.")]
#[case::discard_the_card("Discard ~: Draw a card.")]
#[case::energy_beside_life("{T}, Pay {E} and 2 life: Add one mana of any color.")]
#[case::life_of_an_unnamed_kind("{T}, Pay life: Add {B}.")]
#[case::loyalty_is_not_a_cost("+1: Draw a card.")]
#[case::no_cost_part("{T},: Draw a card.")]
#[case::nothing_to_do("{T}:")]
#[case::no_colon("{T} Draw a card.")]
#[case::regenerate_a_spell("{B}: Regenerate target spell.")]
#[case::regenerate_a_card_in_a_graveyard(
    "{B}: Regenerate target creature card from your graveyard."
)]
#[case::loose_words("When you do, {T}: Draw a card.")]
fn costs_and_restrictions_the_templates_do_not_model_stay_unread(#[case] text: &str) {
    assert_eq!(activated(text), None, "{text}");
}
