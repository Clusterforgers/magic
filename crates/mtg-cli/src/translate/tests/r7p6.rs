//! Goal: a damage replacement an `Effect` object makes (`ReplaceWith$` on `Event$ DamageDone`) becomes a shield that
//! scales the damage or sends it to a player or card by the same source, once ("the next time") or all turn; a recipient
//! or source the script does not name as the card prints stays a named gap, and each shield applies only to the damage
//! it names.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, cost, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

const REDIRECT: &str = "SVar:Redirect:DB$ ReplaceEffect | VarName$ Affected | VarValue$ ReplacedSourceController | VarType$ Player\n";
const REDIRECT_ONCE: &str = "SVar:Redirect:DB$ ReplaceEffect | VarName$ Affected | VarValue$ ReplacedSourceController | VarType$ Player | SubAbility$ Gone\nSVar:Gone:DB$ ChangeZone | Defined$ Self | Origin$ Command | Destination$ Exile\n";
const TWICE: &str = "SVar:Redirect:DB$ ReplaceEffect | VarName$ DamageAmount | VarValue$ X\nSVar:X:ReplaceCount$DamageAmount/Twice\n";

/// An instant whose one ability makes a shield from the `Event$ DamageDone` line `event`.
fn instant(event: &str, part: &str) -> Arc<CardDefinition> {
    let text = format!(
        "Name:Shield\nTypes:Instant\nManaCost:R\nA:SP$ Effect | ReplacementEffects$ Event\nSVar:Event:Event$ DamageDone | {event} | ReplaceWith$ Redirect | Description$ x\n{part}"
    );
    let mut card = CardDefinition::new("Shield")
        .with_type(CardType::Instant)
        .with_mana_cost(cost(0, &[mtg_core::color::Color::Red]));
    for value in abilities(&text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn settle(driver: &mut Driver) {
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle && matches!(decision.kind, DecisionKind::Priority(_)) {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

fn cast_shield_and_bolt(driver: &mut Driver, bolt_at: usize, shield_first: bool) {
    let shield = |driver: &mut Driver| {
        driver.begin_cast("Shield").unwrap();
        driver.pay_tapping(&["Mountain"], 0).unwrap();
    };
    if shield_first {
        shield(driver);
        settle(driver);
    }
    let target = driver.target_player(bolt_at);
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    if !shield_first {
        driver.pass().unwrap();
        shield(driver);
    }
    settle(driver);
}

fn lives(driver: &Driver) -> (i32, i32) {
    let life = |seat| {
        driver
            .state()
            .player(PlayerId::from_index(seat))
            .unwrap()
            .life()
    };
    (life(0), life(1))
}

/// Seat 0 casts `Bolt` (3 damage) at `bolt_at`; seat 1 holds the shield, seat 0 holds it when `shield_first`.
fn game(shield: Arc<CardDefinition>, shield_seat: usize, bolt_at: usize) -> (i32, i32) {
    let mut hands = [vec![burn("Bolt", 3)], vec![]];
    hands[shield_seat].push(shield);
    let [first, second] = hands;
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, first)
        .hand(1, second)
        .battlefield(0, [ready(mountain()), ready(mountain())])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_shield_and_bolt(&mut driver, bolt_at, shield_seat == 0);
    lives(&driver)
}

#[rstest]
#[case::redirected_to_the_controller(
    "ValidTarget$ You | DamageTarget$ ReplacedSourceController",
    REDIRECT_ONCE,
    (17, 20)
)]
#[case::all_damage_form(
    "ValidTarget$ You | DamageTarget$ ReplacedSourceController",
    REDIRECT,
    (17, 20)
)]
#[case::source_kind_not_met(
    "ValidTarget$ You | ValidSource$ Creature | DamageTarget$ ReplacedSourceController",
    REDIRECT_ONCE,
    (20, 17)
)]
fn damage_to_you_goes_to_the_controller_of_the_source(
    #[case] event: &str,
    #[case] part: &str,
    #[case] lives_after: (i32, i32),
) {
    assert_eq!(game(instant(event, part), 1, 1), lives_after);
}

#[rstest]
#[case::to_an_opponent_is_doubled("ValidSource$ Card.YouCtrl | ValidTarget$ Opponent", 1, (20, 14))]
#[case::to_the_caster_is_not("ValidSource$ Card.YouCtrl | ValidTarget$ Opponent", 0, (17, 20))]
fn a_doubling_shield_doubles_only_the_damage_it_names(
    #[case] event: &str,
    #[case] bolt_at: usize,
    #[case] lives_after: (i32, i32),
) {
    assert_eq!(game(instant(event, TWICE), 0, bolt_at), lives_after);
}

#[rstest]
#[case::flame_of_keld_drops_you_control(
    "ValidSource$ Card.RedSource | ValidTarget$ Permanent,Player | Description$ If a red source you control would deal damage",
    TWICE,
    "replacement:DamageDone-shield-source-control"
)]
#[case::unknown_recipient(
    "ValidTarget$ Player,Battle",
    TWICE,
    "replacement:DamageDone-shield-recipient"
)]
#[case::unknown_redirect_target(
    "ValidTarget$ You | DamageTarget$ Remembered",
    "SVar:Redirect:DB$ ReplaceEffect | VarName$ Affected | VarValue$ Remembered | VarType$ Card\n",
    "replacement:DamageDone-redirect-target"
)]
fn a_shape_the_rule_cannot_state_stays_a_named_gap(
    #[case] event: &str,
    #[case] part: &str,
    #[case] gap: &str,
) {
    let text = format!(
        "Name:Shield\nTypes:Instant\nA:SP$ Effect | ReplacementEffects$ Event\nSVar:Event:Event$ DamageDone | {event} | ReplaceWith$ Redirect\n{part}"
    );
    assert_eq!(gaps(&text), [gap]);
}

#[test]
fn a_redirect_to_a_card_that_is_gone_deals_the_damage_as_it_was() {
    let part = "SVar:Redirect:DB$ ReplaceEffect | VarName$ Affected | VarValue$ EffectSource | VarType$ Card\n";
    let after = game(
        instant("ValidTarget$ You | DamageTarget$ EffectSource", part),
        1,
        1,
    );
    assert_eq!(after, (20, 17));
}
