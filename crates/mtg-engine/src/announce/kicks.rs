//! Goal: kicker and multikicker are read from the keywords of a spell and asked at announcement (a cost once in `ChooseCosts`, a repeatable one by `ChooseNumber`); a cost is offered only up to what the player can pay for with the rest of the total cost, a cost with a part other than mana is never offered, the total cost holds the printed cost (hybrid symbols too) and the mana of every payment, and what was paid stays with the spell, the permanent it becomes and what its abilities read, until the object leaves.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::Selector;
use mtg_cards::value::{Amount, Condition};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::{Answered, Next, Pending, What, announce, optional};
use crate::choose::Choices;
use crate::decision::{Decision, DecisionKind, PayOptions};
use crate::events::Event;
use crate::fixtures::state;
use crate::fold::apply_event;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

const SEAT: PlayerId = PlayerId(0);

fn mana(text: &str) -> Cost {
    Cost::mana(text.parse().unwrap())
}

/// An instant that costs `cost` and has `keywords`.
fn instant(cost: &str, keywords: Vec<Keyword>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Spell")
        .with_type(CardType::Instant)
        .with_mana_cost(ManaCost::parse(cost).unwrap())
        .with_rules(serde_json::from_str(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap());
    for keyword in keywords {
        card = card.with_rules(Ability::Keyword(keyword));
    }
    Arc::new(card)
}

/// A creature that costs `{1}` and has `keywords`.
fn creature(keywords: Vec<Keyword>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Kicked Bear")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2)
        .with_mana_cost(ManaCost::parse("{1}").unwrap());
    for keyword in keywords {
        card = card.with_rules(Ability::Keyword(keyword));
    }
    Arc::new(card)
}

/// Seat 0 holds `card` and controls `lands` Forests.
fn table(card: Arc<CardDefinition>, lands: usize) -> GameState {
    let forests = (0..lands)
        .map(|_| {
            Arc::new(CardDefinition::basic_land(Some(
                mtg_core::color::Color::Green,
            )))
        })
        .collect();
    state(vec![vec![card], vec![]], vec![forests, vec![]])
}

fn pending(game: &GameState) -> Pending {
    Pending::new(SEAT, What::Spell(game.zone(Zone::Hand(SEAT))[0]))
}

/// What the announcement asked, in order, and what it came to.
struct Walk {
    asked: Vec<&'static str>,
    /// The `(min, max)` of the number asked for multikicker.
    number: Option<(u32, Option<u32>)>,
    /// The costs offered in `ChooseCosts`, by index.
    offered: Vec<usize>,
    paying: Option<PayOptions>,
    pending: Pending,
}

/// Answers the costs with `costs` and the number with `times`; the rest of the announcement has nothing to ask.
fn walk(game: &GameState, costs: &[usize], times: u32) -> Walk {
    let mut pending = pending(game);
    let mut walk = Walk {
        asked: Vec::new(),
        number: None,
        offered: Vec::new(),
        paying: None,
        pending: pending.clone(),
    };
    for _ in 0..8 {
        match announce(game, &mut pending) {
            Next::Ask(Decision {
                kind: DecisionKind::PayMana(options),
                ..
            }) => {
                walk.asked.push("pay-mana");
                walk.paying = Some(options);
                break;
            }
            Next::Ask(Decision { kind, .. }) => {
                walk.asked.push(kind.name());
                let answer = match kind {
                    DecisionKind::ChooseCosts(options) => {
                        walk.offered = options.optional.iter().map(|cost| cost.index).collect();
                        Answered::Costs(costs.to_vec())
                    }
                    DecisionKind::ChooseNumber(options) => {
                        walk.number = Some((options.min, options.max));
                        Answered::Number(times)
                    }
                    other => panic!("{} was not expected", other.name()),
                };
                pending.accept(answer).unwrap();
            }
            Next::Commit(_) | Next::Drop => break,
        }
    }
    walk.pending = pending;
    walk
}

const KICKER: fn() -> Keyword = || Keyword::Kicker(mana("{1}"));
const MULTIKICKER: fn() -> Keyword = || Keyword::Multikicker(mana("{1}"));

#[rstest]
#[case::the_kicker_cannot_be_paid(1, &[])]
#[case::the_kicker_can_be_paid(2, &[0])]
#[case::with_mana_to_spare(5, &[0])]
fn a_kicker_is_offered_only_when_the_player_can_pay_for_it(
    #[case] lands: usize,
    #[case] offered: &[usize],
) {
    let walk = walk(&table(instant("{1}", vec![KICKER()]), lands), &[], 0);
    assert_eq!(walk.offered, offered);
    let asked: &[&str] = if offered.is_empty() {
        &["pay-mana"]
    } else {
        &["choose-costs", "pay-mana"]
    };
    assert_eq!(walk.asked, asked);
}

#[rstest]
#[case::not_kicked(&[], 1)]
#[case::kicked(&[0], 2)]
fn the_total_cost_holds_the_kicker_only_when_it_is_paid(
    #[case] costs: &[usize],
    #[case] total: u32,
) {
    let walk = walk(&table(instant("{1}", vec![KICKER()]), 3), costs, 0);
    assert_eq!(walk.paying.unwrap().cost.mana_value(), total);
    assert_eq!(walk.pending.choices.optional_costs, costs);
}

#[rstest]
#[case::no_mana_for_a_kick(1, 0, None, 1, 0)]
#[case::one_kick_is_all_that_can_be_paid(2, 0, Some(1), 1, 0)]
#[case::one_kick_of_one(2, 1, Some(1), 2, 1)]
#[case::none_of_many(5, 0, Some(4), 1, 0)]
#[case::one_of_many(5, 1, Some(4), 2, 1)]
#[case::some_of_many(5, 3, Some(4), 4, 3)]
#[case::all_of_many(5, 4, Some(4), 5, 4)]
fn a_multikicker_is_asked_up_to_the_times_the_player_can_pay_and_adds_its_cost_for_each(
    #[case] lands: usize,
    #[case] times: u32,
    #[case] most: Option<u32>,
    #[case] total: u32,
    #[case] kicks: usize,
) {
    let walk = walk(
        &table(instant("{1}", vec![MULTIKICKER()]), lands),
        &[],
        times,
    );
    assert_eq!(walk.number, most.map(|most| (0, Some(most))));
    assert_eq!(walk.asked.contains(&"choose-number"), most.is_some());
    assert_eq!(walk.paying.unwrap().cost.mana_value(), total);
    assert_eq!(walk.pending.choices.optional_costs, vec![0; kicks]);
    assert!(
        walk.pending.done.optional,
        "the multikicker is settled once it is answered"
    );
}

#[test]
fn a_kicker_and_a_multikicker_are_asked_one_after_the_other_and_the_number_leaves_what_the_kick_left()
 {
    let card = instant("{1}", vec![KICKER(), MULTIKICKER()]);
    let walk = walk(&table(card, 5), &[0], 2);
    assert_eq!(walk.asked, ["choose-costs", "choose-number", "pay-mana"]);
    assert_eq!(walk.offered, [0]);
    // Five lands: one for the cost and one for the kicker leave three.
    assert_eq!(walk.number, Some((0, Some(3))));
    assert_eq!(walk.pending.choices.optional_costs, [0, 1, 1]);
    assert_eq!(walk.paying.unwrap().cost.mana_value(), 4);
}

#[rstest]
#[case::paid_with_the_rest(2, true)]
#[case::one_land_short(1, false)]
fn a_hybrid_symbol_stays_in_the_total_beside_the_kicker(
    #[case] lands: usize,
    #[case] kicked: bool,
) {
    let card = instant("{G/W}", vec![Keyword::Kicker(mana("{G}"))]);
    let walk = walk(&table(card, lands), &[0], 0);
    assert_eq!(walk.offered.is_empty(), !kicked);
    let cost = walk.paying.unwrap().cost;
    assert_eq!(cost.symbols().len(), if kicked { 2 } else { 1 });
    assert_eq!(cost.mana_value(), if kicked { 2 } else { 1 });
    assert_eq!(
        cost.symbols().iter().filter(|s| s.is_plain()).count(),
        usize::from(kicked)
    );
}

#[test]
fn a_cost_with_a_part_other_than_mana_is_not_offered() {
    let sacrifice = Cost(vec![CostItem::Sacrifice {
        filter: Selector::default(),
        count: Amount::one(),
    }]);
    let card = instant("{1}", vec![Keyword::Kicker(sacrifice), MULTIKICKER()]);
    let game = table(card, 4);
    let read = optional::read(&game, &What::Spell(game.zone(Zone::Hand(SEAT))[0]));
    let labels: Vec<&str> = read.iter().map(|cost| cost.label.as_str()).collect();
    assert_eq!(labels, ["Multikicker {1}"]);
}

#[rstest]
#[case::kicker(Keyword::Kicker(mana("{4}")), "Kicker {4}", false)]
#[case::multikicker(Keyword::Multikicker(mana("{1}{G}")), "Multikicker {1}{G}", true)]
fn the_keywords_of_a_spell_give_its_optional_costs_as_the_card_prints_them(
    #[case] keyword: Keyword,
    #[case] label: &str,
    #[case] repeatable: bool,
) {
    let game = table(instant("{R}", vec![keyword]), 0);
    let read = optional::read(&game, &What::Spell(game.zone(Zone::Hand(SEAT))[0]));
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].label, label);
    assert_eq!(read[0].repeatable, repeatable);
}

