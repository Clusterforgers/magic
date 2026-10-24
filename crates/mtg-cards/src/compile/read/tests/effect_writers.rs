//! Goal: the writers of the proposal checker give the oracle wording of the effects the sample of model-written rules
//! found missing: a bound exile that returns at the next end step or whose mana value an amount names, counters on a
//! player, a sacrifice in place of damage, a choice of several steps, a spell cast from anywhere other than the hand, a
//! trigger of the card and another creature, life gained this turn and a creature that died; a changed count, kind,
//! player or object never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const ENTERS: &str = r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#;
const DIES: &str = r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#;

fn exile_returned(under: &str, step: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{2}}"}},"Tap"],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},"Other"]}}}}],"effects":[{{"Exile":{{"what":{{"Target":0}},"as":"exiled"}}}},{{"DelayedTrigger":{{"when":{{"StepBegins":{{"step":"{step}"}}}},"effects":[{{"Move":{{"what":{{"Bound":"exiled"}},"to":{{"Battlefield":{under}}}}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::roon(
    exile_returned("{}", "End"),
    "{2}, {T}: Exile another target creature. Return that card to the battlefield under its owner's control at the beginning of the next end step."
)]
#[case::pronoun(
    exile_returned("{}", "End"),
    "{2}, {T}: Exile another target creature. Return it to the battlefield under its owner's control at the beginning of the next end step."
)]
#[case::under_your_control(
    exile_returned(r#"{"under":"You"}"#, "End"),
    "{2}, {T}: Exile another target creature. Return that card to the battlefield under your control at the beginning of the next end step."
)]
#[case::tapped(
    exile_returned(r#"{"tapped":true}"#, "End"),
    "{2}, {T}: Exile another target creature. Return that card to the battlefield tapped under its owner's control at the beginning of the next end step."
)]
fn an_exile_that_returns_at_the_next_end_step_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::the_upkeep(exile_returned("{}", "Upkeep"))]
#[case::to_another_player(exile_returned(r#"{"under":"Opponents"}"#, "End"))]
fn an_exile_that_returns_at_another_step_does_not_read_as_the_next_end_step(#[case] json: String) {
    let clause = "{2}, {T}: Exile another target creature. Return that card to the battlefield under its owner's control at the beginning of the next end step.";
    assert!(!written(&json, clause), "{clause}");
}

const RUINOUS: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}},{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"PutCounters":{"on":{"Target":1},"counter":"+1/+1","count":{"Stat":{"of":{"Bound":"exiled"},"stat":"ManaValue"}}}}]}}"#;

#[rstest]
#[case::ruinous_intrusion(
    RUINOUS,
    "Exile target artifact or enchantment. Put X +1/+1 counters on target creature you control, where X is the mana value of the permanent exiled this way."
)]
fn the_counters_of_a_second_target_count_what_the_first_exile_took(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::the_power(
    "Exile target artifact or enchantment. Put X +1/+1 counters on target creature you control, where X is the power of the permanent exiled this way."
)]
#[case::no_where_clause(
    "Exile target artifact or enchantment. Put X +1/+1 counters on target creature you control."
)]
#[case::the_other_target_first(
    "Exile target creature you control. Put X +1/+1 counters on target artifact or enchantment, where X is the mana value of the permanent exiled this way."
)]
#[case::one_target_only(
    "Exile target artifact or enchantment. Put X +1/+1 counters on it, where X is the mana value of the permanent exiled this way."
)]
fn the_counters_of_a_second_target_do_not_read_as_another_stat_or_target(#[case] clause: &str) {
    assert!(!written(RUINOUS, clause), "{clause}");
}

#[rstest]
#[case::two_targets_in_separate_sentences(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":"Player"}],"effects":[{"Destroy":{"what":{"Target":0}}},{"LoseLife":{"who":{"Target":1},"amount":2}}]}}"#,
    "Destroy target creature. Target player loses 2 life."
)]
#[case::an_effect_without_a_target_joins_the_one_before(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":"Player"}],"effects":[{"Destroy":{"what":{"Target":0}}},{"Draw":{}},{"LoseLife":{"who":{"Target":1},"amount":2}}]}}"#,
    "Destroy target creature. Draw a card. Target player loses 2 life."
)]
fn the_effects_of_each_target_are_sentences_of_their_own(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::the_second_target_first(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":"Player"}],"effects":[{"LoseLife":{"who":{"Target":1},"amount":2}},{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::an_effect_on_two_targets(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Fight":{"what":{"Target":0},"with":{"Target":1}}}]}}"#
)]
#[case::a_target_no_effect_uses(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":"Player"}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
fn effects_that_do_not_use_the_targets_in_order_are_not_written(#[case] json: &str) {
    assert!(
        !written(json, "Destroy target creature. Target player loses 2 life."),
        "{json}"
    );
}

