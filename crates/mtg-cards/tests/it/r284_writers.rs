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
