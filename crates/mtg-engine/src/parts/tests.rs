//! Goal: a cost is read into the parts the engine pays (life, counters removed from the source, sacrifices, discards, exiles from a graveyard, taps and returns to hand) with fixed amounts, and the name of the part is given for an amount it cannot count; each pick offers exactly the objects of its zone that the payer controls or holds, match its filter and are untapped for a tap, never the objects barred; each part pays with the event the engine already logs for it; a part is payable only with the life, the counters and the candidates it needs.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::*;
use crate::counters::CountersChanged;
use crate::events::ZoneCause;
use crate::fixtures::{creature, setup};
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

fn item(json: &str) -> CostItem {
    serde_json::from_str(json).unwrap()
}

fn artifact(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Artifact))
}

/// Seat 0 holds a Bear and a Relic, controls the Source (with three charge counters), a Wolf, an Ogre (tapped) and a
/// Relic, and has a Bear and an Elk in the graveyard; seat 1 controls a Giant.
fn game() -> GameState {
    let mut game = setup(
        vec![
            vec![creature("Held Bear", 2, 2), artifact("Held Relic")],
            vec![],
        ],
        vec![
            vec![
                creature("Source", 1, 1),
                creature("Wolf", 2, 2),
                creature("Ogre", 3, 3),
                artifact("Relic"),
            ],
            vec![creature("Giant", 5, 5)],
        ],
    );
    game.players[0].battlefield[0].counters = vec![(CounterKind::Charge, 3)];
    game.players[0].battlefield[2].tapped = true;
    game.players[0].graveyard = vec![creature("Dead Bear", 2, 2), creature("Dead Elk", 1, 1)];
    GameState::from_setup(&game).unwrap()
}

fn named(game: &GameState, name: &str) -> ObjectId {
    game.objects()
        .find(|object| object.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn names(game: &GameState, ids: &[ObjectId]) -> Vec<String> {
    let mut found: Vec<String> = ids
        .iter()
        .map(|id| game.object(*id).unwrap().card().name.clone())
        .collect();
    found.sort();
    found
}

fn payer<'a>(game: &'a GameState, barred: &'a [ObjectId]) -> Payer<'a> {
    Payer {
        state: game,
        player: ME,
        source: named(game, "Source"),
        barred,
    }
}

fn part(json: &str) -> Part {
    Part::read(&item(json)).unwrap().unwrap()
}

#[rstest]
#[case::life(r#"{"PayLife":2}"#, Part::Life(2))]
#[case::counters(
    r#"{"RemoveCounters":{"counter":"charge","count":2}}"#,
    Part::Counters { counter: CounterKind::Charge, count: 2 }
)]
#[case::one_sacrifice(
    r#"{"Sacrifice":{"filter":[{"Type":"Creature"}]}}"#,
    Part::Pick(Pick { kind: Kind::Sacrifice, filter: Selector(vec![mtg_cards::refs::Pred::Type(CardType::Creature)]), count: 1 })
)]
#[case::two_discards(
    r#"{"Discard":{"count":2}}"#,
    Part::Pick(Pick { kind: Kind::Discard, filter: Selector::default(), count: 2 })
)]
fn a_part_is_read_with_its_fixed_amount(#[case] json: &str, #[case] expected: Part) {
    assert_eq!(Part::read(&item(json)), Ok(Some(expected)));
}

#[rstest]
#[case::mana(r#"{"Mana":"{1}"}"#)]
#[case::tap(r#""Tap""#)]
#[case::sacrificing_the_source(r#""SacrificeSelf""#)]
fn what_is_not_a_part_is_not_read_as_one(#[case] json: &str) {
    assert_eq!(Part::read(&item(json)), Ok(None));
}

#[rstest]
#[case::life(r#"{"PayLife":{"Life":"You"}}"#, "Cost.PayLife.Counted")]
#[case::energy(r#"{"PayEnergy":{"Life":"You"}}"#, "Cost.PayEnergy.Counted")]
#[case::counters(
    r#"{"RemoveCounters":{"counter":"charge","count":{"Life":"You"}}}"#,
    "Cost.RemoveCounters.Counted"
)]
#[case::sacrifice(
    r#"{"Sacrifice":{"filter":[],"count":{"Life":"You"}}}"#,
    "Cost.Sacrifice.Counted"
)]
#[case::negative(r#"{"PayLife":-1}"#, "Cost.PayLife.Counted")]
fn an_amount_that_is_not_a_fixed_number_is_named(#[case] json: &str, #[case] name: &str) {
    assert_eq!(Part::read(&item(json)), Err(name));
}

