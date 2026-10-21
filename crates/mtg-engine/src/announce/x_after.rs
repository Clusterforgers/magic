//! Goal: the X a spell was cast with is remembered by the spell on the stack and by the permanent it becomes, and by nothing else: a card that comes back from the graveyard or from exile, and a permanent a trigger or replacement reads after it has left, have X = 0; the replacement "enters with X counters" and an enters trigger read it (CR 107.3m), the mana value of a spell on the stack counts X for each `{X}` of its cost (CR 202.3e) and the mana value of the same card anywhere else does not.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use crate::choose::Choices;
use crate::events::{Event, ZoneCause};
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::resolve::Scope;
use crate::snapshot::snapshot;
use crate::state::GameState;
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);

/// A creature that costs `{X}{X}{1}` and enters with X +1/+1 counters.
fn walker() -> Arc<CardDefinition> {
    let enters = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":"X"}}}}"#;
    Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 0)
            .with_mana_cost(ManaCost::parse("{1}").unwrap())
            .with_x_in_cost(2)
            .with_rules(serde_json::from_str(enters).unwrap()),
    )
}

fn table() -> GameState {
    state(vec![vec![walker()], vec![]], vec![vec![], vec![]])
}

fn cast(game: &mut GameState, x: Option<u32>) -> crate::object::ObjectRef {
    let card = game.zone(Zone::Hand(SEAT))[0];
    let choices = Choices {
        x,
        ..Choices::default()
    };
    let event = Event::SpellCast {
        player: SEAT,
        object: card,
        choices,
    };
    apply_event(game, &event).unwrap();
    game.object(card).unwrap().reference()
}

fn moved(game: &mut GameState, from: Zone, to: Zone) {
    let card = game.zone(from)[0];
    let event = Event::ZoneChanged {
        object: card,
        from,
        to,
        cause: ZoneCause::Destroyed,
    };
    apply_event(game, &event).unwrap();
}

fn value(game: &GameState, object: crate::object::ObjectRef) -> i64 {
    Scope::announcing(game, SEAT, object)
        .amount(&Amount::X)
        .unwrap()
}

#[rstest]
#[case::nothing(Some(0))]
#[case::one(Some(1))]
#[case::large(Some(250))]
fn the_spell_remembers_x_on_the_stack_and_the_permanent_after_it(#[case] x: Option<u32>) {
    let mut game = table();
    let spell = cast(&mut game, x);
    let card = spell.id;
    assert_eq!(game.object(card).unwrap().announced_x(), x);
    assert_eq!(snapshot(&game, card).unwrap().x, x);
    assert_eq!(value(&game, spell), i64::from(x.unwrap()));

    apply_event(&mut game, &Event::SpellResolved { object: card }).unwrap();
    let permanent = game.object(card).unwrap();
    assert_eq!(permanent.zone(), Zone::Battlefield);
    assert_eq!(permanent.announced_x(), x);
    assert_eq!(snapshot(&game, card).unwrap().x, x);
    assert!(permanent.choices().is_empty(), "the stack is left behind");
    // The permanent itself has X = 0 (CR 107.3m): only what enters reads the X it was cast with.
    assert_eq!(value(&game, permanent.reference()), 0);

    moved(&mut game, Zone::Battlefield, Zone::Graveyard(SEAT));
    let gone = game.object(card).unwrap();
    assert_eq!(gone.announced_x(), None);
    assert_eq!(value(&game, gone.reference()), 0);
}

#[rstest]
#[case::from_the_graveyard(Zone::Graveyard(SEAT))]
#[case::from_exile(Zone::Exile)]
fn a_card_put_onto_the_battlefield_by_another_effect_has_x_0(#[case] from: Zone) {
    let mut game = table();
    cast(&mut game, Some(4));
    let card = game.zone(Zone::Stack)[0];
    apply_event(&mut game, &Event::SpellResolved { object: card }).unwrap();
    moved(&mut game, Zone::Battlefield, from);
    moved(&mut game, from, Zone::Battlefield);
    let again = game.object(card).unwrap();
    assert_eq!(again.zone(), Zone::Battlefield);
    assert_eq!(again.announced_x(), None, "a new object remembers nothing");
    assert_eq!(snapshot(&game, card).unwrap().x, None);
}

#[test]
fn the_replacement_of_the_card_entering_reads_the_x_of_the_spell_still_on_the_stack() {
    let mut game = table();
    let spell = cast(&mut game, Some(3));
    assert_eq!(value(&game, spell), 3);
}

#[rstest]
#[case::on_the_stack(true, 7, 15)]
#[case::nothing_chosen(true, 0, 1)]
#[case::in_the_hand(false, 7, 1)]
fn the_mana_value_of_a_spell_counts_the_x_chosen_for_each_symbol(
    #[case] on_the_stack: bool,
    #[case] x: u32,
    #[case] expected: i64,
) {
    // {X}{X}{1} with X = 7 is 15, with X = 0 is 1; the mana value of the card in a hand has X = 0.
    let mut game = table();
    let object = if on_the_stack {
        cast(&mut game, Some(x))
    } else {
        let card = game.zone(Zone::Hand(SEAT))[0];
        game.object(card).unwrap().reference()
    };
    let selector = serde_json::from_str(r#"{"Stat":{"of":"This","stat":"ManaValue"}}"#).unwrap();
    let scope = Scope::announcing(&game, SEAT, object);
    assert_eq!(scope.amount(&selector).unwrap(), expected);
}
