//! Goal: each fault the compiler's output showed against a full Oracle Cards download stays fixed, so the cards that had it generate valid rules or none: a keyword Scryfall lists is explained by a grant or a landwalk in the text, a spell has no event for "that much" to name, a partner is the card itself or a card that exists, and the X of "mana value X" is no X the caster announces.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::ability::Ability;
use mtg_cards::card::{Card, ImplStatus};
use mtg_cards::compile::{compile, compile_among};
use mtg_cards::generated::{Generation, generate};
use mtg_cards::keyword::Keyword;
use mtg_cards::load::Library;
use mtg_cards::support::{PrintedCard, PrintedFace};
use rstest::rstest;

use crate::common::token_sources;

const ID: &str = "4457ed35-7c10-48c8-9776-456485fdf070";

/// A one-face card as the importer reads it, with the text of the real card.
fn printed(name: &str, type_line: &str, cost: &str, text: &str, keywords: &[&str]) -> PrintedCard {
    PrintedCard {
        layout: String::new(),
        oracle_id: ID.into(),
        name: name.into(),
        keywords: keywords
            .iter()
            .map(|keyword| (*keyword).to_owned())
            .collect(),
        faces: vec![PrintedFace {
            name: name.into(),
            mana_cost: cost.into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

fn names(card: &PrintedCard, others: &[&str]) -> BTreeSet<String> {
    others
        .iter()
        .copied()
        .chain([card.name.as_str()])
        .map(str::to_lowercase)
        .collect()
}

/// The rules generated for the card, or `None` when it generates nothing; an invalid rule fails the test.
fn rules_of(card: &PrintedCard, others: &[&str]) -> Option<Card> {
    let tokens = Library::from_sources(&token_sources()).expect("the token files load");
    match generate(card, &tokens, &names(card, others)) {
        Generation::Rules(rules) => Some(*rules),
        Generation::Nothing => None,
        Generation::Invalid(problems) => panic!("{}: {problems:?}", card.name),
    }
}

const AURA: &str = "Enchantment \u{2014} Aura";
const SOLDIER: &str = "Legendary Creature \u{2014} Human Soldier";

#[rstest]
#[case::a_grant_to_the_enchanted_creature(
    printed(
        "Asha's Favor", AURA, "{2}{W}",
        "Enchant creature\nEnchanted creature has flying, first strike, and vigilance.",
        &["First strike", "Enchant"],
    ),
    Some(ImplStatus::Full)
)]
#[case::a_grant_with_a_power_and_toughness(
    printed(
        "Super State", "Legendary Enchantment \u{2014} Aura", "{7}",
        "Enchant creature you control\nEnchanted creature has base power and toughness 9/9 and has flying, first strike, trample, and haste.\nWhenever enchanted creature deals combat damage to an opponent, it deals that much damage to each other opponent.",
        &["First strike", "Trample", "Enchant"],
    ),
    Some(ImplStatus::Partial)
)]
#[case::a_token_with_keywords(
    printed(
        "Battle at the Helvault", "Enchantment \u{2014} Saga", "{4}{W}{W}",
        "(As this Saga enters and after your draw step, add a lore counter. Sacrifice after III.)\nI, II \u{2014} For each player, exile up to one target non-Saga, nonland permanent that player controls until this Saga leaves the battlefield.\nIII \u{2014} Create Avacyn, a legendary 8/8 white Angel creature token with flying, vigilance, and indestructible.",
        &["Vigilance"],
    ),
    Some(ImplStatus::Partial)
)]
#[case::a_grant_to_other_creatures(
    printed(
        "Commander Mustard", SOLDIER, "{3}{R}{W}",
        "Vigilance\nOther Soldiers you control have vigilance, trample, and haste.\n{2}{R}{W}: Until end of turn, Soldiers you control gain \"Whenever this creature attacks, it deals 1 damage to defending player.\"",
        &["Vigilance", "Trample"],
    ),
    Some(ImplStatus::Partial)
)]
#[case::a_landwalk_left_to_the_table(
    printed(
        "Kevin, Questing Dragon", "Legendary Creature \u{2014} Dragon", "{4}{R}{R}{R}{R}",
        "Kevin can't be countered, devour 2\nFlying, mountainwalk, rampage 2, bushido 2, trample over planeswalkers\nDamage can't be prevented.\nPlayers can't gain life.\nWhenever Kevin deals combat damage to a player, gain control of target land that player controls. Untap it.",
        &["Flying", "Landwalk", "Bushido", "Trample", "Mountainwalk", "Rampage"],
    ),
    Some(ImplStatus::Partial)
)]
#[case::a_grant_to_a_type_of_creature(
    printed(
        "Zur, Eternal Schemer", "Legendary Creature \u{2014} Human Wizard", "{W}{U}{B}",
        "Flying\nEnchantment creatures you control have deathtouch, lifelink, and hexproof.\n{1}{W}: Target non-Aura enchantment you control becomes a creature in addition to its other types and has base power and base toughness each equal to its mana value.",
        &["Flying", "Lifelink"],
    ),
    Some(ImplStatus::Partial)
)]
#[case::that_much_in_a_spell(
    printed(
        "Foul-Tongue Shriek", "Instant", "{B}",
        "Target opponent loses 1 life for each attacking creature you control. You gain that much life.",
        &[],
    ),
    None
)]
#[case::a_partner_that_is_the_card_itself(
    printed(
        "Mothers Yamazaki", "Legendary Creature \u{2014} Human Samurai", "{2}{R}{W}",
        "Partner with itself (When this enters, target player may put Mothers Yamazaki into their hand from their library, then shuffle. A Commander deck can include two of this card, and they can be your commanders.)\nAs long as you control exactly two permanents named Mothers Yamazaki, the \"legend rule\" doesn't apply to them, and Samurai you control get +2/+2 and have vigilance and haste.",
        &["Partner with", "Partner"],
    ),
    Some(ImplStatus::Partial)
)]
#[case::a_partner_that_is_no_card(
    printed(
        "The Knight of Land Drops", "Legendary Creature \u{2014} Human Knight", "{1}{R}",
        "Partner with Knight (You may select any legendary Knight to be this creature's partner. It doesn't have to have Partner. You don't search for a Knight when this enters the battlefield.)\nKnights you control have haste and \"At the beginning of your end step, if you didn't play a land this turn, you may discard a card, then draw a card.\"",
        &["Partner with", "Partner"],
    ),
    None
)]
#[case::mana_value_x_in_a_cost(
    printed(
        "Nahiri's Sacrifice", "Sorcery", "{1}{R}",
        "As an additional cost to cast this spell, sacrifice an artifact or creature with mana value X.\n~ deals X damage divided as you choose among any number of target creatures.",
        &[],
    ),
    None
)]
fn the_cards_that_failed_the_checks_generate_valid_rules_or_none(
    #[case] card: PrintedCard,
    #[case] status: Option<ImplStatus>,
) {
    assert_eq!(rules_of(&card, &[]).map(|rules| rules.impl_status), status);
}