#[test]
fn a_spell_without_the_keywords_has_no_optional_costs() {
    let game = table(instant("{1}", Vec::new()), 2);
    let read = optional::read(&game, &What::Spell(game.zone(Zone::Hand(SEAT))[0]));
    assert_eq!(read.len(), 0);
}

fn read(game: &GameState, object: crate::object::ObjectRef) -> (bool, i64) {
    let scope = Scope::announcing(game, SEAT, object);
    (
        scope.holds(&Condition::Kicked).unwrap(),
        scope.amount(&Amount::Kicks).unwrap(),
    )
}

#[rstest]
#[case::not_kicked(&[], false, 0)]
#[case::kicked_once(&[0], true, 1)]
#[case::kicked_three_times(&[0, 0, 0], true, 3)]
fn the_spell_remembers_what_it_paid_while_it_is_on_the_stack_and_the_permanent_after_it(
    #[case] paid: &[usize],
    #[case] kicked: bool,
    #[case] kicks: i64,
) {
    let mut game = table(creature(vec![MULTIKICKER()]), 0);
    let card = game.zone(Zone::Hand(SEAT))[0];
    assert_eq!(
        read(&game, game.object(card).unwrap().reference()),
        (false, 0)
    );

    let cast = Event::SpellCast {
        player: SEAT,
        object: card,
        choices: Choices {
            optional_costs: paid.to_vec(),
            ..Choices::default()
        },
    };
    apply_event(&mut game, &cast).unwrap();
    let spell = game.object(card).unwrap();
    assert_eq!(spell.paid_costs(), paid);
    assert_eq!(read(&game, spell.reference()), (kicked, kicks));

    apply_event(&mut game, &Event::SpellResolved { object: card }).unwrap();
    let permanent = game.object(card).unwrap();
    assert_eq!(permanent.zone(), Zone::Battlefield);
    assert!(permanent.choices().is_empty(), "the stack is left behind");
    assert_eq!(permanent.paid_costs(), paid);
    assert_eq!(read(&game, permanent.reference()), (kicked, kicks));

    // A reference to the spell that was is no longer this object, and the permanent that leaves forgets.
    let left = Event::ZoneChanged {
        object: card,
        from: Zone::Battlefield,
        to: Zone::Graveyard(SEAT),
        cause: crate::events::ZoneCause::Destroyed,
    };
    apply_event(&mut game, &left).unwrap();
    let gone = game.object(card).unwrap();
    assert_eq!(gone.paid_costs().len(), 0);
    assert_eq!(read(&game, gone.reference()), (false, 0));
}

#[test]
fn an_instant_that_resolves_forgets_what_it_paid() {
    let mut game = table(instant("{1}", vec![KICKER()]), 0);
    let card = game.zone(Zone::Hand(SEAT))[0];
    let choices = Choices {
        optional_costs: vec![0],
        ..Choices::default()
    };
    let cast = Event::SpellCast {
        player: SEAT,
        object: card,
        choices,
    };
    apply_event(&mut game, &cast).unwrap();
    apply_event(&mut game, &Event::SpellResolved { object: card }).unwrap();
    assert_eq!(game.object(card).unwrap().zone(), Zone::Graveyard(SEAT));
    assert_eq!(game.object(card).unwrap().paid_costs().len(), 0);
}
