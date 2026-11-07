//! Goal: a granted replacement, rule of the game or cost modifier (CR 613.1f) is translated from an animation's `Replacements$`, a static's `AddReplacementEffect$` and a granted `AddStaticAbility$`, works on the permanent that has it at 2 to 4 seats, and ends with the effect or the static that gives it; a grant the engine cannot hold stays a gap.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, attached_to, ready, tapped};
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

const DIES: &str = "SVar:R:Event$ Moved | ActiveZones$ Battlefield | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | ReplaceWith$ X | Description$ If this creature would die, exile it instead.\nSVar:X:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | Defined$ ReplacedCard\n";

const LEAVES: &str = "SVar:R:Event$ Moved | ActiveZones$ Battlefield | Origin$ Battlefield | ValidCard$ Card.Self | ReplaceWith$ X | Description$ If this creature would leave the battlefield, exile it instead.\nSVar:X:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | Defined$ ReplacedCard\n";

/// "Return target creature card from your graveyard to the battlefield. It gains \"<R>\"."
fn reanimation(part: &str) -> String {
    format!(
        "Name:Subject\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn | SubAbility$ A\nSVar:A:DB$ Animate | Replacements$ R | Defined$ ParentTarget | Duration$ Permanent\n{part}"
    )
}

/// An Aura whose static gives the enchanted creature the replacement `R`.
fn aura(part: &str) -> String {
    format!(
        "Name:Subject\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddReplacementEffect$ R | Description$ x\n{part}"
    )
}

fn card(text: &str, ty: CardType) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject")
        .with_type(ty)
        .with_mana_cost(ManaCost::parse("{0}").unwrap());
    if text.contains("Enchantment Aura") {
        card = card.with_subtype("Aura");
    }
    for value in abilities(text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn settle(driver: &mut Driver) {
    for _ in 0..80 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter(|id| state.object(**id).unwrap().card().name == name)
        .count()
}

fn graveyard() -> Zone {
    Zone::Graveyard(PlayerId::from_index(0))
}

/// Bolts the Bear and lets the stack resolve.
fn bolt_the_bear(driver: &mut Driver) {
    let at = driver.target_named("Bear").unwrap();
    driver.cast_at("Bolt", &[at]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).ok();
    settle(driver);
}

/// The grant the first ability of a translated card gives, as JSON.
fn first_grant(text: &str) -> Value {
    let text = Value::Array(abilities(text)).to_string();
    let start = text.find("\"GrantAbility\":").unwrap() + "\"GrantAbility\":".len();
    let grant: Value = serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()
        .unwrap()
        .unwrap();
    grant
}

#[rstest]
#[case::dies_from_an_animation(
    reanimation(DIES),
    json!({"Replacement": {"when": {"ZoneChange": {"what": ["This"], "from": "Battlefield", "to": "Graveyard"}}, "by": {"MoveTo": "Exile"}}})
)]
#[case::leaves_from_an_animation(
    reanimation(LEAVES),
    json!({"Replacement": {"when": {"ZoneChange": {"what": ["This"], "from": "Battlefield"}}, "by": {"MoveTo": "Exile"}}})
)]
#[case::dies_from_a_static(
    aura(DIES),
    json!({"Replacement": {"when": {"ZoneChange": {"what": ["This"], "from": "Battlefield", "to": "Graveyard"}}, "by": {"MoveTo": "Exile"}}})
)]
#[case::cant_lose_from_a_static(
    aura("SVar:R:Event$ GameLoss | ValidPlayer$ You | Layer$ CantHappen | Description$ You can't lose the game.\n"),
    json!({"Rule": {"rule": {"CantLoseGame": {"who": "You"}}}})
)]
#[case::a_rule_from_a_static(
    "Name:Subject\nTypes:Enchantment Aura\nK:Enchant:Creature\nS:Mode$ Continuous | AffectedDefined$ Enchanted | AddStaticAbility$ S | Description$ x\nSVar:S:Mode$ Continuous | Affected$ You | SetMaxHandSize$ Unlimited | Description$ You have no maximum hand size.\n".to_owned(),
    json!({"Rule": {"rule": {"NoMaxHandSize": "You"}}})
)]
#[case::a_rule_from_an_animation(
    "Name:Subject\nTypes:Sorcery\nA:SP$ Animate | ValidTgts$ Creature | staticAbilities$ S | Duration$ Permanent\nSVar:S:Mode$ Continuous | Affected$ You | SetMaxHandSize$ Unlimited | Description$ You have no maximum hand size.\n".to_owned(),
    json!({"Rule": {"rule": {"NoMaxHandSize": "You"}}})
)]
fn a_granted_replacement_or_rule_is_translated(#[case] text: String, #[case] grant: Value) {
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    assert_eq!(first_grant(&text), grant);
}

