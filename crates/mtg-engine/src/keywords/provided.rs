//! Goal: a keyword built from triggers (ward, prowess, battle cry) adds its abilities wherever the keyword is: after the keyword on the card and after the effect or static ability that gives it, under an address that names the keyword and the place among the abilities it adds, which reads back the very ability it was listed with; the abilities go with the keyword when an effect takes it away or takes every ability away, each instance of a keyword adds its own, and they are rules the DSL validates and the engine's capabilities cover.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::load::{Library, Source};
use mtg_cards::modify::Modification as Wanted;
use mtg_cards::refs::{Pred, Ref, Selector as Select, ZoneKind};
use mtg_cards::support::oracle_fingerprint;
use mtg_cards::validate::used_primitives;
use mtg_core::card::CardType;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use rstest::rstest;

use super::{keywords_of, rules};
use crate::abilities::{AbilityRef, AbilitySource, Grant};
use crate::ability_reader::{abilities_of, ability};
use crate::caps::caps;
use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
use crate::events::Event;
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::state::GameState;
use crate::triggers::{introduces, triggers_something};
use crate::zones::Zone;

fn ward() -> Keyword {
    Keyword::Ward(Cost::mana("{2}".parse().unwrap()))
}

/// A 2/2 creature that prints these keywords, in this order.
fn printing(keywords: &[Keyword]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Holder")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for keyword in keywords {
        card = card.with_rules(Ability::Keyword(keyword.clone()));
    }
    Arc::new(card)
}

/// Seat 0 controls the `printing` as its only permanent.
fn board(keywords: &[Keyword]) -> (GameState, ObjectId) {
    let game = state(vec![vec![], vec![]], vec![vec![printing(keywords)], vec![]]);
    let id = game.zone(Zone::Battlefield)[0];
    (game, id)
}

fn affect(game: &mut GameState, id: ObjectId, modification: Modification, n: u64) {
    let effect = ContinuousEffect::new(
        EffectId(n),
        Selector::objects(game, [id]),
        modification,
        Duration::UntilEndOfTurn,
        n,
    );
    apply_event(game, &Event::EffectCreated { effect }).unwrap();
}

/// What the reader lists for `id`: the source of each address and its index.
fn addresses(game: &GameState, id: ObjectId) -> Vec<(AbilitySource, usize)> {
    abilities_of(game, id)
        .into_iter()
        .map(|(address, _)| (address.source, address.ability))
        .collect()
}

/// The keyword abilities among what the reader lists, and the abilities keywords added (triggered abilities here).
fn counts(game: &GameState, id: ObjectId) -> (usize, usize) {
    let listed = abilities_of(game, id);
    let keywords = listed
        .iter()
        .filter(|(_, rules)| matches!(**rules, Ability::Keyword(_)))
        .count();
    let triggered = listed
        .iter()
        .filter(|(_, rules)| matches!(**rules, Ability::Triggered { .. }))
        .count();
    (keywords, triggered)
}

#[rstest]
#[case::prowess_after_haste(
    &[Keyword::Haste, Keyword::Prowess],
    &[
        (AbilitySource::Printed, 0),
        (AbilitySource::Printed, 1),
        (AbilitySource::PrintedKeyword(1), 0),
    ]
)]
#[case::each_keyword_adds_after_itself(
    &[ward(), Keyword::BattleCry, Keyword::Flying],
    &[
        (AbilitySource::Printed, 0),
        (AbilitySource::PrintedKeyword(0), 0),
        (AbilitySource::Printed, 1),
        (AbilitySource::PrintedKeyword(1), 0),
        (AbilitySource::Printed, 2),
    ]
)]
#[case::a_keyword_of_hooks_alone_adds_nothing(
    &[Keyword::Flying, Keyword::Trample],
    &[(AbilitySource::Printed, 0), (AbilitySource::Printed, 1)]
)]
fn a_printed_keyword_adds_its_abilities_right_after_itself(
    #[case] printed: &[Keyword],
    #[case] expected: &[(AbilitySource, usize)],
) {
    let (game, id) = board(printed);
    assert_eq!(addresses(&game, id), expected, "with the layers");
    // The same list when the layers have nothing to work out and when the object is a card in a hand.
    let (idle, held) = board(&[]);
    assert_eq!(addresses(&idle, held).len(), 0);
    let in_hand = state(vec![vec![printing(printed)], vec![]], vec![vec![], vec![]]);
    let card = in_hand.zone(Zone::Hand(PlayerId(0)))[0];
    assert_eq!(addresses(&in_hand, card), expected, "in a hand");
}