fn trigger_upkeep(effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"Opponents"}}}},"effects":[{effect}]}}}}"#
    )
}

fn damage_unless(cost: &str, who: &str) -> String {
    trigger_upkeep(&format!(
        r#"{{"Pay":{{"who":"{who}","cost":{cost},"unpaid":[{{"DealDamage":{{"to":"{who}","amount":2}}}}]}}}}"#
    ))
}

#[rstest]
#[case::mogis(
    damage_unless(r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#, "Triggering"),
    "At the beginning of each opponent's upkeep, ~ deals 2 damage to that player unless they sacrifice a creature of their choice."
)]
#[case::a_land(
    damage_unless(r#"[{"Sacrifice":{"filter":[{"Type":"Land"}]}}]"#, "Triggering"),
    "At the beginning of each opponent's upkeep, ~ deals 2 damage to that player unless they sacrifice a land of their choice."
)]
fn damage_that_a_sacrifice_prevents_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_permanent(
    "At the beginning of each opponent's upkeep, ~ deals 2 damage to that player unless they sacrifice a land of their choice."
)]
#[case::more_damage(
    "At the beginning of each opponent's upkeep, ~ deals 3 damage to that player unless they sacrifice a creature of their choice."
)]
#[case::you_sacrifice(
    "At the beginning of each opponent's upkeep, ~ deals 2 damage to that player unless you sacrifice a creature of their choice."
)]
fn damage_that_a_sacrifice_prevents_does_not_read_as_another_permanent_or_amount(
    #[case] clause: &str,
) {
    let json = damage_unless(
        r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#,
        "Triggering",
    );
    assert!(!written(&json, clause), "{clause}");
}

fn rad_counters(on: &str, count: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{ENTERS},"targets":[{{"what":"Player"}}],"effects":[{{"PutCounters":{{"on":{on},"counter":{{"Other":"rad"}},"count":{count}}}}}]}}}}"#
    )
}

#[rstest]
#[case::nightkin_ambusher(
    rad_counters(r#"{"Target":0}"#, 4),
    "When this creature enters, target player gets four rad counters."
)]
#[case::one(
    rad_counters(r#"{"Target":0}"#, 1),
    "When this creature enters, target player gets a rad counter."
)]
fn counters_on_a_player_are_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::three("When this creature enters, target player gets three rad counters.")]
#[case::put_on_the_player("When this creature enters, put four rad counters on target player.")]
#[case::poison("When this creature enters, target player gets four poison counters.")]
fn counters_on_a_player_do_not_read_as_another_count_or_kind(#[case] clause: &str) {
    assert!(
        !written(&rad_counters(r#"{"Target":0}"#, 4), clause),
        "{clause}"
    );
}

fn defending_counters(counter: &str, number: u32) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":["CantBeBlocked"],"condition":{{"Compare":{{"left":{{"Counters":{{"on":"Defending","counter":{{"Other":"{counter}"}}}}}},"op":"Ge","right":{number}}}}}}}}}"#
    )
}

#[rstest]
#[case::nightkin_ambusher(
    defending_counters("rad", 1),
    "This creature can't be blocked as long as defending player has a rad counter."
)]
#[case::three(
    defending_counters("rad", 3),
    "This creature can't be blocked as long as defending player has three or more rad counters."
)]
fn a_static_that_asks_for_counters_on_the_defending_player_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_kind(
    "This creature can't be blocked as long as defending player has a poison counter."
)]
#[case::you("This creature can't be blocked as long as you have a rad counter.")]
#[case::on_it("This creature can't be blocked as long as it has a rad counter on it.")]
fn a_static_that_asks_for_counters_on_the_defending_player_does_not_read_as_another_holder(
    #[case] clause: &str,
) {
    assert!(!written(&defending_counters("rad", 1), clause), "{clause}");
}