/// The partner the first ability of the card names, when it is a "Partner with" keyword.
fn partner_of(card: &Card) -> Option<String> {
    match card.faces.first()?.abilities.first()? {
        Ability::Keyword(Keyword::PartnerWith(name)) => Some(name.clone()),
        _ => None,
    }
}

const PIR: &str = "Pir, Imaginative Rascal";

#[rstest]
#[case::itself_is_the_card("Mothers Yamazaki", "Partner with itself", &[], Some("Mothers Yamazaki"))]
#[case::itself_in_any_case("Mothers Yamazaki", "Partner with Itself", &[], Some("Mothers Yamazaki"))]
#[case::a_card_of_the_library("Toothy, Imaginary Friend", "Partner with Pir, Imaginative Rascal", &[PIR], Some(PIR))]
#[case::a_name_in_any_case("Toothy, Imaginary Friend", "Partner with pir, imaginative rascal", &[PIR], Some("pir, imaginative rascal"))]
#[case::a_name_no_card_bears("The Knight of Land Drops", "Partner with Knight", &[PIR], None)]
#[case::a_name_no_card_bears_in_a_library_of_one("Toothy, Imaginary Friend", "Partner with Pir", &[], None)]
fn a_partner_is_the_card_itself_or_a_card_that_exists(
    #[case] name: &str,
    #[case] line: &str,
    #[case] library: &[&str],
    #[case] partner: Option<&str>,
) {
    let card = printed(name, "Legendary Creature \u{2014} Human", "{2}", line, &[]);
    let known = names(&card, library);
    let compiled = compile_among(&card, Some(&known)).card.unwrap();
    assert_eq!(partner_of(&compiled).as_deref(), partner, "{compiled:?}");
    assert_eq!(compiled.impl_status == ImplStatus::Full, partner.is_some());
}