#[test]
fn every_address_reads_back_the_ability_it_was_listed_with() {
    let (mut game, id) = board(&[Keyword::Prowess, ward()]);
    affect(
        &mut game,
        id,
        Modification::AddKeyword(Keyword::BattleCry),
        3,
    );
    let listed = abilities_of(&game, id);
    assert_eq!(listed.len(), 2 + 2 + 1);
    for (address, body) in &listed {
        assert_eq!(
            ability(&game, *address),
            Some(Arc::clone(body)),
            "{address:?}"
        );
    }
    // An index past the abilities the keyword adds, and a keyword rule that is not one, name nothing.
    let past = AbilityRef::keyword(id, AbilitySource::PrintedKeyword(0), 1);
    assert_eq!(ability(&game, past), None);
    let not_a_keyword = AbilityRef::keyword(id, AbilitySource::PrintedKeyword(9), 0);
    assert_eq!(ability(&game, not_a_keyword), None);
    let ungranted = AbilityRef::keyword(
        id,
        AbilitySource::GrantedKeyword(Grant::Effect(EffectId(8))),
        0,
    );
    assert_eq!(ability(&game, ungranted), None);
}

#[test]
fn a_granted_keyword_adds_its_abilities_under_the_effect_that_gave_it() {
    let (mut game, id) = board(&[Keyword::Haste]);
    affect(&mut game, id, Modification::AddKeyword(Keyword::Prowess), 5);
    assert_eq!(
        addresses(&game, id),
        [
            (AbilitySource::Printed, 0),
            (AbilitySource::GrantedKeyword(Grant::Effect(EffectId(5))), 0),
        ]
    );
    assert_eq!(keywords_of(&game, id), [&Keyword::Haste, &Keyword::Prowess]);
}

#[test]
fn a_keyword_given_by_a_static_ability_is_named_by_the_static() {
    let lord = Arc::new(
        CardDefinition::new("Lord")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Static {
                affected: Ref::Each(Select(vec![Pred::Type(CardType::Creature), Pred::Other])),
                mods: vec![Wanted::AddKeyword(ward())],
                condition: None,
                defining: false,
                from: ZoneKind::Battlefield,
            }),
    );
    let game = state(
        vec![vec![], vec![]],
        vec![vec![lord, printing(&[Keyword::Haste])], vec![]],
    );
    let [lord, holder] = [0, 1].map(|at| game.zone(Zone::Battlefield)[at]);
    let given = AbilitySource::GrantedKeyword(Grant::Static {
        source: lord,
        rule: 0,
        index: 0,
    });
    assert_eq!(
        addresses(&game, holder),
        [(AbilitySource::Printed, 0), (given, 0)]
    );
    let address = AbilityRef::keyword(holder, given, 0);
    let read = ability(&game, address).unwrap();
    assert_eq!(*read, rules(&ward()).adds_abilities(&ward())[0]);
    // The lord keeps what it gives to others to others.
    assert_eq!(addresses(&game, lord), [(AbilitySource::Printed, 0)]);
}

