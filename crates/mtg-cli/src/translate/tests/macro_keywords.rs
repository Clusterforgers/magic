//! Goal: the keywords of the macro table translate from Forge's `K:` lines with their number and nothing else, every shape the table cannot express stays a gap, and a translated keyword plays through the engine on the object its rules name.
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn script(line: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nPT:2/2\n{line}\n")
}

#[rstest]
#[case::bushido("K:Bushido:2", "bushido", 2)]
#[case::modular("K:Modular:3", "modular", 3)]
#[case::mobilize("K:Mobilize:1", "mobilize", 1)]
#[case::flanking("K:Flanking", "flanking", 0)]
#[case::firebending("K:Firebending:2", "firebending", 2)]
#[case::soulshift("K:Soulshift:4", "soulshift", 4)]
#[case::bloodthirst("K:Bloodthirst:1", "bloodthirst", 1)]
#[case::training("K:Training", "training", 0)]
#[case::ingest("K:Ingest", "ingest", 0)]
#[case::unleash("K:Unleash", "unleash", 0)]
#[case::living_metal("K:Living metal", "living metal", 0)]
#[case::teamwork("K:Teamwork:2", "teamwork", 2)]
#[case::increment("K:Increment", "increment", 0)]
#[case::rampage("K:Rampage:2", "rampage", 2)]
#[case::enlist("K:Enlist", "enlist", 0)]
#[case::provoke("K:Provoke", "provoke", 0)]
fn a_macro_keyword_keeps_its_number(#[case] line: &str, #[case] name: &str, #[case] amount: u32) {
    assert_eq!(
        abilities(&script(line)),
        [json!({"Keyword": {"Macro": {"name": name, "amount": amount}}})]
    );
}

#[rstest]
#[case::a_variable_number("K:Bloodthirst:X")]
#[case::a_variable_mobilize("K:Mobilize:X")]
#[case::a_described_variable("K:Firebending:X:, where X is this creature's power.")]
#[case::a_word_for_a_number("K:Modular:Sunburst")]
#[case::zero("K:Rampage:0")]
#[case::a_missing_number("K:Rampage")]
#[case::a_number_it_does_not_take("K:Flanking:1")]
#[case::a_second_field("K:Bushido:1:Extra")]
#[case::double_team("K:Double team")]
fn a_shape_the_table_cannot_express_is_a_gap(#[case] line: &str) {
    let found = gaps(&script(line));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(abilities(&script(line)).is_empty());
}

/// The first ability of a translated script as a card.
fn card_of(line: &str, name: &str, power: i32, toughness: i32) -> std::sync::Arc<CardDefinition> {
    let text = format!("Name:{name}\nTypes:Creature Elf\nPT:{power}/{toughness}\n{line}\n");
    let ability: Ability = serde_json::from_value(abilities(&text).remove(0)).unwrap();
    std::sync::Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, toughness)
            .with_rules(ability),
    )
}

fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let id = driver.id_of(name).unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// Seat 0's attacker is blocked by the named walls of seat 1; the game goes on to the end of combat.
fn blocked_by(attacker: std::sync::Arc<CardDefinition>, walls: usize) -> Driver {
    let foes: Vec<_> = (0..walls)
        .map(|at| ready(free_creature(&format!("Wall {at}"), 0, 9)))
        .collect();
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(attacker)])
        .battlefield(1, foes);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Attacker", 1)]).unwrap();
    for _ in 0..60 {
        match &driver.decision().unwrap().kind {
            mtg_engine::decision::DecisionKind::DeclareBlockers(options) => {
                if driver.decision().unwrap().player == mtg_core::ids::PlayerId(1) {
                    let pairs: Vec<(String, &str)> = (0..walls)
                        .map(|at| (format!("Wall {at}"), "Attacker"))
                        .collect();
                    let chosen: Vec<(&str, &str)> =
                        pairs.iter().map(|(a, b)| (a.as_str(), *b)).collect();
                    assert!(options.blockers.len() == walls);
                    driver.block(&chosen).unwrap();
                } else {
                    driver.block(&[]).unwrap();
                }
            }
            mtg_engine::decision::DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::EndCombat
                    && driver.state().stack().is_empty() =>
            {
                return driver;
            }
            mtg_engine::decision::DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("combat did not end");
}

#[rstest]
#[case::rampage_two_blockers("K:Rampage:2", 2, (4, 24))]
#[case::rampage_three_blockers("K:Rampage:1", 3, (4, 24))]
#[case::rampage_one_blocker("K:Rampage:3", 1, (2, 22))]
#[case::bushido_on_block("K:Bushido:2", 1, (4, 24))]
fn a_translated_keyword_changes_the_creature_it_is_printed_on(
    #[case] line: &str,
    #[case] walls: usize,
    #[case] expected: (i32, i32),
) {
    let driver = blocked_by(card_of(line, "Attacker", 2, 22), walls);
    let (power, toughness) = size(&driver, "Attacker");
    assert_eq!((power, toughness), expected);
    for at in 0..walls {
        assert_eq!(
            size(&driver, &format!("Wall {at}")),
            (0, 9),
            "no wall changes"
        );
    }
}

#[rstest]
#[case::attack("K:CARDNAME can't attack alone.", &["CantAttackAlone"])]
#[case::block("K:CARDNAME can't block alone.", &["CantBlockAlone"])]
#[case::both("K:CARDNAME can't attack or block alone.", &["CantAttackAlone", "CantBlockAlone"])]
fn a_keyword_in_words_is_a_combat_rule_on_the_creature(
    #[case] line: &str,
    #[case] quirks: &[&str],
) {
    let expected: Vec<Value> = quirks
        .iter()
        .map(|quirk| json!({"Rule": {"rule": {"Combat": {"who": "This", "quirk": quirk}}}}))
        .collect();
    assert_eq!(abilities(&script(line)), expected);
}

#[rstest]
#[case::be_blocked("K:CARDNAME must be blocked if able.", json!({"MustBeBlocked": "AtLeastOne"}))]
#[case::all_block("K:All creatures able to block CARDNAME do so.", json!({"MustBeBlocked": "Every"}))]
#[case::all_block_by_name("K:MustBeBlockedByAll", json!({"MustBeBlocked": "Every"}))]
fn a_requirement_to_be_blocked_is_a_combat_rule(#[case] line: &str, #[case] quirk: Value) {
    assert_eq!(
        abilities(&script(line)),
        [json!({"Rule": {"rule": {"Combat": {"who": "This", "quirk": quirk}}}})]
    );
}

#[rstest]
#[case::exactly_one("K:CARDNAME must be blocked by exactly one creature if able.")]
#[case::two_or_more("K:CARDNAME must be blocked by two or more creatures if able.")]
#[case::attack_only_alone("K:CARDNAME can only attack alone.")]
fn a_combat_requirement_with_a_count_stays_a_gap(#[case] line: &str) {
    assert_eq!(gaps(&script(line)).len(), 1);
}

#[test]
fn a_creature_that_cant_attack_alone_attacks_with_a_partner_only() {
    let loner = card_of("K:CARDNAME can't attack alone.", "Loner", 2, 2);
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(loner), ready(free_creature("Partner", 1, 1))])
        .battlefield(1, vec![ready(free_creature("Wall", 0, 9))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    assert!(driver.attack(&[("Loner", 1)]).is_err(), "alone");
    driver.attack(&[("Loner", 1), ("Partner", 1)]).unwrap();
}

#[test]
fn a_creature_that_must_be_blocked_is_blocked_by_a_free_creature() {
    let bait = card_of("K:CARDNAME must be blocked if able.", "Attacker", 2, 9);
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(bait)])
        .battlefield(
            1,
            vec![
                ready(free_creature("Wall 0", 0, 9)),
                ready(free_creature("Wall 1", 0, 9)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Attacker", 1)]).unwrap();
    for _ in 0..30 {
        if let mtg_engine::decision::DecisionKind::DeclareBlockers(options) =
            &driver.decision().unwrap().kind
            && driver.decision().unwrap().player == mtg_core::ids::PlayerId(1)
        {
            assert_eq!(options.must_be_blocked, [driver.id_of("Attacker").unwrap()]);
            assert!(driver.block(&[]).is_err());
            driver.block(&[("Wall 1", "Attacker")]).unwrap();
            return;
        }
        driver.pass().unwrap();
    }
    panic!("no block declaration");
}

#[rstest]
#[case::attack_despite_defender(
    "S:Mode$ CanAttackDefender | ValidCard$ Card.Self | Description$ x\n",
    json!({"Combat": {"who": "This", "quirk": "AttackDespiteDefender"}})
)]
#[case::block_while_tapped(
    "S:Mode$ BlockTapped | ValidCard$ Creature.tapped+YouCtrl | Description$ x\n",
    json!({"Combat": {"who": {"Each": [{"Type": "Creature"}, "Tapped", {"ControlledBy": "You"}]}, "quirk": "BlockWhileTapped"}})
)]
#[case::hexproof_ignored(
    "S:Mode$ IgnoreHexproof | Activator$ You | ValidEntity$ Permanent.OppCtrl,Opponent | Description$ x\n",
    json!({"IgnoreHexproof": {"by": "You", "what": {"Each": [{"ControlledBy": "Opponents"}]}, "players": "Opponents"}})
)]
fn a_static_mode_of_combat_is_a_rule(#[case] line: &str, #[case] rule: Value) {
    let text = format!("Name:T\nTypes:Creature Elf\nPT:2/2\n{line}");
    assert_eq!(abilities(&text), [json!({"Rule": {"rule": rule}})]);
}

