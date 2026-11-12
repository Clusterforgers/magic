//! Goal: the r28-4 wordings read back exactly the shapes they name and nothing near them: a target spell or permanent given colors for good or a chosen color until end of turn, a creature made to attack or block this turn, a target that dealt damage this turn, a change then damage to that creature, damage from sources of one color, the Defiler's 2 life for one symbol, and splice costs in words.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::{read_keywords, written_variants};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::{Keyword, SpliceOnto};
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::Amount;
use mtg_core::card::{CardType, Subtype};
use rstest::rstest;

const SPELL_OR_PERMANENT: &str =
    r#"[{"what":{"Object":[{"AnyOf":["Spell",{"In":"Battlefield"}]}]}}]"#;
const CREATURE: &str = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;

fn spell(targets: &str, effects: &str) -> String {
    format!(r#"{{"Spell":{{"targets":{targets},"effects":{effects}}}}}"#)
}

fn colors(colors: &str, duration: &str) -> String {
    format!(
        r#"[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"SetColors":{colors}}}],"duration":"{duration}"}}}}]"#
    )
}

const CHOSEN: &str = r#"[{"ChooseValue":{"kind":"Color","as":"c"}},{"Continuous":{"affected":{"Target":0},"mods":[{"SetColorsChosen":"c"}],"duration":"UntilEndOfTurn"}}]"#;
const ATTACKS: &str =
    r#"[{"Rule":{"rule":{"MustAttack":{"Target":0}},"duration":"UntilEndOfTurn"}}]"#;
const BLOCKS: &str =
    r#"[{"Rule":{"rule":{"MustBlock":{"Target":0}},"duration":"UntilEndOfTurn"}}]"#;
const ATTACKS_FOR_GOOD: &str =
    r#"[{"Rule":{"rule":{"MustAttack":{"Target":0}},"duration":"Forever"}}]"#;
const SHRINK: &str = r#"[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":-5,"toughness":-5}}],"duration":"UntilEndOfTurn"}}]"#;
const DEALT: &str =
    r#"[{"what":{"Object":[{"Type":"Creature"},{"DidThisTurn":{"event":"Damaged"}}]}}]"#;
const ATTACKED: &str =
    r#"[{"what":{"Object":[{"Type":"Creature"},{"DidThisTurn":{"event":"Attacked"}}]}}]"#;
const FLIER: &str = r#"[{"what":{"Object":[{"Type":"Creature"},{"HasKeyword":"Flying"}]}}]"#;
const GROUNDED_THEN_FORESTS: &str = r#"[{"Continuous":{"affected":{"Target":0},"mods":[{"RemoveKeyword":"Flying"}],"duration":"UntilEndOfTurn"}},{"DealDamage":{"to":{"Target":0},"amount":{"Count":[{"Subtype":"Forest"},{"ControlledBy":"You"}]}}}]"#;
const FORESTS_THEN_GROUNDED: &str = r#"[{"DealDamage":{"to":{"Target":0},"amount":{"Count":[{"Subtype":"Forest"},{"ControlledBy":"You"}]}}},{"Continuous":{"affected":{"Target":0},"mods":[{"RemoveKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]"#;
const PUMP_AND_COLORLESS_SHIELD: &str = r#"[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}},{"Shield":{"when":{"Damage":{"to":{"Target":0},"source":["Colorless"]}},"by":"Prevent","duration":"UntilEndOfTurn"}}]"#;
const PUMP_AND_RED_SHIELD: &str = r#"[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}},{"Shield":{"when":{"Damage":{"to":{"Target":0},"source":[{"Color":"Red"}]}},"by":"Prevent","duration":"UntilEndOfTurn"}}]"#;

fn wordings(json: &str) -> Vec<String> {
    written_variants(&serde_json::from_str::<Ability>(json).expect("an ability"))
}

#[rstest]
#[case::black(spell(SPELL_OR_PERMANENT, &colors(r#"["Black"]"#, "Forever")), "Target spell or permanent becomes black.", true)]
#[case::colorless(spell(SPELL_OR_PERMANENT, &colors("[]", "Forever")), "Target spell or permanent becomes colorless.", true)]
#[case::black_this_turn_only(spell(SPELL_OR_PERMANENT, &colors(r#"["Black"]"#, "UntilEndOfTurn")), "Target spell or permanent becomes black.", false)]
#[case::a_creature_is_no_spell_or_permanent(spell(CREATURE, &colors(r#"["Black"]"#, "Forever")), "Target spell or permanent becomes black.", false)]
#[case::two_colors(spell(SPELL_OR_PERMANENT, &colors(r#"["Black","Red"]"#, "Forever")), "Target spell or permanent becomes black.", false)]
#[case::chosen(
    spell(SPELL_OR_PERMANENT, CHOSEN),
    "Target spell or permanent becomes the color of your choice until end of turn.",
    true
)]
#[case::attacks(
    spell(CREATURE, ATTACKS),
    "Target creature attacks this turn if able.",
    true
)]
#[case::blocks(
    spell(CREATURE, BLOCKS),
    "Target creature blocks this turn if able.",
    true
)]
#[case::attacks_for_good(
    spell(CREATURE, ATTACKS_FOR_GOOD),
    "Target creature attacks this turn if able.",
    false
)]
#[case::dealt_damage(
    spell(DEALT, SHRINK),
    "Target creature that dealt damage this turn gets -5/-5 until end of turn.",
    true
)]
#[case::attacked_is_not_dealt_damage(
    spell(ATTACKED, SHRINK),
    "Target creature that dealt damage this turn gets -5/-5 until end of turn.",
    false
)]
#[case::change_then_damage(
    spell(FLIER, GROUNDED_THEN_FORESTS),
    "Target creature with flying loses flying until end of turn. ~ deals damage to that creature equal to the number of Forests you control.",
    true
)]
#[case::damage_then_change(
    spell(FLIER, FORESTS_THEN_GROUNDED),
    "Target creature with flying loses flying until end of turn. ~ deals damage to that creature equal to the number of Forests you control.",
    false
)]
#[case::colorless_sources(
    spell(CREATURE, PUMP_AND_COLORLESS_SHIELD),
    "Target creature gets +2/+2 until end of turn. Prevent all damage that would be dealt to that creature this turn by colorless sources.",
    true
)]
#[case::red_sources_are_not_colorless(
    spell(CREATURE, PUMP_AND_RED_SHIELD),
    "Target creature gets +2/+2 until end of turn. Prevent all damage that would be dealt to that creature this turn by colorless sources.",
    false
)]
fn a_spell_reads_back_only_in_its_shape(
    #[case] ability: String,
    #[case] text: &str,
    #[case] written: bool,
) {
    assert_eq!(
        wordings(&ability).iter().any(|found| found == text),
        written,
        "{:?}",
        wordings(&ability)
    );
}

