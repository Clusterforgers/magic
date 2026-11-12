//! Goal: pack e15g's pieces play whole at tables of two, three and four seats: a Licid loses its ability, becomes an Aura with enchant creature attached to the target, and only its controller may end the effect by paying, a special action that uses no stack, after which it is a creature again, unattached and with its ability; an Aura Licid whose creature leaves goes to the graveyard; a spell that gains keywords on the stack from a cast trigger deals damage with them, and one that gains rebound is exiled as it resolves; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::ObjectId;
use mtg_engine::ability_reader::abilities_of;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PaidEnd, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::pay;
use crate::common::pid;

/// Calming Licid as the translator writes it: "{W}, {T}: ~ loses this ability and becomes an Aura enchantment with
/// enchant creature. Attach it to target creature. You may pay {W} to end this effect." and "Enchanted creature can't
/// attack."
fn licid() -> Arc<CardDefinition> {
    let rules = [
        r#"{"Activated":{"cost":[{"Mana":"{W}"},"Tap"],"effects":[{"Continuous":{"affected":"This","duration":{"UntilPaid":"{W}"},"mods":[{"AddSubtype":"Aura"},{"SetTypes":["Enchantment"]},{"AddKeyword":{"Enchant":[{"Type":"Creature"}]}},"LoseThisAbility"]}},{"Attach":{"to":{"Target":0},"what":"This"}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#,
        r#"{"Static":{"affected":"Attached","mods":["CantAttack"]}}"#,
    ];
    let mut def = CardDefinition::new("Calming Licid")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    def.subtypes = vec![Subtype("Licid".to_owned())];
    def.rules = rules
        .iter()
        .map(|json| Arc::new(serde_json::from_str::<Ability>(json).unwrap()))
        .collect();
    Arc::new(def)
}

/// A creature with "Whenever you cast an instant or sorcery spell, that spell gains <keywords>."
fn granter(name: &str, keywords: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"by":"You","what":[{{"AnyOf":[{{"Type":"Instant"}},{{"Type":"Sorcery"}}]}}]}}}},"effects":[{{"Continuous":{{"affected":"Triggering","duration":"Forever","mods":{keywords}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str::<Ability>(&json).unwrap()),
    )
}

/// Passes and answers passively until the active player holds priority with an empty stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?;
        if decision.player == pid(0)
            && matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().zone(Zone::Stack).is_empty()
        {
            return Ok(());
        }
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    panic!("the stack does not settle");
}

/// The effects the deciding player may end by paying now.
fn endable(driver: &Driver) -> Vec<PaidEnd> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.endable.clone(),
        _ => Vec::new(),
    }
}

fn is(driver: &Driver, id: ObjectId, kind: CardType) -> bool {
    characteristics(driver.state(), id).is_some_and(|chars| chars.types.contains(kind))
}

fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(licid()), ready(plains()), ready(plains())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))],
        );
    }
    scenario
}

/// Activates the Licid at Bear 1 and lets it resolve.
fn enchant(driver: &mut Driver) -> Result<(ObjectId, ObjectId), ScriptError> {
    let (licid, bear) = (driver.id_of("Calming Licid")?, driver.id_of("Bear 1")?);
    driver.begin_activation("Calming Licid")?;
    let target = driver.target_named("Bear 1")?;
    driver.choose_targets(&[target])?;
    pay(driver)?;
    settle(driver)?;
    Ok((licid, bear))
}

#[rstest]
fn a_licid_becomes_an_aura_until_its_controller_pays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let printed = abilities_of(driver.state(), driver.id_of("Calming Licid")?).len();
    let (licid, bear) = enchant(&mut driver)?;
    assert!(driver.state().zone(Zone::Battlefield).contains(&licid));
    assert!(is(&driver, licid, CardType::Enchantment) && !is(&driver, licid, CardType::Creature));
    assert_eq!(
        driver.state().host_of(licid),
        Some(bear),
        "attached to the target"
    );
    assert_eq!(
        abilities_of(driver.state(), licid).len(),
        printed - 1,
        "loses this ability"
    );
    let offers = endable(&driver);
    assert_eq!(offers.len(), 1, "its controller may end the effect");
    assert_eq!(
        (offers[0].object, offers[0].cost.to_string()),
        (licid, "{W}".to_owned())
    );
    // An opponent who holds priority may not end it.
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    assert!(endable(&driver).is_empty(), "not the opponent's to end");
    settle(&mut driver)?;
    let effect = endable(&driver)[0].effect;
    driver.answer(Answer::Priority(PriorityAction::EndEffect(effect)))?;
    pay(&mut driver)?;
    assert!(
        driver.state().zone(Zone::Stack).is_empty(),
        "a special action uses no stack"
    );
    assert_eq!(
        driver.decision()?.player,
        pid(0),
        "the payer keeps priority"
    );
    assert!(is(&driver, licid, CardType::Creature) && !is(&driver, licid, CardType::Enchantment));
    assert_eq!(driver.state().host_of(licid), None, "no longer attached");
    assert_eq!(
        abilities_of(driver.state(), licid).len(),
        printed,
        "the ability is back"
    );
    assert!(endable(&driver).is_empty(), "ended once");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn an_aura_licid_without_its_creature_goes_to_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table(seats)
            .hand(0, vec![burn("Shock", 2)])
            .battlefield(0, vec![ready(licid()), ready(plains()), ready(mountain())]),
    )?;
    let (licid, _) = enchant(&mut driver)?;
    driver.begin_cast("Shock")?;
    let target = driver.target_named("Bear 1")?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let owner = Zone::Graveyard(pid(0));
    let dead = driver.state().zone(owner).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == "Calming Licid")
    });
    assert!(dead, "the Aura is put into its owner's graveyard");
    assert!(!driver.state().zone(Zone::Battlefield).contains(&licid));
    assert!(endable(&driver).is_empty(), "the effect is gone with it");
    Ok(())
}

#[rstest]
fn a_spell_deals_damage_with_the_keywords_it_gained(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![burn("Shock", 1)])
        .battlefield(
            0,
            vec![
                ready(granter(
                    "Judith",
                    r#"[{"AddKeyword":"Deathtouch"},{"AddKeyword":"Lifelink"}]"#,
                )),
                ready(mountain()),
            ],
        )
        .battlefield(1, vec![ready(free_creature("Wall", 0, 5))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.begin_cast("Shock")?;
    let target = driver.target_named("Wall")?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    assert!(
        driver.id_of("Wall").is_err(),
        "deathtouch destroys the wall"
    );
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        life + 1,
        "lifelink"
    );
    Ok(())
}

#[rstest]
fn a_spell_that_gains_rebound_is_exiled_as_it_resolves(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] granted: bool,
) -> Result<(), ScriptError> {
    let mine = if granted {
        vec![
            ready(granter("Taigam", r#"[{"AddKeyword":"Rebound"}]"#)),
            ready(mountain()),
        ]
    } else {
        vec![ready(free_creature("Taigam", 1, 1)), ready(mountain())]
    };
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![burn("Shock", 1)])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Shock")?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    settle(&mut driver)?;
    let held = |zone: Zone| {
        driver.state().zone(zone).iter().any(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Shock")
        })
    };
    assert_eq!(
        (held(Zone::Exile), held(Zone::Graveyard(pid(0)))),
        (granted, !granted)
    );
    Ok(())
}
