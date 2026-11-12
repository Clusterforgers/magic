//! Goal: "unless they sacrifice a creature or pay 3 life" asks the payer to take one of the costs they can pay or none, at two and four seats: a cost they cannot pay is not offered, the cost taken is paid, and the unpaid effects follow only when none is.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ChoiceOption, OptionChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SPELL: &str = r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Pay":{"who":{"Target":0},"cost":[{"OneOf":[[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],[{"PayLife":3}]]}],"unpaid":[{"Draw":{}}]}}]}}"#;

fn demand() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(SPELL).unwrap();
    Arc::new(
        CardDefinition::new("Demand")
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{R}").unwrap())
            .with_rules(spell),
    )
}

/// What the opponent does when asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Take {
    Creature,
    Life,
    Nothing,
}

#[rstest]
#[case::sacrifices(2, true, Take::Creature)]
#[case::pays_life(2, true, Take::Life)]
#[case::pays_nothing(2, true, Take::Nothing)]
#[case::no_creature_pays_life(2, false, Take::Life)]
#[case::four_seats_sacrifices(4, true, Take::Creature)]
#[case::four_seats_pays_nothing(4, false, Take::Nothing)]
fn the_payer_takes_one_cost_they_can_pay_or_none(
    #[case] seats: usize,
    #[case] has_creature: bool,
    #[case] take: Take,
) -> Result<(), ScriptError> {
    let them = PlayerId(1);
    let mut scenario = Scenario::new(seats)
        .hand(0, [demand()])
        .battlefield(0, [ready(mountain())]);
    if has_creature {
        scenario = scenario.battlefield(1, [ready(card("Grizzly Bears").unwrap())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = driver.state().zone(Zone::Hand(PlayerId(0))).len();
    driver.begin_cast("Demand")?;
    let mut offered = Vec::new();
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![driver.target_player(1)]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseOption(options) if decision.player == them => {
                offered = options.options.clone();
                let wanted = match take {
                    Take::Creature => "Sacrifice",
                    Take::Life => "life",
                    Take::Nothing => "none",
                };
                let at = offered
                    .iter()
                    .position(|o| matches!(o, ChoiceOption::Label(l) if l.contains(wanted)))
                    .unwrap();
                Answer::ChooseOption(OptionChoice(at))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert_eq!(
        offered.len(),
        if has_creature { 3 } else { 2 },
        "{offered:?}"
    );
    let state = driver.state();
    let life = state.player(them).unwrap().life();
    assert_eq!(life, if take == Take::Life { 17 } else { 20 });
    let creatures = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).unwrap().controller() == them)
        .count();
    assert_eq!(
        creatures,
        usize::from(has_creature && take != Take::Creature)
    );
    let drawn = state.zone(Zone::Hand(PlayerId(0))).len() + 1 - hand;
    assert_eq!(drawn, usize::from(take == Take::Nothing));
    Ok(())
}