fn cast_from(selector: &str, by: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":{selector},"by":"{by}"}}}},"effects":[{{"DealDamage":{{"to":"Opponents","amount":1}}}}]}}}}"#
    )
}

#[rstest]
#[case::unstable_amulet(
    cast_from(r#"[{"Not":{"CastFrom":"Hand"}}]"#, "You"),
    "Whenever you cast a spell from anywhere other than your hand, ~ deals 1 damage to each opponent."
)]
#[case::a_creature_spell(
    cast_from(r#"[{"Type":"Creature"},{"Not":{"CastFrom":"Hand"}}]"#, "You"),
    "Whenever you cast a creature spell from anywhere other than your hand, ~ deals 1 damage to each opponent."
)]
fn a_spell_cast_from_anywhere_other_than_the_hand_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::any_spell("Whenever you cast a spell, ~ deals 1 damage to each opponent.")]
#[case::from_the_graveyard(
    "Whenever you cast a spell from your graveyard, ~ deals 1 damage to each opponent."
)]
#[case::an_opponent_casts(
    "Whenever an opponent casts a spell from anywhere other than your hand, ~ deals 1 damage to each opponent."
)]
fn a_spell_cast_from_anywhere_other_than_the_hand_does_not_read_as_another_origin(
    #[case] clause: &str,
) {
    let json = cast_from(r#"[{"Not":{"CastFrom":"Hand"}}]"#, "You");
    assert!(!written(&json, clause), "{clause}");
}

fn investigates(other: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"AnyOf":[{DIES},{{"ZoneChange":{{"what":{other},"from":"Battlefield","to":"Graveyard"}}}}]}},"effects":[{{"May":{{"effects":[{{"CreateToken":{{"token":{{"Named":"clue"}}}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::nick_valentine(
    investigates(r#"["Other",{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}]"#),
    "Whenever ~ or another artifact creature you control dies, you may investigate."
)]
#[case::create_a_clue(
    investigates(r#"["Other",{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}]"#),
    "Whenever ~ or another artifact creature you control dies, you may create a Clue token."
)]
fn the_card_and_another_creature_dying_and_investigating_are_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::any_creature("Whenever ~ or another creature you control dies, you may investigate.")]
#[case::no_choice("Whenever ~ or another artifact creature you control dies, investigate.")]
#[case::enters("Whenever ~ or another artifact creature you control enters, you may investigate.")]
fn the_card_and_another_creature_dying_does_not_read_as_another_event(#[case] clause: &str) {
    let json =
        investigates(r#"["Other",{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}]"#);
    assert!(!written(&json, clause), "{clause}");
}

fn may_steps(steps: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"effects":[{{"May":{{"effects":[{steps}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::gain_life(
    may_steps(r#"{"GainLife":{"amount":3}}"#),
    "Whenever ~ attacks, you may gain 3 life."
)]
#[case::draw_and_lose_life(
    may_steps(r#"{"Draw":{}},{"LoseLife":{"amount":1}}"#),
    "Whenever ~ attacks, you may draw a card and lose 1 life."
)]
#[case::three_steps(
    may_steps(r#"{"Draw":{}},{"LoseLife":{"amount":1}},{"GainLife":{"amount":2}}"#),
    "Whenever ~ attacks, you may draw a card, lose 1 life, and gain 2 life."
)]
fn a_choice_of_several_steps_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::no_choice("Whenever ~ attacks, you gain 3 life and draw a card.")]
#[case::other_amount("Whenever ~ attacks, you may draw a card and lose 2 life.")]
#[case::other_order("Whenever ~ attacks, you may lose 1 life and draw a card.")]
fn a_choice_of_several_steps_does_not_read_as_another_choice(#[case] clause: &str) {
    let json = may_steps(r#"{"Draw":{}},{"LoseLife":{"amount":1}}"#);
    assert!(!written(&json, clause), "{clause}");
}

fn at_end_step(condition: &str, effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"if":{condition},"effects":[{effect}]}}}}"#
    )
}

const GAINED_THREE: &str = r#"{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeGainedTotal"}},"op":"Ge","right":3}}"#;
const A_CREATURE_DIED: &str = r#"{"Compare":{"left":{"Tallied":{"who":"EachPlayer","what":"CreaturesDied"}},"op":"Ge","right":1}}"#;
const EACH_OPPONENT_LOSES: &str = r#"{"LoseLife":{"who":"Opponents","amount":3}}"#;

#[rstest]
#[case::indulging_patrician(
    at_end_step(GAINED_THREE, EACH_OPPONENT_LOSES),
    "At the beginning of your end step, if you gained 3 or more life this turn, each opponent loses 3 life."
)]
#[case::in_words(
    at_end_step(GAINED_THREE, EACH_OPPONENT_LOSES),
    "At the beginning of your end step, if you gained three or more life this turn, each opponent loses 3 life."
)]
#[case::a_creature_died(
    at_end_step(A_CREATURE_DIED, EACH_OPPONENT_LOSES),
    "At the beginning of your end step, if a creature died this turn, each opponent loses 3 life."
)]
fn what_happened_this_turn_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::more_life(
    "At the beginning of your end step, if you gained 4 or more life this turn, each opponent loses 3 life."
)]
#[case::once(
    "At the beginning of your end step, if you gained life this turn, each opponent loses 3 life."
)]
#[case::a_creature_died(
    "At the beginning of your end step, if a creature died this turn, each opponent loses 3 life."
)]
#[case::an_opponent_gained(
    "At the beginning of your end step, if an opponent gained 3 or more life this turn, each opponent loses 3 life."
)]
fn what_happened_this_turn_does_not_read_as_another_count_or_event(#[case] clause: &str) {
    assert!(
        !written(&at_end_step(GAINED_THREE, EACH_OPPONENT_LOSES), clause),
        "{clause}"
    );
}