const VIGOR: &str = "As an additional cost to cast green permanent spells, you may pay 2 life. Those spells cost {G} less to cast if you paid life this way. This effect reduces only the amount of green mana you pay.";
const PERMANENTS: &str = r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}"#;

fn defiler(who: &str, color: &str, spells: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"PayLifeForOneSymbol":{{"who":"{who}","color":"{color}","spells":{spells}}}}}}}}}"#
    )
}

#[rstest]
#[case::green_permanents(defiler("You", "Green", &format!(r#"[{{"Color":"Green"}},{PERMANENTS}]"#)), true)]
#[case::creatures_only(
    defiler("You", "Green", r#"[{"Color":"Green"},{"Type":"Creature"}]"#),
    false
)]
#[case::red_symbol(defiler("You", "Red", &format!(r#"[{{"Color":"Green"}},{PERMANENTS}]"#)), false)]
#[case::opponents(defiler("Opponents", "Green", &format!(r#"[{{"Color":"Green"}},{PERMANENTS}]"#)), false)]
fn the_defiler_rule_reads_back_only_as_printed(#[case] ability: String, #[case] written: bool) {
    assert_eq!(
        wordings(&ability).iter().any(|found| found == VIGOR),
        written,
        "{:?}",
        wordings(&ability)
    );
}

