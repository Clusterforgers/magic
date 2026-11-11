//! Goal: the wf25 writers give the oracle wording of the object of the event as the object of a step ("destroy it",
//! "counter that spell", "tap that creature and it doesn't untap"), mana for each counter removed as the cost, a card
//! that begins the game on the battlefield, a creature that can't attack or block alone, blocks any number of creatures
//! or can't block strong creatures; another verb, object, noun, counter, mana or number never reads back as the clause.
use super::*;

fn reads(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn trigger(when: &str, effects: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when},"effects":{effects}}}}}"#)
}

const DEALT: &str = r#"{"Batch":{"DealsDamage":{"source":[],"to":"Attached"}}}"#;
const DIES: &str = r#"{"ZoneChange":{"what":[{"Type":"Creature"},{"OwnedBy":"You"}],"from":"Battlefield","to":"Graveyard"}}"#;
const DESTROY_IT: &str = r#"[{"Destroy":{"what":"Triggering"}}]"#;
const CAST: &str = r#"{"SpellCast":{"by":"Opponents"}}"#;
const COUNTER_IT: &str = r#"[{"Counter":{"what":"Triggering"}}]"#;
const HITS: &str =
    r#"{"DealsDamage":{"source":["This"],"to":{"Each":[{"Type":"Creature"}]},"combat":true}}"#;
const HELD: &str = r#"[{"Tap":{"what":"Triggering"}},{"Continuous":{"affected":"Triggering","mods":["DoesntUntap"],"duration":"NextUntapStep"}}]"#;
const KASHI: &str = "Whenever this creature deals combat damage to a creature, tap that creature and it doesn't untap during its controller's next untap step.";

