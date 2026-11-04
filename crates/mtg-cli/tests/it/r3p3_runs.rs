//! Goal: a spell that moves itself from the stack, or the spell a cast trigger is about, ends in the zone the script names (not the graveyard) when the translated card is played.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::card::Card;
use mtg_cards::support::{PrintedCard, PrintedFace, oracle_fingerprint};
use mtg_cards::{CardDefinition, builtin};
use mtg_cli::forge::script::Script;
use mtg_cli::translate::card::translate_script;
use mtg_cli::translate::ctx::World;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ready};

/// The definition the engine runs for a script translated whole, printed as `types` with the cost `cost`.
fn translated(script: &str, name: &str, types: &str, cost: &str) -> Arc<CardDefinition> {
    let out = translate_script(&World::default(), &Script::parse(script));
    let face = out.faces.into_iter().next().unwrap();
    assert!(face.gaps.is_empty(), "{:?}", face.gaps);
    let oracle_id = format!("00000000-0000-4000-8000-{:012}", name.len());
    let card: Card = serde_json::from_value(serde_json::json!({
        "oracle_id": oracle_id, "name": name, "impl_status": "Full",
        "faces": [{"name": name, "oracle_fingerprint": oracle_fingerprint(name), "abilities": face.abilities}]}))
    .unwrap();
    let printed = PrintedCard {
        oracle_id,
        name: name.to_owned(),
        keywords: vec![],
        layout: "normal".to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: cost.to_owned(),
            type_line: types.to_owned(),
            oracle_text: name.to_owned(),
            power: (types.contains("Creature")).then(|| "1".to_owned()),
            toughness: (types.contains("Creature")).then(|| "1".to_owned()),
            loyalty: None,
            defense: None,
            colors: vec![],
        }],
    };
    let defined = mtg_cli::drafts::read_definitions(
        builtin::library().unwrap(),
        &mtg_engine::caps(),
        &[(&printed, &card)],
    );
    Arc::new(defined.into_iter().next().unwrap().unwrap())
}

/// Answers every question yes and picks the first cards until the stack is empty again.
fn settle(driver: &mut Driver) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(true)),
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                o.from.iter().copied().take(o.max as usize).collect(),
            )),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game does not settle");
}

fn named(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .collect()
}

const HEALING: &str = "Name:Mend Lore\nManaCost:3 G\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Graveyard | Destination$ Hand | TgtPrompt$ Select target card in your graveyard | ValidTgts$ Card.YouOwn | SubAbility$ DBChange | SpellDescription$ Return target card from your graveyard to your hand. Exile CARDNAME.\nSVar:DBChange:DB$ ChangeZone | Origin$ Stack | Destination$ Exile | StackDescription$ None\n";
const SHUFFLE: &str = "Name:Loop Lore\nManaCost:2 B\nTypes:Sorcery\nA:SP$ GainLife | LifeAmount$ 3 | SubAbility$ DBShuffle | SpellDescription$ You gain 3 life. Shuffle CARDNAME into its owner's library.\nSVar:DBShuffle:DB$ ChangeZone | Origin$ Stack | Destination$ Library | Shuffle$ True | Defined$ Parent\n";
const BOUNCE: &str = "Name:Echo Imp\nManaCost:R\nTypes:Creature Imp\nPT:1/1\nT:Mode$ SpellCast | ValidCard$ Instant,Sorcery | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigBack | TriggerDescription$ Whenever you cast an instant or sorcery spell, return that spell to its owner's hand.\nSVar:TrigBack:DB$ ChangeZone | Defined$ TriggeredSpellAbility | Origin$ Stack | Destination$ Hand\n";
const GAIN: &str = "Name:Small Boon\nManaCost:R\nTypes:Sorcery\nA:SP$ GainLife | LifeAmount$ 1 | SpellDescription$ You gain 1 life.\n";

#[test]
fn a_spell_that_exiles_itself_ends_in_exile() {
    let spell = translated(HEALING, "Mend Lore", "Sorcery", "{3}{G}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .graveyard(0, [bear()])
        .battlefield(0, (0..4).map(|_| ready(forest())));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let bears = named(
        &driver,
        Zone::Graveyard(PlayerId::from_index(0)),
        "Grizzly Bears",
    )[0];
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(bears).unwrap().reference());
    driver.cast_at("Mend Lore", &[target]).unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    assert_eq!(named(&driver, Zone::Exile, "Mend Lore").len(), 1);
    assert_eq!(named(&driver, Zone::Graveyard(me), "Mend Lore").len(), 0);
    assert_eq!(named(&driver, Zone::Hand(me), "Grizzly Bears").len(), 1);
}