#[rstest]
#[case::sacrifice("Splice onto Arcane\u{2014}Sacrifice two Mountains.", Some(CostItem::Sacrifice { filter: Selector(vec![Pred::Subtype(Subtype("Mountain".into()))]), count: Amount::Fixed(2) }))]
#[case::tap("Splice onto Arcane\u{2014}Tap an untapped white creature you control.", Some(CostItem::TapPermanents { filter: Selector(vec![Pred::Type(CardType::Creature), Pred::Color(mtg_core::color::Color::White)]), count: Amount::Fixed(1) }))]
#[case::an_opponent_s_life("Splice onto Arcane\u{2014}An opponent gains 5 life.", None)]
#[case::no_dash("Splice onto Arcane Sacrifice two Mountains.", None)]
fn a_splice_cost_in_words_is_read(#[case] line: &str, #[case] part: Option<CostItem>) {
    let read = read_keywords(line);
    let expected = part.map(|part| {
        vec![Keyword::Splice {
            onto: SpliceOnto::Arcane,
            cost: Cost(vec![part]),
        }]
    });
    assert_eq!(read, expected);
}

#[rstest]
#[case::spell_mastery_flash(
    "If there are two or more instant and/or sorcery cards in your graveyard, you may cast this spell as though it had flash.",
    true
)]
#[case::a_conditional_draw(
    "If there are two or more instant and/or sorcery cards in your graveyard, draw a card.",
    false
)]
fn a_flash_permission_put_after_its_condition_is_a_rule_of_the_spell(
    #[case] text: &str,
    #[case] rule: bool,
) {
    let kind = mtg_cards::compile::classify_clause(text, true);
    assert_eq!(
        kind == mtg_cards::compile::ClauseKind::Static,
        rule,
        "{kind:?}"
    );
}

const SPELL_MASTERY_FLASH: &str = r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"flash":true}},"condition":{"Compare":{"left":{"Count":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"OwnedBy":"You"},{"In":"Graveyard"}]},"op":"Ge","right":2}},"from":"Hand"}}"#;
const KNIGHTS_FROM_GRAVEYARD: &str =
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Subtype":"Knight"}],"from":"Graveyard"}}}}"#;
const KNIGHTS_FREE_FROM_GRAVEYARD: &str = r#"{"Rule":{"rule":{"MayCast":{"what":[{"Subtype":"Knight"}],"from":"Graveyard","free":true}}}}"#;
const DINOSAUR: &str = r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Creature"},{"Subtype":"Dinosaur"}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{}}}},{"Shuffle":{}},{"Continuous":{"affected":{"Bound":"found"},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilYourNextTurn"}}]}}"#;
const DINOSAUR_THIS_TURN: &str = r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Creature"},{"Subtype":"Dinosaur"}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{}}}},{"Shuffle":{}},{"Continuous":{"affected":{"Bound":"found"},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#;
const TREACHEROUS: &str = r#"{"AdditionalCost":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"},{"DidThisTurn":{"event":"Damaged"}}]}}],"required":true}}"#;
const PROCESSOR: &str = r#"{"AdditionalCost":{"cost":[{"ExiledToGraveyard":{"filter":[{"OwnedBy":"Opponents"}]}}],"required":true}}"#;
const PROCESSOR_OWN: &str = r#"{"AdditionalCost":{"cost":[{"ExiledToGraveyard":{"filter":[{"OwnedBy":"You"}]}}],"required":true}}"#;