#[rstest]
#[case::creatures_you_control(r#"{"Sacrifice":{"filter":[{"Type":"Creature"}]}}"#, &[], &["Ogre", "Source", "Wolf"])]
#[case::not_the_source(r#"{"Sacrifice":{"filter":[{"Type":"Creature"}]}}"#, &["Source"], &["Ogre", "Wolf"])]
#[case::an_artifact(r#"{"Sacrifice":{"filter":[{"Type":"Artifact"}]}}"#, &[], &["Relic"])]
#[case::cards_of_the_hand(r#"{"Discard":{}}"#, &[], &["Held Bear", "Held Relic"])]
#[case::a_creature_card_of_the_hand(
    r#"{"Discard":{"filter":[{"Type":"Creature"}]}}"#,
    &[],
    &["Held Bear"]
)]
#[case::the_graveyard(r#"{"ExileFromGraveyard":{"filter":[]}}"#, &[], &["Dead Bear", "Dead Elk"])]
#[case::only_untapped_permanents(
    r#"{"TapPermanents":{"filter":[{"Type":"Creature"}]}}"#,
    &[],
    &["Source", "Wolf"]
)]
#[case::untapped_and_not_barred(
    r#"{"TapPermanents":{"filter":[{"Type":"Creature"}]}}"#,
    &["Source"],
    &["Wolf"]
)]
#[case::untapped_creatures_for_their_power(
    r#"{"TapPower":{"filter":[{"Type":"Creature"}],"power":2}}"#,
    &[],
    &["Source", "Wolf"]
)]
#[case::a_land_to_return(r#"{"ReturnToHand":{"filter":[{"Type":"Artifact"}]}}"#, &[], &["Relic"])]
fn a_pick_offers_what_the_player_may_take(
    #[case] json: &str,
    #[case] barred: &[&str],
    #[case] expected: &[&str],
) {
    let game = game();
    let barred: Vec<ObjectId> = barred.iter().map(|name| named(&game, name)).collect();
    let Part::Pick(pick) = part(json) else {
        panic!("a pick was expected");
    };
    assert_eq!(
        names(&game, &pick.candidates(&payer(&game, &barred))),
        expected
    );
}

#[rstest]
#[case::life_it_has(r#"{"PayLife":20}"#, true)]
#[case::more_life_than_it_has(r#"{"PayLife":21}"#, false)]
#[case::counters_it_has(r#"{"RemoveCounters":{"counter":"charge","count":3}}"#, true)]
#[case::more_counters_than_it_has(r#"{"RemoveCounters":{"counter":"charge","count":4}}"#, false)]
#[case::counters_of_another_kind(r#"{"RemoveCounters":{"counter":"poison"}}"#, false)]
#[case::two_creatures_to_sacrifice(
    r#"{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":3}}"#,
    true
)]
#[case::four_creatures_to_sacrifice(
    r#"{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":4}}"#,
    false
)]
#[case::power_the_untapped_creatures_have(
    r#"{"TapPower":{"filter":[{"Type":"Creature"}],"power":3}}"#,
    true
)]
#[case::more_power_than_the_untapped_creatures_have(
    r#"{"TapPower":{"filter":[{"Type":"Creature"}],"power":4}}"#,
    false
)]
#[case::three_cards_to_discard(r#"{"Discard":{"count":3}}"#, false)]
#[case::two_cards_to_discard(r#"{"Discard":{"count":2}}"#, true)]
fn a_part_is_payable_with_what_it_needs(#[case] json: &str, #[case] payable: bool) {
    let game = game();
    assert_eq!(
        part(json).payable(&payer(&game, &[]), &[]).is_some(),
        payable
    );
}