#[rstest]
#[case::enters_is_over_for_a_permanent(
    aura(
        "SVar:R:Event$ Moved | ValidCard$ Card.Self | Destination$ Battlefield | ReplaceWith$ T | ReplacementResult$ Updated | Description$ x\nSVar:T:DB$ Tap | ETB$ True | Defined$ Self\n"
    ),
    "key:GrantReplacement-enters"
)]
#[case::a_rule_of_another_zone(
    "Name:Subject\nTypes:Sorcery\nA:SP$ Animate | ValidTgts$ Creature | staticAbilities$ S | Duration$ Permanent\nSVar:S:Mode$ Continuous | Affected$ Card.Self | AffectedZone$ Graveyard | EffectZone$ Graveyard | MayPlay$ True | Description$ x\n".to_owned(),
    "key:Animate.staticAbilities-kind"
)]
#[case::a_cost_modifier_of_the_spell_itself(
    "Name:Subject\nTypes:Sorcery\nA:SP$ Animate | ValidTgts$ Creature | staticAbilities$ S | Duration$ Permanent\nSVar:S:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ 1 | Description$ x\n".to_owned(),
    "key:Animate.staticAbilities-kind"
)]
#[case::a_die_line_from_anywhere_else(
    aura(
        "SVar:R:Event$ Moved | ActiveZones$ Graveyard | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | ReplaceWith$ X | Description$ x\nSVar:X:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | Defined$ ReplacedCard\n"
    ),
    "key:Replacement.Moved.ActiveZones"
)]
#[case::a_leave_line_that_does_not_exile(
    reanimation(
        "SVar:R:Event$ Moved | ActiveZones$ Battlefield | Origin$ Battlefield | ValidCard$ Card.Self | ReplaceWith$ X | Description$ x\nSVar:X:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | Defined$ ReplacedCard\n"
    ),
    "replacement:Moved-not-an-exile"
)]
fn a_grant_the_engine_cannot_hold_is_a_gap(#[case] text: String, #[case] gap: &str) {
    let found = gaps(&text);
    assert!(found.iter().any(|g| g == gap), "{found:?}");
}

/// Returns the Bear with the sorcery, then bolts it: a granted "exile instead" sends it to exile.
#[rstest]
#[case::two_dies(2, DIES, true)]
#[case::three_leaves(3, LEAVES, true)]
#[case::four_dies(4, DIES, true)]
#[case::two_without_the_grant(2, "", false)]
fn a_returned_creature_has_the_replacement_it_gains(
    #[case] seats: usize,
    #[case] part: &str,
    #[case] exiled: bool,
) {
    let text = if part.is_empty() {
        "Name:Subject\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn\n".to_owned()
    } else {
        reanimation(part)
    };
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card(&text, CardType::Sorcery), burn("Bolt", 3)])
        .graveyard(0, [free_creature("Bear", 2, 2)])
        .battlefield(0, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let bear = driver.state().zone(graveyard())[0];
    let at = Target::Object(driver.state().object(bear).unwrap().reference());
    driver.cast_at("Subject", &[at]).unwrap();
    settle(&mut driver);
    assert_eq!(count(&driver, Zone::Battlefield, "Bear"), 1);
    bolt_the_bear(&mut driver);
    assert_eq!(count(&driver, Zone::Exile, "Bear"), usize::from(exiled));
    assert_eq!(count(&driver, graveyard(), "Bear"), usize::from(!exiled));
}