#[rstest]
#[case::spell_mastery(
    SPELL_MASTERY_FLASH,
    "If there are two or more instant and/or sorcery cards in your graveyard, you may cast this spell as though it had flash.",
    true
)]
#[case::knights(
    KNIGHTS_FROM_GRAVEYARD,
    "As long as ~ is on the battlefield, you may cast Knight spells from your graveyard.",
    true
)]
#[case::free_knights(
    KNIGHTS_FREE_FROM_GRAVEYARD,
    "As long as ~ is on the battlefield, you may cast Knight spells from your graveyard.",
    false
)]
#[case::dinosaur(
    DINOSAUR,
    "Search your library for a Dinosaur creature card, put it onto the battlefield, then shuffle. It gains indestructible until your next turn.",
    true
)]
#[case::dinosaur_this_turn(
    DINOSAUR_THIS_TURN,
    "Search your library for a Dinosaur creature card, put it onto the battlefield, then shuffle. It gains indestructible until your next turn.",
    false
)]
#[case::sacrifice_one_that_dealt_damage(
    TREACHEROUS,
    "As an additional cost to cast ~, sacrifice a creature that dealt damage this turn.",
    true
)]
#[case::exiled_card_to_its_owner(
    PROCESSOR,
    "As an additional cost to cast this spell, put a card an opponent owns from exile into that player's graveyard.",
    true
)]
#[case::own_exiled_card(
    PROCESSOR_OWN,
    "As an additional cost to cast this spell, put a card an opponent owns from exile into that player's graveyard.",
    false
)]
fn a_rule_or_cost_reads_back_only_in_its_shape(
    #[case] ability: &str,
    #[case] text: &str,
    #[case] written: bool,
) {
    assert_eq!(
        wordings(ability).iter().any(|found| found == text),
        written,
        "{:?}",
        wordings(ability)
    );
}

fn merfolk_anthem(event: &str, at_least: u32) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Subtype":"Merfolk"}},{{"ControlledBy":"You"}}]}},"mods":[{{"ModifyPT":{{"power":1,"toughness":0}}}}],"condition":{{"Compare":{{"left":{{"Tally":{{"event":"{event}","who":"You","filter":[{{"Subtype":"Merfolk"}}]}}}},"op":"Ge","right":{at_least}}}}}}}}}"#
    )
}

#[rstest]
#[case::three_attacked(merfolk_anthem("Attacked", 3), true)]
#[case::two_attacked(merfolk_anthem("Attacked", 2), false)]
#[case::three_blocked(merfolk_anthem("Blocked", 3), false)]
fn an_anthem_under_attacks_this_turn_reads_back_only_as_printed(
    #[case] ability: String,
    #[case] written: bool,
) {
    let text = "As long as you attacked with three or more Merfolk this turn, Merfolk you control get +1/+0.";
    assert_eq!(
        wordings(&ability).iter().any(|found| found == text),
        written,
        "{:?}",
        wordings(&ability)
    );
}

