//! Goal: the general amounts and conditions (printed power, distinct colors, commander colors, the turn number, the
//! starting player, the targets of the triggering spell and the logged tally) read the game as the rules say.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::{Amount, Condition};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::{Scope, Target};
use crate::abilities::Cause;
use crate::choose::Choices;
use crate::events::Event;
use crate::fixtures::{creature, setup};
use crate::fold::apply_event;
use crate::state::GameState;
use crate::tokens::TokenCreated;

fn colored(name: &str, cost: &str, power: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse(cost).unwrap())
            .with_power_toughness(power, power),
    )
}

fn game(commanders: Vec<Vec<Arc<CardDefinition>>>) -> GameState {
    let battlefields = vec![
        vec![colored("Green", "{1}{G}", 2), colored("Gold", "{R}{G}", 4)],
        vec![colored("Blue", "{U}", 1)],
    ];
    let mut setup = setup(vec![vec![], vec![]], battlefields);
    if !commanders.is_empty() {
        setup.format = Arc::new(crate::format::Commander);
        setup = setup.with_commanders(commanders);
    }
    GameState::from_setup(&setup).unwrap()
}

fn scope(game: &GameState) -> Scope<'_> {
    let source = game.objects.values().next().unwrap().reference();
    Scope::announcing(game, PlayerId(0), source)
}

fn amount(game: &GameState, json: &str) -> i64 {
    scope(game)
        .amount(&serde_json::from_str::<Amount>(json).unwrap())
        .unwrap()
}

#[rstest]
#[case::printed_power_of_the_biggest(
    r#"{"Greatest":{"of":[{"Type":"Creature"}],"stat":"BasePower"}}"#,
    4
)]
#[case::printed_power_of_yours(
    r#"{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"BasePower"}}"#,
    6
)]
#[case::printed_power_of_no_creature(
    r#"{"Total":{"of":[{"Type":"Artifact"}],"stat":"BasePower"}}"#,
    0
)]
#[case::printed_toughness_of_yours(
    r#"{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"BaseToughness"}}"#,
    6
)]
#[case::colors_among_all(r#"{"Distinct":{"of":[{"Type":"Creature"}],"stat":"Colors"}}"#, 3)]
#[case::colors_among_yours(
    r#"{"Distinct":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Colors"}}"#,
    2
)]
#[case::colors_among_theirs(
    r#"{"Distinct":{"of":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"stat":"Colors"}}"#,
    1
)]
#[case::colors_among_nothing(r#"{"Distinct":{"of":[{"Type":"Artifact"}],"stat":"Colors"}}"#, 0)]
#[case::the_first_turn(r#""TurnNumber""#, 1)]
#[case::no_commander(r#"{"CommanderColors":"You"}"#, 0)]
fn a_general_amount_reads_the_state(#[case] json: &str, #[case] expected: i64) {
    assert_eq!(amount(&game(vec![]), json), expected);
}

#[rstest]
#[case::two_colors(vec![vec![colored("Boss", "{W}{U}", 1)], vec![]], r#"{"CommanderColors":"You"}"#, 2)]
#[case::the_other_seat_has_none(vec![vec![colored("Boss", "{W}{U}", 1)], vec![]], r#"{"CommanderColors":"Opponents"}"#, 0)]
#[case::both_seats_share_colors(vec![vec![colored("A", "{W}{U}", 1)], vec![colored("B", "{U}{B}", 1)]], r#"{"CommanderColors":"EachPlayer"}"#, 3)]
#[case::partners(vec![vec![colored("A", "{W}", 1), colored("B", "{R}", 1)], vec![]], r#"{"CommanderColors":"You"}"#, 2)]
#[case::colorless_commander(vec![vec![creature("Plain", 1, 1)], vec![]], r#"{"CommanderColors":"You"}"#, 0)]
fn commander_colors_count_the_identity(
    #[case] commanders: Vec<Vec<Arc<CardDefinition>>>,
    #[case] json: &str,
    #[case] expected: i64,
) {
    assert_eq!(amount(&game(commanders), json), expected);
}