#[test]
fn a_spell_that_shuffles_itself_in_ends_in_the_library() {
    let spell = translated(SHUFFLE, "Loop Lore", "Sorcery", "{2}{B}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, (0..3).map(|_| ready(swamp())));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Loop Lore").unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    assert_eq!(named(&driver, Zone::Library(me), "Loop Lore").len(), 1);
    assert_eq!(named(&driver, Zone::Graveyard(me), "Loop Lore").len(), 0);
}

#[test]
fn a_cast_trigger_returns_the_spell_to_hand_without_resolving_it() {
    let imp = translated(BOUNCE, "Echo Imp", "Creature — Imp", "{R}");
    let boon = translated(GAIN, "Small Boon", "Sorcery", "{R}");
    let scenario = Scenario::new(2)
        .hand(0, [boon])
        .battlefield(0, [ready(mountain()), ready(imp)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let life = driver
        .state()
        .player(PlayerId::from_index(0))
        .unwrap()
        .life();
    driver.cast("Small Boon").unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    assert_eq!(named(&driver, Zone::Hand(me), "Small Boon").len(), 1);
    assert_eq!(
        driver.state().player(me).unwrap().life(),
        life,
        "the spell never resolved"
    );
}

const BOG: &str = "Name:Grave Wipe\nManaCost:B\nTypes:Sorcery\nA:SP$ ChangeZoneAll | ValidTgts$ Player | Origin$ Graveyard | Destination$ Exile | ChangeType$ Card | SpellDescription$ Exile target player's graveyard.\n";

#[test]
fn a_targeted_players_graveyard_is_exiled_and_the_other_stays() {
    let spell = translated(BOG, "Grave Wipe", "Sorcery", "{B}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .graveyard(0, [bear()])
        .graveyard(1, [bear(), bear()])
        .battlefield(0, [ready(swamp())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.cast_at("Grave Wipe", &[target]).unwrap();
    settle(&mut driver);
    assert_eq!(named(&driver, Zone::Exile, "Grizzly Bears").len(), 2);
    assert_eq!(
        named(
            &driver,
            Zone::Graveyard(PlayerId::from_index(0)),
            "Grizzly Bears"
        )
        .len(),
        1
    );
}

const WANDER: &str = "Name:Wander Lore\nManaCost:2 G\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Library | ChangeType$ Land.Basic | ChangeNum$ 2 | RememberChanged$ True | Reveal$ True | Shuffle$ False | SubAbility$ Hand\nSVar:Hand:DB$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ Land.IsRemembered | Mandatory$ True | NoLooking$ True | SubAbility$ Rest\nSVar:Rest:DB$ ChangeZone | Origin$ Library | Destination$ Battlefield | ChangeType$ Land.IsRemembered | Tapped$ True | Mandatory$ True | NoLooking$ True\n";

#[test]
fn a_search_that_leaves_the_cards_in_the_library_feeds_the_next_steps() {
    let spell = translated(WANDER, "Wander Lore", "Sorcery", "{2}{G}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, (0..3).map(|_| ready(forest())))
        .library(0, [forest(), forest(), bear(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Wander Lore").unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    assert_eq!(named(&driver, Zone::Hand(me), "Forest").len(), 1);
    assert_eq!(named(&driver, Zone::Battlefield, "Forest").len(), 4);
}

const BRAIN: &str = "Name:Think Lore\nManaCost:U\nTypes:Instant\nA:SP$ Draw | NumCards$ 3 | SubAbility$ Back\nSVar:Back:DB$ ChangeZone | Origin$ Hand | Destination$ Library | ChangeNum$ 2 | Mandatory$ True | Reorder$ True\n";

#[test]
fn two_cards_from_the_hand_go_back_on_top_of_the_library() {
    let spell = translated(BRAIN, "Think Lore", "Instant", "{U}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(0, [ready(mtg_testkit::cards::island())])
        .library(0, [forest(), forest(), forest(), bear(), bear()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Think Lore").unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    assert_eq!(
        driver.state().zone(Zone::Hand(me)).len(),
        1,
        "three drawn, two put back"
    );
}
