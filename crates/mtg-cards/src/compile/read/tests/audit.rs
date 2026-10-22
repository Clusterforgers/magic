//! Goal: the wordings that a listing of compiled cards beside their oracle text showed to be read wrongly stay with the table: a counter whose effect the rules give and a counter that reminder text explains; the neighbouring wordings that are read faithfully still read, a mana ability with other effects among them (the data holds it whole, as the rules play it without the stack).
use super::*;
use crate::card::ImplStatus;
use crate::compile::compile;
use crate::support::{PrintedCard, PrintedFace};

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::mana_and_damage_to_you("{T}: Add {R}. ~ deals 1 damage to you.")]
#[case::a_painland("{T}: Add {W} or {U}. ~ deals 1 damage to you.")]
#[case::a_painland_of_the_chosen_color(
    "{T}: Add one mana of the chosen color. ~ deals 1 damage to you."
)]
#[case::an_egg("{T}, Sacrifice ~: Add one mana of any color. Draw a card.")]
#[case::a_choice_and_a_second_effect("{T}: Add {W} or {U}. Draw a card.")]
#[case::mana_and_a_counter("{2}, {T}: Add one mana of any color. Put a brick counter on ~.")]
#[case::mana_and_a_card("{2}, {T}, Sacrifice ~: Add {U}{B}. Draw a card.")]
#[case::mana_and_life("{T}: Add {C}. You gain 1 life.")]
#[case::life_first("{T}: You gain 1 life. Add {C}.")]
fn a_mana_ability_with_other_effects_reads_whole_and_writes_back(#[case] text: &str) {
    let read = activated(text).unwrap_or_else(|| panic!("{text} is not read"));
    assert!(read.is_mana_ability(), "{text}");
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::mana_alone("{T}: Add {R}.")]
#[case::two_mana("{T}: Add {C}{C}.")]
#[case::any_color("{1}, {T}: Add one mana of any color.")]
#[case::a_target_makes_it_a_stack_ability(
    "{T}: Add {G}. Target creature gains haste until end of turn."
)]
#[case::no_mana("{T}: ~ deals 1 damage to any target.")]
fn a_mana_ability_alone_and_an_ability_that_is_not_one_still_read(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::spell("Add {B}{B}{B}.", ClauseKind::Spell)]
#[case::spell_with_damage("Add {R}. ~ deals 1 damage to you.", ClauseKind::Spell)]
#[case::trigger("When ~ enters, add {R}. You gain 1 life.", ClauseKind::Triggered)]
fn mana_in_a_spell_or_a_trigger_uses_the_stack_and_reads(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    golden(text, kind, false);
}

#[rstest]
#[case::shield("Put a shield counter on target creature.")]
#[case::stun("Put a stun counter on target creature.")]
#[case::finality("Put a finality counter on target creature.")]
#[case::flying("Put a flying counter on target creature.")]
#[case::deathtouch("Put a deathtouch counter on each creature you control.")]
#[case::lifelink("Put a lifelink counter on ~.")]
#[case::lore("Put a lore counter on ~.")]
#[case::defense("Put two defense counters on ~.")]
#[case::removing("Remove a shield counter from ~.")]
#[case::moving("Move a flying counter from target creature onto ~.")]
fn a_counter_the_rules_give_an_effect_to_is_not_read(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::quest("Put a quest counter on ~.")]
#[case::time("Remove a time counter from ~.")]
#[case::charge("Put two charge counters on target artifact.")]
fn a_counter_the_card_gives_its_meaning_to_still_reads(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

fn card_with(text: &str) -> PrintedCard {
    PrintedCard {
        layout: String::new(),
        oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
        name: "Test Card".into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: "Test Card".into(),
            type_line: "Creature \u{2014} Dwarf".into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

fn status_of(text: &str) -> Option<ImplStatus> {
    compile(&card_with(text)).card.map(|card| card.impl_status)
}

#[rstest]
#[case::hone(
    "Whenever ~ attacks, put a hone counter on each Equipment you control. (Each hone counter on an Equipment grants +1/+0 to equipped creature.)",
    Some(ImplStatus::Manual)
)]
#[case::a_counter_explained_in_an_activated_ability(
    "{2}: Put a hone counter on target Equipment. (Each hone counter grants +1/+0 to equipped creature.)",
    Some(ImplStatus::Manual)
)]
#[case::a_counter_without_reminder_text(
    "Whenever ~ attacks, put a hone counter on each Equipment you control.",
    Some(ImplStatus::Full)
)]
#[case::reminder_text_about_something_else(
    "When ~ enters, create a Treasure token. (It's an artifact with \"{T}, Sacrifice this token: Add one mana of any color.\")",
    Some(ImplStatus::Full)
)]
#[case::reminder_text_of_a_counter_in_a_keyword_line(
    "Training (Whenever this creature attacks with another creature with greater power, put a +1/+1 counter on this creature.)",
    Some(ImplStatus::Manual)
)]
fn reminder_text_that_explains_a_counter_keeps_the_clause_with_the_table(
    #[case] text: &str,
    #[case] status: Option<ImplStatus>,
) {
    assert_eq!(status_of(text), status, "{text}");
}
