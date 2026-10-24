//! Goal: a sentence that acts on the tokens the sentence before created (counters on them, an Equipment attached, a grant until end of turn, a must-attack rule) reads into a `CreateToken` that binds them and an effect over `Bound`, and writes back as the same two sentences, for one token and for many.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::counters_by_the_mana_value(
    "Whenever you cast an instant or sorcery spell, create a 0/0 green and blue Fractal creature token. Put X +1/+1 counters on it, where X is that spell's mana value."
)]
#[case::counters_by_a_count(
    "At the beginning of your end step, create a 0/0 red Gremlin artifact creature token. Put X +1/+1 counters on it, where X is the number of artifacts you control."
)]
#[case::an_equipment_attached(
    "When this Equipment enters, create a 1/1 white Soldier creature token, then attach this Equipment to it."
)]
#[case::haste(
    "At the beginning of combat on your turn, create a 1/1 colorless Thopter artifact creature token with flying. That token gains haste until end of turn."
)]
#[case::haste_for_many(
    "At the beginning of combat on your turn, create two 1/1 red Goblin creature tokens. Those tokens gain haste until end of turn."
)]
#[case::haste_and_must_attack(
    "At the beginning of combat on your turn, create a 1/1 red Goblin creature token. That token gains haste until end of turn and attacks this combat if able."
)]
fn a_sentence_on_the_new_tokens_reads_and_writes_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::a_counter_on_each(
    "Create three tapped 1/1 green Treefolk creature tokens. Put a +1/+1 counter on each of them."
)]
#[case::a_counter_on_it("Create a 2/2 black Zombie creature token. Put a +1/+1 counter on it.")]
#[case::a_keyword_counter_on_each(
    "Create three tapped X/X green Treefolk creature tokens, where X is the amount of life you gained this turn. Put a reach counter on each of them."
)]
#[case::a_keyword_counter_on_it(
    "Create a 2/2 black Zombie creature token. Put a flying counter on it."
)]
fn counters_on_the_tokens_of_a_spell_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::commander(
    "At the beginning of combat on your turn, if you control your commander, create a 1/1 colorless Thopter artifact creature token with flying."
)]
#[case::commander_haste(
    "At the beginning of combat on your turn, if you control your commander, create a 1/1 colorless Thopter artifact creature token with flying. That token gains haste until end of turn."
)]
#[case::commander_haste_for_many(
    "At the beginning of combat on your turn, if you control your commander, create two 1/1 red Goblin creature tokens. Those tokens gain haste until end of turn."
)]
#[case::artifacts(
    "At the beginning of your end step, if you control three or more artifacts, create a Treasure token."
)]
fn an_intervening_if_on_what_you_control_guards_the_tokens(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::lieutenant(
    "Lieutenant \u{2014} At the beginning of combat on your turn, if you control your commander, create a 1/1 colorless Thopter artifact creature token with flying. That token gains haste until end of turn."
)]
fn an_ability_word_leads_the_trigger(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
}

#[rstest]
#[case::the_tokens_are_not_the_last_effect(
    "Create a 1/1 white Soldier creature token and a 2/2 black Zombie creature token. Put a +1/+1 counter on it."
)]
#[case::nothing_was_created("Draw a card. Put a +1/+1 counter on it.")]
#[case::attach_to_another_object(
    "Create a 1/1 white Soldier creature token. Attach this Equipment to target creature."
)]
fn a_sentence_that_names_no_new_token_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