#[test]
fn compiling_without_a_library_keeps_the_name_the_card_prints() {
    let card = printed(
        "The Knight of Land Drops",
        "Legendary Creature \u{2014} Human Knight",
        "{1}{R}",
        "Partner with Knight",
        &[],
    );
    let compiled = compile(&card).card.unwrap();
    assert_eq!(partner_of(&compiled).as_deref(), Some("Knight"));
}

/// Whether some ability of the card names the amount of an event (`Triggering`) or counts X.
fn mentions(card: &Card, needle: &str) -> bool {
    serde_json::to_string(card).unwrap().contains(needle)
}

#[rstest]
#[case::the_stat_of_what_the_cost_takes(
    "As an additional cost to cast this spell, sacrifice an artifact or creature with mana value X.\n~ deals X damage to any target."
)]
#[case::the_power_of_what_the_cost_takes(
    "As an additional cost to cast this spell, sacrifice a creature with power X.\n~ deals X damage to any target."
)]
fn the_x_of_a_stat_in_a_cost_is_no_x_the_caster_announces(#[case] text: &str) {
    let card = printed("Test Spell", "Sorcery", "{1}{R}", text, &[]);
    let compiled = compile(&card);
    assert!(
        compiled.clauses.iter().all(|clause| !clause.read),
        "{:?}",
        compiled.clauses
    );
    assert!(compiled.card.is_none_or(|card| !mentions(&card, "\"X\"")));
}

#[rstest]
#[case::pay_x_life(
    "Sorcery",
    "{R}",
    "As an additional cost to cast this spell, pay X life.\n~ deals X damage to any target."
)]
#[case::sacrifice_x_creatures(
    "Sorcery",
    "{R}",
    "As an additional cost to cast this spell, sacrifice X creatures.\n~ deals X damage to any target."
)]
#[case::a_stat_named_by_the_x_of_the_cost(
    "Creature \u{2014} Avatar",
    "{4}",
    "{X}, Sacrifice an artifact with mana value X: ~ deals X damage to any target."
)]
#[case::a_stat_named_by_the_x_of_the_card(
    "Sorcery",
    "{X}{R}",
    "As an additional cost to cast this spell, sacrifice a creature with power X.\n~ deals X damage to any target."
)]
fn the_x_a_cost_counts_stays_an_x_the_caster_announces(
    #[case] type_line: &str,
    #[case] cost: &str,
    #[case] text: &str,
) {
    let card = printed("Test Spell", type_line, cost, text, &[]);
    let compiled = compile(&card);
    assert!(
        compiled.clauses.iter().any(|clause| clause.read),
        "{:?}",
        compiled.clauses
    );
}

#[rstest]
#[case::a_loss_then_a_gain("Instant", "Target opponent loses 3 life. You gain that much life.")]
#[case::damage_then_a_gain(
    "Instant",
    "~ deals 3 damage to target creature. You gain that much life."
)]
#[case::damage_then_a_loss(
    "Sorcery",
    "~ deals 2 damage to target creature. Target opponent loses that much life."
)]
fn a_spell_never_reads_that_much_as_the_amount_of_an_event(
    #[case] type_line: &str,
    #[case] text: &str,
) {
    let compiled = compile(&printed("Test Spell", type_line, "{B}", text, &[]));
    assert!(
        compiled
            .card
            .is_none_or(|card| !mentions(&card, "Triggering")),
        "{text}"
    );
}