#[rstest]
#[case::you(r#""You""#, true)]
#[case::opponents(r#""Opponents""#, false)]
#[case::everyone(r#""EachPlayer""#, false)]
fn the_starting_player_is_the_first_seat(#[case] who: &str, #[case] expected: bool) {
    let game = game(vec![]);
    let text = format!(r#"{{"StartingPlayer":{who}}}"#);
    let condition: Condition = serde_json::from_str(&text).unwrap();
    assert_eq!(scope(&game).holds(&condition), Ok(expected));
}

#[rstest]
#[case::none(vec![], 0)]
#[case::one(vec![Target::Player(PlayerId(1))], 1)]
#[case::three(vec![Target::Player(PlayerId(1)), Target::Player(PlayerId(0)), Target::Player(PlayerId(1))], 3)]
fn the_triggering_spell_has_its_targets(#[case] targets: Vec<Target>, #[case] expected: i64) {
    let mut game = game(vec![]);
    let spell = game.objects.values().next().unwrap().reference();
    game.objects.get_mut(&spell.id).unwrap().choices = Choices {
        targets: vec![targets],
        ..Choices::default()
    };
    let cause = Cause {
        subjects: vec![Target::Object(spell)],
        snapshots: Vec::new(),
        amount: 0,
        source: None,
        against: None,
    };
    let scope = Scope::triggered(&game, PlayerId(0), spell, &cause);
    assert_eq!(scope.amount(&Amount::TriggeringTargets), Ok(expected));
    // Outside a trigger there is no spell.
    let outside = Scope::announcing(&game, PlayerId(0), spell);
    assert!(outside.amount(&Amount::TriggeringTargets).is_err());
}

fn make_token(game: &mut GameState, id: u64, controller: PlayerId, creature: bool) {
    let definition = if creature {
        CardDefinition::new("Spirit")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
    } else {
        CardDefinition::new("Treasure").with_type(CardType::Artifact)
    };
    let event = Event::TokenCreated(TokenCreated {
        object: ObjectId(id),
        copy_of: None,
        controller,
        name: "Token".into(),
        tapped: false,
        attacking: None,
        definition: Arc::new(definition),
    });
    apply_event(game, &event).unwrap();
}

#[rstest]
#[case::every_token(r#"{"Tally":{"event":"TokenCreated","who":"You","filter":[]}}"#, 3)]
#[case::creature_tokens(
    r#"{"Tally":{"event":"TokenCreated","who":"You","filter":[{"Type":"Creature"}]}}"#,
    2
)]
#[case::artifact_tokens(
    r#"{"Tally":{"event":"TokenCreated","who":"You","filter":[{"Type":"Artifact"}]}}"#,
    1
)]
#[case::the_opponents_tokens(
    r#"{"Tally":{"event":"TokenCreated","who":"Opponents","filter":[]}}"#,
    1
)]
#[case::everyones(
    r#"{"Tally":{"event":"TokenCreated","who":"EachPlayer","filter":[]}}"#,
    4
)]
#[case::no_spell_was_cast(r#"{"Tally":{"event":"SpellCast","who":"You","filter":[]}}"#, 0)]
fn the_tally_counts_the_logged_events_the_filter_selects(
    #[case] json: &str,
    #[case] expected: i64,
) {
    let mut game = game(vec![]);
    make_token(&mut game, 900, PlayerId(0), true);
    make_token(&mut game, 901, PlayerId(0), true);
    make_token(&mut game, 902, PlayerId(0), false);
    make_token(&mut game, 903, PlayerId(1), true);
    assert_eq!(amount(&game, json), expected);
}

#[test]
fn a_token_that_ceased_is_counted_as_it_was_and_the_next_turn_starts_over() {
    let mut game = game(vec![]);
    make_token(&mut game, 900, PlayerId(0), true);
    crate::tokens::cease(&mut game, ObjectId(900));
    let json = r#"{"Tally":{"event":"TokenCreated","who":"You","filter":[{"Type":"Creature"}]}}"#;
    assert_eq!(amount(&game, json), 1);
    let next = Event::TurnBegan {
        number: 2,
        active: PlayerId(1),
        extra: false,
    };
    apply_event(&mut game, &next).unwrap();
    assert_eq!(amount(&game, json), 0);
    assert_eq!(amount(&game, r#""TurnNumber""#), 2);
}