#[rstest]
#[case::only_the_keyword_taken_away(Modification::RemoveKeyword(Keyword::Prowess), 1, 0)]
#[case::another_keyword_leaves_them(Modification::RemoveKeyword(Keyword::Haste), 1, 1)]
#[case::every_ability_taken_away(Modification::LoseAbilities, 0, 0)]
fn an_effect_that_takes_a_keyword_away_takes_what_it_added(
    #[case] taken: Modification,
    #[case] keywords: usize,
    #[case] triggered: usize,
) {
    let (mut game, id) = board(&[Keyword::Haste, Keyword::Prowess]);
    assert_eq!(counts(&game, id), (2, 1));
    affect(&mut game, id, taken, 1);
    assert_eq!(counts(&game, id), (keywords, triggered));
    assert_eq!(keywords_of(&game, id).len(), keywords);
}

#[rstest]
#[case::given_after_the_loss(&[Modification::LoseAbilities, Modification::AddKeyword(Keyword::BattleCry)], 1)]
#[case::given_before_the_loss(&[Modification::AddKeyword(Keyword::BattleCry), Modification::LoseAbilities], 0)]
#[case::given_then_taken_away(&[Modification::AddKeyword(Keyword::BattleCry), Modification::RemoveKeyword(Keyword::BattleCry)], 1)]
fn the_order_of_the_effects_decides_which_abilities_a_keyword_leaves(
    #[case] effects: &[Modification],
    #[case] triggered: usize,
) {
    let (mut game, id) = board(&[Keyword::Prowess]);
    for (n, modification) in effects.iter().enumerate() {
        affect(&mut game, id, modification.clone(), n as u64 + 1);
    }
    assert_eq!(counts(&game, id).1, triggered);
}

#[test]
fn each_instance_of_a_keyword_adds_its_own_abilities() {
    let (mut game, id) = board(&[Keyword::Prowess, Keyword::Prowess]);
    affect(&mut game, id, Modification::AddKeyword(Keyword::Prowess), 4);
    // Three instances, three triggers (CR 702.108b); the keyword list names it as often as the card prints it.
    assert_eq!(counts(&game, id).1, 3);
    let names = addresses(&game, id);
    let mut distinct = names.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), names.len(), "no two abilities share a name");
}

#[test]
fn an_effect_names_only_a_permanent() {
    let (mut game, id) = board(&[]);
    let hand = state(vec![vec![printing(&[])], vec![]], vec![vec![], vec![]]);
    let card = hand.zone(Zone::Hand(PlayerId(0)))[0];
    affect(&mut game, id, Modification::AddKeyword(Keyword::Prowess), 1);
    assert_eq!(counts(&game, id), (0, 1));
    assert_eq!(
        counts(&hand, card),
        (0, 0),
        "a card in a hand is not given keywords"
    );
}

#[test]
fn what_can_trigger_follows_the_keywords_that_add_triggers() {
    for (keyword, adds) in [
        (Keyword::Prowess, true),
        (Keyword::BattleCry, true),
        (ward(), true),
        (Keyword::Flying, false),
        (Keyword::Equip(Cost::mana("{1}".parse().unwrap())), false),
    ] {
        assert_eq!(triggers_something(&Ability::Keyword(keyword.clone())), adds);
        let (game, _) = board(std::slice::from_ref(&keyword));
        assert_eq!(game.watches(), adds, "{keyword:?} printed");
        let creation = Event::EffectCreated {
            effect: ContinuousEffect::new(
                EffectId(1),
                Selector::Objects(Vec::new()),
                Modification::AddKeyword(keyword.clone()),
                Duration::UntilEndOfTurn,
                1,
            ),
        };
        assert_eq!(introduces(&[creation]), adds, "{keyword:?} given");
    }
}