const MELTING: &str =
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":[{"RemoveSupertype":"Snow"}]}}"#;
const MELTING_CREATURES: &str =
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"RemoveSupertype":"Snow"}]}}"#;
const SIDESWIPE: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Subtype":"Arcane"}]}}],"effects":[{"ChangeTargets":{"what":{"Target":0},"to":"Choose"}}]}}"#;
const SIDESWIPE_ANOTHER: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Subtype":"Arcane"}]}}],"effects":[{"ChangeTargets":{"what":{"Target":0},"to":"Another"}}]}}"#;
const PLAGIARIZE: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Shield":{"when":{"Draw":{"who":{"Target":0}}},"by":{"Instead":[{"Draw":{}}]},"duration":"UntilEndOfTurn"}}]}}"#;
const PLAGIARIZE_FOREVER: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Shield":{"when":{"Draw":{"who":{"Target":0}}},"by":{"Instead":[{"Draw":{}}]},"duration":"Forever"}}]}}"#;
const IGNITE: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"ChooseAtRandom":{"from":{"Each":[{"OwnedBy":{"Target":0}},{"In":"Hand"}]},"as":"revealed"}},{"Reveal":{"what":{"Bound":"revealed"}}},{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":{"Bound":"revealed"},"stat":"ManaValue"}}}}]}}"#;
const IGNITE_POWER: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"ChooseAtRandom":{"from":{"Each":[{"OwnedBy":{"Target":0}},{"In":"Hand"}]},"as":"revealed"}},{"Reveal":{"what":{"Bound":"revealed"}}},{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":{"Bound":"revealed"},"stat":"Power"}}}}]}}"#;
const OPERATIVE: &str = r#"{"Static":{"affected":"This","mods":[{"SetCreatureTypes":["Human","Citizen"]},{"SetPT":{"power":1,"toughness":1}},"CantBeBlocked"],"condition":{"Matches":{"subject":"This","selector":["Tapped"]}}}}"#;
const OPERATIVE_UNTAPPED: &str = r#"{"Static":{"affected":"This","mods":[{"SetCreatureTypes":["Human","Citizen"]},{"SetPT":{"power":1,"toughness":1}},"CantBeBlocked"],"condition":{"Matches":{"subject":"This","selector":["Untapped"]}}}}"#;

#[rstest]
#[case::melting(MELTING, "All lands are no longer snow.", true)]
#[case::melting_creatures(MELTING_CREATURES, "All lands are no longer snow.", false)]
#[case::sideswipe(SIDESWIPE, "You may change any targets of target Arcane spell.", true)]
#[case::sideswipe_one_target(
    SIDESWIPE_ANOTHER,
    "You may change any targets of target Arcane spell.",
    false
)]
#[case::plagiarize(
    PLAGIARIZE,
    "Until end of turn, if target player would draw a card, instead that player skips that draw and you draw a card.",
    true
)]
#[case::plagiarize_for_good(
    PLAGIARIZE_FOREVER,
    "Until end of turn, if target player would draw a card, instead that player skips that draw and you draw a card.",
    false
)]
#[case::ignite(
    IGNITE,
    "Target player reveals a card at random from their hand. ~ deals damage to that player equal to that card's mana value.",
    true
)]
#[case::ignite_by_power(
    IGNITE_POWER,
    "Target player reveals a card at random from their hand. ~ deals damage to that player equal to that card's mana value.",
    false
)]
#[case::operative(
    OPERATIVE,
    "As long as ~ is tapped, it's a Human Citizen with base power and toughness 1/1 and can't be blocked.",
    true
)]
#[case::operative_untapped(
    OPERATIVE_UNTAPPED,
    "As long as ~ is tapped, it's a Human Citizen with base power and toughness 1/1 and can't be blocked.",
    false
)]
fn an_exact_shape_reads_back_only_as_itself(
    #[case] ability: &str,
    #[case] text: &str,
    #[case] written: bool,
) {
    assert_eq!(
        wordings(ability).iter().any(|found| found == text),
        written,
        "{:?}",
        wordings(ability)
    );
}

const BREACH: &str = r#"{"Spell":{"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Creature"}]},"as":"creature"}},{"Move":{"what":{"Bound":"creature"},"to":{"Battlefield":{}}}},{"Continuous":{"affected":{"Bound":"creature"},"mods":[{"AddKeyword":"Haste"}],"duration":"Forever"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"creature"}}]}}]}}]}}]}}"#;
const BREACH_KEPT: &str = r#"{"Spell":{"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Creature"}]},"as":"creature"}},{"Move":{"what":{"Bound":"creature"},"to":{"Battlefield":{}}}},{"Continuous":{"affected":{"Bound":"creature"},"mods":[{"AddKeyword":"Haste"}],"duration":"Forever"}}]}}]}}"#;

#[rstest]
#[case::breach(BREACH, true)]
#[case::kept(BREACH_KEPT, false)]
fn a_creature_put_in_for_a_turn_reads_back_only_with_its_sacrifice(
    #[case] ability: &str,
    #[case] written: bool,
) {
    let text = "You may put a creature card from your hand onto the battlefield. That creature gains haste. Sacrifice that creature at the beginning of the next end step.";
    assert_eq!(
        wordings(ability).iter().any(|found| found == text),
        written,
        "{:?}",
        wordings(ability)
    );
}