#[rstest]
#[case::with_a_condition(
    "S:Mode$ CanAttackDefender | ValidCard$ Card.Self | IsPresent$ Gate.YouCtrl | Description$ x\n"
)]
#[case::against_some_players(
    "S:Mode$ CanAttackDefender | ValidCard$ Card.Self | ValidAttacked$ Player.attackedYouTheirLastTurn | Description$ x\n"
)]
#[case::with_ward(
    "S:Mode$ IgnoreHexproof | ValidEntity$ Creature.OppCtrl | Description$ Creatures can be the targets as though they didn't have hexproof. Ward abilities of those creatures don't trigger.\n"
)]
#[case::another_activator(
    "S:Mode$ IgnoreHexproof | Activator$ Player.IsRemembered | ValidEntity$ Creature.OppCtrl | Description$ x\n"
)]
fn a_static_mode_with_a_condition_it_cannot_express_is_a_gap(#[case] line: &str) {
    let text = format!("Name:T\nTypes:Creature Elf\nPT:2/2\n{line}");
    assert_eq!(gaps(&text).len(), 1, "{:?}", gaps(&text));
    assert!(abilities(&text).is_empty());
}

#[test]
fn a_translated_permission_to_attack_lets_a_defender_attack() {
    let text = "Name:Wall\nTypes:Creature Wall\nPT:0/4\nK:Defender\nS:Mode$ CanAttackDefender | ValidCard$ Card.Self | Description$ x\n";
    let mut card = CardDefinition::new("Wall")
        .with_type(CardType::Creature)
        .with_power_toughness(0, 4);
    for ability in abilities(text) {
        card = card.with_rules(serde_json::from_value::<Ability>(ability).unwrap());
    }
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(std::sync::Arc::new(card))])
        .battlefield(1, vec![ready(free_creature("Foe", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Wall", 1)]).unwrap();
}

#[test]
fn the_json_of_a_translated_keyword_is_the_one_the_table_names() {
    let found: Value = abilities(&script("K:Rampage:2")).remove(0);
    let ability: Ability = serde_json::from_value(found).unwrap();
    let Ability::Keyword(mtg_cards::keyword::Keyword::Macro { name, amount }) = ability else {
        panic!("a macro keyword");
    };
    assert!(mtg_cards::macros::keyword_abilities(&name, amount).is_some());
}