const URZAS_TOWER: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"Fixed":["Colorless"]},"when":{"All":[{"Compare":{"left":{"Count":[{"Subtype":"Urza's Mine"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Count":[{"Subtype":"Urza's Power-Plant"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}]},"instead":{"Fixed":["Colorless","Colorless","Colorless"]}}}}}]}}"#;

#[rstest]
#[case::urzas_tower(
    URZAS_TOWER,
    "{T}: Add {C}. If you control an Urza's Mine and an Urza's Power-Plant, add {C}{C}{C} instead."
)]
fn mana_that_depends_on_two_lands_is_written_as_the_oracle_words_it(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::one_land("{T}: Add {C}. If you control an Urza's Mine, add {C}{C}{C} instead.")]
#[case::or(
    "{T}: Add {C}. If you control an Urza's Mine or an Urza's Power-Plant, add {C}{C}{C} instead."
)]
#[case::two_mana(
    "{T}: Add {C}. If you control an Urza's Mine and an Urza's Power-Plant, add {C}{C} instead."
)]
fn mana_that_depends_on_two_lands_does_not_read_as_one_land_or_another_amount(
    #[case] clause: &str,
) {
    assert!(!written(URZAS_TOWER, clause), "{clause}");
}

#[rstest]
#[case::reanimate(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"under":"You"}}}},{"LoseLife":{"amount":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}"#,
    "Put target creature card from a graveyard onto the battlefield under your control. You lose life equal to that card's mana value."
)]
#[case::its_mana_value(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"under":"You"}}}},{"LoseLife":{"amount":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}"#,
    "Put target creature card from a graveyard onto the battlefield under your control. You lose life equal to its mana value."
)]
fn life_equal_to_a_characteristic_of_the_target_is_written_as_the_oracle_words_it(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::kirri(
    r#"{"Static":{"affected":{"Each":["Other",{"AnyOf":[{"Subtype":"Plant"},{"Subtype":"Treefolk"}]},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":0}}]}}"#,
    "Other Plants and Treefolk you control get +2/+0."
)]
#[case::without_two_keywords(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}},{"Not":{"HasKeyword":{"Landwalk":[{"Subtype":"Island"}]}}}]},"mods":["CantAttack"]}}"#,
    "Creatures without flying or islandwalk can't attack."
)]
fn a_group_is_named_as_the_oracle_names_it(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::treefolks("Other Plants and Treefolks you control get +2/+0.")]
#[case::all_of_them("Plants and Treefolk you control get +2/+0.")]
fn a_group_of_a_subtype_with_the_same_plural_is_not_written_with_an_s(#[case] clause: &str) {
    let json = r#"{"Static":{"affected":{"Each":["Other",{"AnyOf":[{"Subtype":"Plant"},{"Subtype":"Treefolk"}]},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":0}}]}}"#;
    assert!(!written(json, clause), "{clause}");
}