#[rstest]
#[case::destroy_it(&trigger(DEALT, DESTROY_IT), "When enchanted creature is dealt damage, destroy it.", true)]
#[case::destroy_that_creature(&trigger(DEALT, DESTROY_IT), "When enchanted creature is dealt damage, destroy that creature.", true)]
#[case::exile_for_destroy(&trigger(DEALT, &DESTROY_IT.replace("Destroy", "Exile")), "When enchanted creature is dealt damage, destroy it.", false)]
#[case::the_card_itself(&trigger(DEALT, &DESTROY_IT.replace("Triggering", "This")), "When enchanted creature is dealt damage, destroy it.", false)]
#[case::a_noun_the_head_lacks(&trigger(DEALT, DESTROY_IT), "When enchanted creature is dealt damage, destroy that land.", false)]
#[case::regenerable(&trigger(DEALT, DESTROY_IT), "When enchanted creature is dealt damage, destroy it. It can't be regenerated.", false)]
#[case::counter_that_spell(&trigger(CAST, COUNTER_IT), "Whenever an opponent casts a spell, counter that spell.", true)]
#[case::counter_it(&trigger(CAST, COUNTER_IT), "Whenever an opponent casts a spell, counter it.", true)]
#[case::any_caster(&trigger(r#"{"SpellCast":{}}"#, COUNTER_IT), "Whenever an opponent casts a spell, counter that spell.", false)]
#[case::sacrificed_and_countered(&trigger(CAST, &COUNTER_IT.replace("}]", r#"},{"Sacrifice":{"filter":["This"]}}]"#)), "When an opponent casts a spell, counter that spell and sacrifice this enchantment.", true)]
#[case::countered_itself(&trigger(CAST, r#"[{"Counter":{"what":"Triggering"}},{"Counter":{"what":"This"}}]"#), "When an opponent casts a spell, counter that spell and counter this enchantment.", false)]
#[case::sacrificed_and_it(&trigger(CAST, &COUNTER_IT.replace("}]", r#"},{"Sacrifice":{"filter":["This"]}}]"#)), "When an opponent casts a spell, counter it and sacrifice this enchantment.", false)]
#[case::not_regenerated(&trigger(HITS, r#"[{"Destroy":{"what":"Triggering","regenerate":false}}]"#), "Whenever this creature deals combat damage to a creature, destroy that creature. It can't be regenerated.", true)]
#[case::regenerated(&trigger(HITS, r#"[{"Destroy":{"what":"Triggering"}}]"#), "Whenever this creature deals combat damage to a creature, destroy that creature. It can't be regenerated.", false)]
#[case::sacrificed_as_the_cost(&trigger(DIES, r#"[{"Pay":{"cost":["SacrificeSelf"],"paid":[{"Move":{"what":"Triggering","to":"Hand"}}]}}]"#), "Whenever a creature is put into your graveyard from the battlefield, you may sacrifice that card. Return it from your graveyard to your hand if you do.", false)]
#[case::on_your_library(&trigger(DIES, r#"[{"Move":{"what":"Triggering","to":{"Library":{"place":"Top"}}}}]"#), "Whenever a creature is put into your graveyard from the battlefield, put that card on top of your library.", true)]
#[case::on_the_bottom(&trigger(DIES, r#"[{"Move":{"what":"Triggering","to":{"Library":{"place":"Bottom"}}}}]"#), "Whenever a creature is put into your graveyard from the battlefield, put that card on top of your library.", false)]
#[case::the_other_creature(&trigger(r#"{"DealsDamage":{"source":[{"Is":"Attached"}],"to":{"Each":[{"Type":"Creature"}]}}}"#, DESTROY_IT), "Whenever enchanted creature deals damage to a creature, destroy the other creature.", true)]
#[case::the_other_for_the_enchanted(&trigger(r#"{"DealsDamage":{"source":[{"Is":"Attached"}],"to":{"Each":[{"Type":"Creature"}]}}}"#, r#"[{"Destroy":{"what":"Attached"}}]"#), "Whenever enchanted creature deals damage to a creature, destroy the other creature.", false)]
#[case::tapped_and_held(&trigger(HITS, HELD), KASHI, true)]
#[case::held_only(&trigger(HITS, &HELD.replace(r#"{"Tap":{"what":"Triggering"}},"#, "")), KASHI, false)]
#[case::held_for_a_turn(&trigger(HITS, &HELD.replace("NextUntapStep", "UntilEndOfTurn")), KASHI, false)]
#[case::it_after_the_card(&trigger(HITS, HELD), "Whenever this creature deals combat damage to a creature, tap it and it doesn't untap during its controller's next untap step.", false)]
fn wf25_the_object_of_the_event_is_named_in_its_steps(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const STORAGE: &str = r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":{"Other":"storage"},"count":"X"}}],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":"X"}}]}}"#;
const VAULT: &str = "{T}, Remove any number of storage counters from this land: Add {R} for each storage counter removed this way.";
const BATTERY: &str = r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":"charge","count":"X"}}],"effects":[{"AddMana":{"mana":{"Fixed":["Black"]}}},{"AddMana":{"mana":{"Fixed":["Black"]},"times":"X"}}]}}"#;
const CELL: &str = "{T}, Remove any number of charge counters from this artifact: Add {B}, then add an additional {B} for each charge counter removed this way.";