/// The Aura's grant applies while it enchants the Bear.
#[rstest]
#[case::two(2)]
#[case::four(4)]
fn an_aura_gives_the_replacement_while_it_is_attached(#[case] seats: usize) {
    let text = aura(DIES);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(
            0,
            [
                ready(free_creature("Bear", 2, 2)),
                attached_to(ready(card(&text, CardType::Enchantment)), 0, 0),
                ready(mountain()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    bolt_the_bear(&mut driver);
    assert_eq!(count(&driver, Zone::Exile, "Bear"), 1);
}

fn dies_exiled() -> Arc<Ability> {
    Arc::new(serde_json::from_value(json!({"Replacement": {"when": {"ZoneChange": {"what": ["This"], "from": "Battlefield", "to": "Graveyard"}}, "by": {"MoveTo": "Exile"}}})).unwrap())
}

/// A replacement granted until end of turn is gone the next turn: the Bear bolted then dies as usual.
#[rstest]
#[case::this_turn(3, false, true)]
#[case::next_turn_ended(3, true, false)]
#[case::next_turn_ended_at_two_seats(2, true, false)]
fn a_granted_replacement_ends_with_its_effect(
    #[case] seats: usize,
    #[case] later: bool,
    #[case] exiled: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(0, [ready(free_creature("Bear", 2, 2)), ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let bear = driver.id_of("Bear").unwrap();
    let on = driver.on(bear);
    driver
        .add_effect(
            on,
            Modification::GrantAbility(dies_exiled()),
            Duration::UntilEndOfTurn,
        )
        .unwrap();
    if later {
        let next = u32::try_from(3 + seats).unwrap();
        driver.idle_until(next, Step::Main1).unwrap();
    }
    bolt_the_bear(&mut driver);
    assert_eq!(count(&driver, Zone::Exile, "Bear"), usize::from(exiled));
}

fn untap_rule() -> Arc<Ability> {
    Arc::new(
        serde_json::from_value(
            json!({"Rule": {"rule": {"UntapDuring": {"what": ["This"], "whose": "Opponents"}}}}),
        )
        .unwrap(),
    )
}

/// A land given "Untap this land during each other player's untap step" untaps in the next player's untap step, unless
/// the grant ended with the turn.
#[rstest]
#[case::two_forever(2, Some(Duration::Forever), true)]
#[case::three_forever(3, Some(Duration::Forever), true)]
#[case::four_for_the_turn(4, Some(Duration::UntilEndOfTurn), false)]
#[case::two_without(2, None, false)]
fn a_granted_rule_applies_and_ends_with_its_effect(
    #[case] seats: usize,
    #[case] duration: Option<Duration>,
    #[case] untapped: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [tapped(forest())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let land = driver.id_of("Forest").unwrap();
    if let Some(duration) = duration {
        let on = driver.on(land);
        driver
            .add_effect(on, Modification::GrantAbility(untap_rule()), duration)
            .unwrap();
    }
    driver.idle_until(4, Step::Upkeep).unwrap();
    let tapped_now = driver
        .state()
        .object(land)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(!tapped_now, untapped);
}

/// A 2/2 for {1}{R}.
fn ogre() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Ogre")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse("{1}{R}").unwrap())
            .with_power_toughness(2, 2),
    )
}

fn cheaper_creatures() -> Arc<Ability> {
    Arc::new(serde_json::from_value(json!({"CostModifier": {"applies_to": ["Spell", {"Type": "Creature"}, {"ControlledBy": "You"}], "change": "Reduce", "mana": "{1}"}})).unwrap())
}

/// A creature given "Creature spells you cast cost {1} less to cast" lets its controller cast a two-mana creature with
/// one land; without the grant, or for another player, it costs its full price.
#[rstest]
#[case::two_granted(2, true, true)]
#[case::three_granted(3, true, true)]
#[case::two_without(2, false, false)]
fn a_granted_cost_modifier_reduces_the_cost(
    #[case] seats: usize,
    #[case] granted: bool,
    #[case] cast: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [ogre()])
        .battlefield(0, [ready(free_creature("Bear", 2, 2)), ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if granted {
        let bear = driver.id_of("Bear").unwrap();
        let on = driver.on(bear);
        driver
            .add_effect(
                on,
                Modification::GrantAbility(cheaper_creatures()),
                Duration::Forever,
            )
            .unwrap();
    }
    let paid = driver.cast_tapping("Ogre", &["Mountain"]).is_ok() && {
        settle(&mut driver);
        count(&driver, Zone::Battlefield, "Ogre") == 1
    };
    assert_eq!(paid, cast);
}