#[test]
fn a_part_leaves_what_the_mana_may_need_when_it_can() {
    let game = game();
    let (wolf, source, ogre) = (
        named(&game, "Wolf"),
        named(&game, "Source"),
        named(&game, "Ogre"),
    );
    let one = part(r#"{"Sacrifice":{"filter":[{"Type":"Creature"}]}}"#);
    let taken = one.payable(&payer(&game, &[]), &[wolf, source]).unwrap();
    assert_eq!(taken, [ogre]);
    let all = part(r#"{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":3}}"#);
    let taken = all.payable(&payer(&game, &[]), &[wolf]).unwrap();
    assert_eq!(
        taken.len(),
        3,
        "what the mana needs is given when nothing else is left"
    );
    assert_eq!(taken.last(), Some(&wolf));
}

#[test]
fn the_parts_of_a_cost_are_paid_with_the_events_the_engine_logs() {
    let game = game();
    let (source, wolf, relic) = (
        named(&game, "Source"),
        named(&game, "Wolf"),
        named(&game, "Relic"),
    );
    let (held, dead) = (named(&game, "Held Bear"), named(&game, "Dead Bear"));
    let events = |json: &str, picked: &[ObjectId]| part(json).events(&game, ME, source, picked);
    assert_eq!(
        events(r#"{"PayLife":2}"#, &[]),
        [Event::LifePaid {
            player: ME,
            amount: 2
        }]
    );
    assert_eq!(events(r#"{"PayLife":0}"#, &[]), []);
    assert_eq!(
        events(r#"{"RemoveCounters":{"counter":"charge","count":2}}"#, &[]),
        [Event::CountersChanged(CountersChanged {
            on: crate::choose::Target::Object(game.object(source).unwrap().reference()),
            counter: CounterKind::Charge,
            delta: -2,
        })]
    );
    assert_eq!(
        events(r#"{"Sacrifice":{"filter":[]}}"#, &[relic]),
        [Event::ZoneChanged {
            object: relic,
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Sacrificed,
        }]
    );
    assert_eq!(
        events(r#"{"Discard":{}}"#, &[held]),
        [Event::CardDiscarded {
            player: ME,
            object: held
        }]
    );
    assert_eq!(
        events(r#"{"ExileFromGraveyard":{"filter":[]}}"#, &[dead]),
        [Event::ZoneChanged {
            object: dead,
            from: Zone::Graveyard(ME),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        }]
    );
    assert_eq!(
        events(r#"{"TapPermanents":{"filter":[]}}"#, &[wolf]),
        [Event::PermanentTapped { object: wolf }]
    );
    assert_eq!(
        events(r#"{"ReturnToHand":{"filter":[]}}"#, &[relic]),
        [Event::ZoneChanged {
            object: relic,
            from: Zone::Battlefield,
            to: Zone::Hand(ME),
            cause: ZoneCause::Moved,
        }]
    );
}

#[rstest]
#[case::sacrifice(
    r#"{"Sacrifice":{"filter":[{"Type":"Creature"}]}}"#,
    "Sacrifice a creature."
)]
#[case::sacrifice_two(
    r#"{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":2}}"#,
    "Sacrifice 2 creatures."
)]
#[case::discard(r#"{"Discard":{}}"#, "Discard a card.")]
#[case::exile(
    r#"{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}]}}"#,
    "Exile a creature card from your graveyard."
)]
#[case::exile_any(
    r#"{"ExileFromGraveyard":{"filter":[]}}"#,
    "Exile a card from your graveyard."
)]
#[case::tap(
    r#"{"TapPermanents":{"filter":[{"Type":"Creature"}]}}"#,
    "Tap an untapped creature you control."
)]
#[case::tap_two(
    r#"{"TapPermanents":{"filter":[{"Type":"Creature"}],"count":2}}"#,
    "Tap 2 untapped creatures you control."
)]
#[case::tap_for_power(
    r#"{"TapPower":{"filter":[{"Type":"Creature"}],"power":3}}"#,
    "Tap any number of untapped creatures you control with total power 3 or more."
)]
#[case::return_one(
    r#"{"ReturnToHand":{"filter":[{"Type":"Land"}]}}"#,
    "Return a land you control to its owner's hand."
)]
fn a_pick_is_asked_in_the_words_of_a_card(#[case] json: &str, #[case] words: &str) {
    let Part::Pick(pick) = part(json) else {
        panic!("a pick was expected");
    };
    assert_eq!(pick.text(), words);
}

#[test]
fn a_cost_of_parts_has_the_names_a_refusal_gives() {
    let cost: Cost = serde_json::from_str(
        r#"[{"PayLife":1},{"RemoveCounters":{"counter":"charge"}},{"Sacrifice":{"filter":[]}},{"Discard":{}},{"ExileFromGraveyard":{"filter":[]}},{"TapPermanents":{"filter":[]}},{"TapPower":{"filter":[],"power":3}},{"ReturnToHand":{"filter":[]}}]"#,
    )
    .unwrap();
    let named: Vec<&str> = cost
        .0
        .iter()
        .map(|item| Part::read(item).unwrap().unwrap().name())
        .collect();
    assert_eq!(
        named,
        [
            "Cost.PayLife",
            "Cost.RemoveCounters",
            "Cost.Sacrifice",
            "Cost.Discard",
            "Cost.ExileFromGraveyard",
            "Cost.TapPermanents",
            "Cost.TapPower",
            "Cost.ReturnToHand"
        ]
    );
}