/// A card file whose only face has these abilities, for the loader's validation to read.
fn library_of(abilities: &[Ability]) -> Result<Library, String> {
    let text = "Holder";
    let json = serde_json::json!({
        "oracle_id": "00000000-0000-4000-8000-000000000001",
        "name": "Holder",
        "impl_status": "Full",
        "faces": [{
            "name": "Holder",
            "oracle_fingerprint": oracle_fingerprint(text),
            "abilities": abilities,
        }],
    });
    Library::from_sources(&[Source {
        path: "cards/holder.json".into(),
        text: json.to_string(),
    }])
    .map_err(|errors| errors.to_string())
}

#[rstest]
#[case::prowess(Keyword::Prowess)]
#[case::battle_cry(Keyword::BattleCry)]
#[case::ward_with_mana(ward())]
#[case::ward_with_other_mana(Keyword::Ward(Cost::mana("{1}{U}".parse().unwrap())))]
fn the_abilities_a_keyword_adds_are_valid_rules_and_the_engine_runs_them(#[case] keyword: Keyword) {
    let added = rules(&keyword).adds_abilities(&keyword);
    assert_eq!(added.len(), 1);
    library_of(&added).unwrap_or_else(|errors| panic!("{errors}"));
    let caps = caps();
    for ability in std::iter::once(&Ability::Keyword(keyword.clone())).chain(&added) {
        let missing = caps.missing(&used_primitives(ability));
        assert!(missing.is_empty(), "{keyword:?}: {missing:?}");
    }
}

#[rstest]
#[case::equip(Keyword::Equip(Cost::mana("{1}".parse().unwrap())), 1)]
#[case::equip_with_colored_mana(Keyword::Equip(Cost::mana("{R}{R}".parse().unwrap())), 1)]
#[case::reconfigure(Keyword::Reconfigure(Cost::mana("{2}".parse().unwrap())), 3)]
#[case::living_weapon(Keyword::LivingWeapon, 1)]
#[case::for_mirrodin(Keyword::ForMirrodin, 1)]
#[case::job_select(Keyword::JobSelect, 1)]
#[case::equip_to(Keyword::EquipTo { to: mtg_cards::refs::Selector(vec![mtg_cards::refs::Pred::Supertype(mtg_core::card::Supertype::Legendary)]), cost: Cost::mana("{1}".parse().unwrap()) }, 1)]
#[case::reconfigure_with_colored_mana(Keyword::Reconfigure(Cost::mana("{R}".parse().unwrap())), 3)]
fn the_abilities_of_an_attachment_keyword_are_valid_rules_and_the_engine_runs_them(
    #[case] keyword: Keyword,
    #[case] count: usize,
) {
    let added = rules(&keyword).adds_abilities(&keyword);
    assert_eq!(added.len(), count);
    library_of(&added).unwrap_or_else(|errors| panic!("{errors}"));
    let caps = caps();
    for ability in std::iter::once(&Ability::Keyword(keyword.clone())).chain(&added) {
        let missing = caps.missing(&used_primitives(ability));
        assert!(missing.is_empty(), "{keyword:?}: {missing:?}");
    }
}

#[rstest]
#[case::life(r#"[{"PayLife":2}]"#, &[])]
#[case::mana_and_life(r#"[{"Mana":"{1}"},{"PayLife":2}]"#, &[])]
#[case::discarding_a_card(r#"[{"Discard":{}}]"#, &[])]
#[case::sacrificing_a_permanent(r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#, &[])]
#[case::energy(r#"[{"PayEnergy":2}]"#, &[])]
#[case::losing_the_source(r#"["SacrificeSelf"]"#, &["Effect.Pay.SacrificeSelf"])]
#[case::an_x(r#"[{"PayLife":"X"}]"#, &["Amount.X.Elsewhere", "Effect.Pay.X"])]
fn a_ward_cost_the_engine_cannot_pay_keeps_the_card_from_being_verified(
    #[case] cost: &str,
    #[case] named: &[&str],
) {
    let ward = Keyword::Ward(serde_json::from_str(cost).unwrap());
    let missing = caps().missing(&used_primitives(&Ability::Keyword(ward)));
    assert_eq!(missing, named);
}