#[rstest]
#[case::storage(STORAGE, VAULT, true)]
#[case::other_color(&STORAGE.replace("Red", "Blue"), VAULT, false)]
#[case::other_counter(&STORAGE.replace("storage", "charge"), VAULT, false)]
#[case::one_counter(&STORAGE.replace(r#","count":"X""#, ""), VAULT, false)]
#[case::mana_once(&STORAGE.replace(r#","times":"X""#, ""), VAULT, false)]
#[case::untapped(&STORAGE.replace(r#""Tap","#, ""), VAULT, false)]
#[case::battery(BATTERY, CELL, true)]
#[case::battery_without_the_first(BATTERY, &CELL.replace("Add {B}, then add an additional {B}", "Add {B}"), false)]
#[case::battery_other_first(&BATTERY.replacen("Black", "Red", 1), CELL, false)]
fn wf25_mana_for_each_counter_removed(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const LEYLINE: &str =
    "If this card is in your opening hand, you may begin the game with it on the battlefield.";
const ALONE: &str = r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":"CantAttackAlone"}}}}"#;
const ANY_NUMBER: &str = r#"{"Static":{"affected":"This","mods":[{"BlocksAdditional":99}]}}"#;
const STRONG: &str = r#"{"Static":{"affected":"This","mods":[{"CanBlockOnly":[{"AnyOf":[{"Not":{"Type":"Creature"}},{"Not":{"Compare":{"stat":"Power","op":"Ge","to":2}}}]}]}]}}"#;

#[rstest]
#[case::leyline(r#""OpeningHand""#, LEYLINE, true)]
#[case::leyline_revealed(
    r#""OpeningHand""#,
    "You may reveal this card from your opening hand.",
    false
)]
#[case::attack_alone(ALONE, "This creature can't attack alone.", true)]
#[case::block_alone(&ALONE.replace("Attack", "Block"), "This creature can't block alone.", true)]
#[case::block_for_attack(&ALONE.replace("Attack", "Block"), "This creature can't attack alone.", false)]
#[case::enchanted_alone(&ALONE.replace("This", "Attached"), "This creature can't attack alone.", false)]
#[case::any_number(ANY_NUMBER, "This creature can block any number of creatures.", true)]
#[case::two_more(&ANY_NUMBER.replace("99", "2"), "This creature can block any number of creatures.", false)]
#[case::strong(
    STRONG,
    "This creature can't block creatures with power 2 or greater.",
    true
)]
#[case::stronger(&STRONG.replace(r#""to":2"#, r#""to":3"#), "This creature can't block creatures with power 2 or greater.", false)]
#[case::weak(&STRONG.replace("Ge", "Le"), "This creature can't block creatures with power 2 or greater.", false)]
#[case::strong_artifacts(&STRONG.replace("Creature", "Artifact"), "This creature can't block creatures with power 2 or greater.", false)]
fn wf25_rules_of_the_card(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

#[rstest]
#[case::attack_or_block(ALONE, &ALONE.replace("Attack", "Block"), true)]
#[case::reversed(&ALONE.replace("Attack", "Block"), ALONE, false)]
#[case::twice(ALONE, ALONE, false)]
fn wf25_alone_in_one_sentence(#[case] first: &str, #[case] second: &str, #[case] same: bool) {
    let written = static_pair_texts(&ability(first), &ability(second));
    assert_eq!(
        written
            .iter()
            .any(|text| same_text(text, "This creature can't attack or block alone.")),
        same
    );
}

const SINGLE: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Together":{"what":{"Object":[{"In":"Graveyard"}]},"rule":"SameController"}},"bounds":{"min":0,"max":3}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#;
const BEETLES: &str = "{T}: Exile up to three target cards from a single graveyard.";

#[rstest]
#[case::single(SINGLE, BEETLES, true)]
#[case::any_graveyards(&SINGLE.replace(r#"{"Together":{"what":{"Object":[{"In":"Graveyard"}]},"rule":"SameController"}}"#, r#"{"Object":[{"In":"Graveyard"}]}"#), BEETLES, false)]
#[case::different_owners(&SINGLE.replace("SameController", "DifferentControllers"), BEETLES, false)]
#[case::your_graveyard(&SINGLE.replace(r#"{"In":"Graveyard"}"#, r#"{"In":"Graveyard"},{"OwnedBy":"You"}"#), BEETLES, false)]
#[case::two(&SINGLE.replace(r#""max":3"#, r#""max":2"#), BEETLES, false)]
fn wf25_cards_from_a_single_graveyard(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const TAPPED: &str = r#"{"Tapped":{"what":[{"Is":"Attached"}]}}"#;
const CRYOSHATTER: &str = "When enchanted creature becomes tapped or is dealt damage, destroy it.";

#[rstest]
#[case::either(&trigger(TAPPED, DESTROY_IT), &trigger(DEALT, DESTROY_IT), true)]
#[case::other_steps(&trigger(TAPPED, DESTROY_IT), &trigger(DEALT, &DESTROY_IT.replace("Destroy", "Exile")), false)]
#[case::one_event_twice(&trigger(TAPPED, DESTROY_IT), &trigger(TAPPED, DESTROY_IT), false)]
#[case::one_event_alone(&trigger(TAPPED, DESTROY_IT), r#"{"Keyword":"Flying"}"#, false)]
fn wf25_two_events_of_one_subject(#[case] first: &str, #[case] second: &str, #[case] same: bool) {
    let written = either_way_pair_texts(&ability(first), &ability(second));
    assert_eq!(
        written.iter().any(|text| same_text(text, CRYOSHATTER)),
        same,
        "{written:?}"
    );
}
