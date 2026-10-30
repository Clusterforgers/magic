//! Goal: `ObjectView.gives` says what each Aura and Equipment gives its host as the layers apply it now (power and toughness, keywords with protection worded by its qualities, restrictions, umbra armor), and what an Aura on a player gives that player, to every seat alike, only its own share when a host has several, nothing for an unattached Equipment, an attachment that gives nothing or a condition that fails, and the host's characteristics are its base plus the sum of what its attachments give.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::gives::GivenRule;
use mtg_engine::keywords::player_keywords;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::zones::Zone;
use mtg_protocol::{GivesView, ObjectView, StateView};
use mtg_testkit::cards::bear;
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, attached_to, enchanting, ready};
use proptest::prelude::*;
use rstest::rstest;
use serde_json::json;

/// A card of type `kind` (and subtype `subtype`) with the abilities `rules`, each written as data.
fn card(name: &str, kind: CardType, subtype: &str, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(kind)
        .with_subtype(subtype);
    for rule in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(card)
}

/// An Aura that enchants a creature, with these abilities.
fn aura(name: &str, rules: &[&str]) -> Arc<CardDefinition> {
    let mut all = vec![r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#];
    all.extend_from_slice(rules);
    card(name, CardType::Enchantment, "Aura", &all)
}

/// An Equipment with these abilities.
fn equipment(name: &str, rules: &[&str]) -> Arc<CardDefinition> {
    card(name, CardType::Artifact, "Equipment", rules)
}

/// A static ability of an attachment that makes `mods` (data) to its host.
fn on_host(mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":"Attached","mods":{mods}}}}}"#)
}

/// A table where seat 0 has a Bear (place 0) with `attachments` on it, then `others`; seat 1 has nothing.
fn table(attachments: Vec<Arc<CardDefinition>>, others: Vec<PresetPermanent>) -> Driver {
    let mut permanents = vec![ready(bear())];
    permanents.extend(attachments.into_iter().map(|a| attached_to(ready(a), 0, 0)));
    permanents.extend(others);
    Driver::from_scenario(Scenario::new(2).battlefield(0, permanents)).unwrap()
}

fn object(driver: &Driver, viewer: usize, name: &str) -> ObjectView {
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
        .objects
        .into_iter()
        .find(|o| o.name == name && o.zone == Zone::Battlefield)
        .unwrap()
}

// It returns what the view field holds, so a case compares against it directly.
#[allow(clippy::unnecessary_wraps)]
fn gives(power: i32, toughness: i32, keywords: &[&str], rules: &[GivenRule]) -> Option<GivesView> {
    Some(GivesView {
        power,
        toughness,
        set: None,
        keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
        rules: rules.to_vec(),
    })
}

fn pacifism() -> Arc<CardDefinition> {
    aura("Pacifism", &[&on_host(r#"["CantAttack","CantBlock"]"#)])
}

fn mighty_wings() -> Arc<CardDefinition> {
    aura(
        "Mighty Wings",
        &[&on_host(
            r#"[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Flying"}]"#,
        )],
    )
}

fn bonesplitter() -> Arc<CardDefinition> {
    equipment(
        "Splitter",
        &[&on_host(r#"[{"ModifyPT":{"power":2,"toughness":0}}]"#)],
    )
}

fn ward_blade() -> Arc<CardDefinition> {
    equipment(
        "Ward Blade",
        &[&on_host(
            r#"[{"AddKeyword":{"Protection":[{"Color":"Red"}]}},{"AddKeyword":"Lifelink"}]"#,
        )],
    )
}

fn umbra() -> Arc<CardDefinition> {
    aura(
        "Hyena Shade",
        &[
            &on_host(r#"[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"FirstStrike"}]"#),
            r#"{"Keyword":"UmbraArmor"}"#,
        ],
    )
}

/// "As long as you control an artifact, enchanted creature gets +2/+2."
fn forged_aura() -> Arc<CardDefinition> {
    aura(
        "Forged Aura",
        &[
            r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}"#,
        ],
    )
}

fn frog_curse() -> Arc<CardDefinition> {
    aura(
        "Frog Curse",
        &[&on_host(
            r#"["LoseAbilities",{"SetPT":{"power":1,"toughness":1}}]"#,
        )],
    )
}

fn trinket() -> PresetPermanent {
    ready(card("Trinket", CardType::Artifact, "Charm", &[]))
}

#[rstest]
#[case::pacifism(vec![pacifism()], vec![], "Pacifism", gives(0, 0, &[], &[GivenRule::CantAttack, GivenRule::CantBlock]))]
#[case::pump_and_flying(vec![mighty_wings()], vec![], "Mighty Wings", gives(2, 2, &["Flying"], &[]))]
#[case::equipment_pump(vec![bonesplitter()], vec![], "Splitter", gives(2, 0, &[], &[]))]
#[case::protection_and_lifelink(vec![ward_blade()], vec![], "Ward Blade", gives(0, 0, &["Protection from red", "Lifelink"], &[]))]
#[case::umbra(vec![umbra()], vec![], "Hyena Shade", gives(1, 1, &["First strike"], &[GivenRule::UmbraArmor]))]
#[case::condition_holds(vec![forged_aura()], vec![trinket()], "Forged Aura", gives(2, 2, &[], &[]))]
#[case::condition_fails(vec![forged_aura()], vec![], "Forged Aura", None)]
#[case::gives_nothing(vec![aura("Plain Aura", &[])], vec![], "Plain Aura", None)]
#[case::unattached(vec![], vec![ready(bonesplitter())], "Splitter", None)]
#[case::first_of_two(vec![bonesplitter(), mighty_wings()], vec![], "Splitter", gives(2, 0, &[], &[]))]
#[case::second_of_two(vec![bonesplitter(), mighty_wings()], vec![], "Mighty Wings", gives(2, 2, &["Flying"], &[]))]
fn an_attachment_shows_what_it_gives_its_host(
    #[case] attachments: Vec<Arc<CardDefinition>>,
    #[case] others: Vec<PresetPermanent>,
    #[case] name: &str,
    #[case] expected: Option<GivesView>,
) {
    let driver = table(attachments, others);
    for viewer in 0..2 {
        assert_eq!(
            object(&driver, viewer, name).gives,
            expected,
            "seat {viewer}"
        );
    }
    assert_eq!(object(&driver, 0, "Grizzly Bears").gives, None);
}

/// Equipment that grants each of `qualities` (protection data) one after the other, then `after`.
fn protector(qualities: &[&str], after: &[&str]) -> Arc<CardDefinition> {
    let mut mods: Vec<String> = qualities
        .iter()
        .map(|quality| format!(r#"{{"AddKeyword":{{"Protection":{quality}}}}}"#))
        .collect();
    mods.extend(
        after
            .iter()
            .map(|keyword| format!(r#"{{"AddKeyword":"{keyword}"}}"#)),
    );
    equipment("Protector", &[&on_host(&format!("[{}]", mods.join(",")))])
}

#[rstest]
#[case::one_color(&[r#"[{"Color":"Red"}]"#], &[], &["Protection from red"])]
#[case::two_colors(&[r#"[{"Color":"Red"}]"#, r#"[{"Color":"Blue"}]"#], &[], &["Protection from red and from blue"])]
#[case::a_kind(&[r#"[{"Type":"Creature"}]"#], &[], &["Protection from creatures"])]
#[case::a_color_and_a_kind(&[r#"[{"Color":"White"}]"#, r#"[{"Type":"Land"}]"#], &[], &["Protection from white and from lands"])]
#[case::a_keyword_between(&[r#"[{"Color":"Red"}]"#], &["Flying"], &["Protection from red", "Flying"])]
#[case::no_word(&[r#"[{"Subtype":"Rat"},{"Color":"Black"}]"#], &[], &["Protection"])]
fn protection_is_worded_by_its_qualities(
    #[case] qualities: &[&str],
    #[case] after: &[&str],
    #[case] expected: &[&str],
) {
    let driver = table(vec![protector(qualities, after)], vec![]);
    for viewer in 0..2 {
        let shown = object(&driver, viewer, "Protector").gives.unwrap();
        assert_eq!(shown.keywords, expected, "seat {viewer}");
    }
}

/// An Aura that enchants a player, with a static ability that makes `mods` (data) to that player.
fn curse(name: &str, mods: &str) -> Arc<CardDefinition> {
    card(
        name,
        CardType::Enchantment,
        "Aura",
        &[
            r#"{"Keyword":{"EnchantPlayer":{"opponent":false}}}"#,
            &format!(r#"{{"Static":{{"affected":"AttachedPlayer","mods":{mods}}}}}"#),
        ],
    )
}

/// Seat 0 controls `auras`, each enchanting seat 1.
fn cursed(auras: Vec<Arc<CardDefinition>>) -> Driver {
    let permanents = auras.into_iter().map(|a| enchanting(ready(a), 1));
    Driver::from_scenario(Scenario::new(2).battlefield(0, permanents)).unwrap()
}

#[rstest]
#[case::hexproof("Ward of Hexes", r#"[{"AddKeyword":"Hexproof"}]"#, gives(0, 0, &["Hexproof"], &[]))]
#[case::shroud_and_protection("Veil", r#"[{"AddKeyword":"Shroud"},{"AddKeyword":{"Protection":[{"Type":"Creature"}]}}]"#, gives(0, 0, &["Shroud", "Protection from creatures"], &[]))]
#[case::takes_a_keyword("Exposure", r#"[{"RemoveKeyword":"Hexproof"}]"#, gives(0, 0, &[], &[GivenRule::LoseKeyword]))]
#[case::gives_nothing("Empty Curse", "[]", None)]
fn an_aura_on_a_player_shows_what_it_gives_that_player(
    #[case] name: &str,
    #[case] mods: &str,
    #[case] expected: Option<GivesView>,
) {
    let driver = cursed(vec![curse(name, mods)]);
    for viewer in 0..2 {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        let aura = view.objects.iter().find(|o| o.name == name).unwrap();
        assert_eq!(aura.gives, expected, "seat {viewer}");
        assert_eq!(view.table.players[1].enchanted_by, [aura.id]);
    }
}

#[test]
fn each_aura_on_a_player_shows_only_its_own_share() {
    let driver = cursed(vec![
        curse("Ward of Hexes", r#"[{"AddKeyword":"Hexproof"}]"#),
        curse("Veil", r#"[{"AddKeyword":"Shroud"}]"#),
    ]);
    assert_eq!(
        object(&driver, 1, "Ward of Hexes").gives,
        gives(0, 0, &["Hexproof"], &[])
    );
    assert_eq!(
        object(&driver, 0, "Veil").gives,
        gives(0, 0, &["Shroud"], &[])
    );
}

#[test]
fn an_aura_that_sets_and_takes_abilities_shows_both() {
    let driver = table(vec![frog_curse()], vec![]);
    let shown = object(&driver, 1, "Frog Curse").gives.unwrap();
    assert_eq!(shown.set, Some((1, 1)));
    assert_eq!(shown.rules, vec![GivenRule::LoseAbilities]);
    let host = object(&driver, 1, "Grizzly Bears");
    assert_eq!((host.power, host.toughness), (Some(1), Some(1)));
}

#[test]
fn the_field_is_left_out_of_the_json_while_empty() {
    let driver = table(vec![bonesplitter(), aura("Plain Aura", &[])], vec![]);
    let plain = serde_json::to_value(object(&driver, 0, "Plain Aura")).unwrap();
    assert!(plain.get("gives").is_none());
    let splitter = serde_json::to_value(object(&driver, 0, "Splitter")).unwrap();
    assert_eq!(splitter["gives"], json!({ "power": 2 }));
}

#[rstest]
#[case::bonesplitter("Bonesplitter")]
#[case::epic_proportions("Epic Proportions")]
#[case::swiftfoot_boots("Swiftfoot Boots")]
#[case::control_magic("Control Magic")]
#[case::pacifism("Pacifism")]
fn a_compiled_attachment_shows_what_its_card_gives(#[case] name: &str) {
    let card = corpus::card(name).unwrap();
    let driver = table(vec![card], vec![]);
    let shown = object(&driver, 1, name).gives.unwrap();
    let expected = match name {
        "Bonesplitter" => gives(2, 0, &[], &[]),
        "Epic Proportions" => gives(5, 5, &["Trample"], &[]),
        "Control Magic" => gives(0, 0, &[], &[GivenRule::SetController]),
        "Pacifism" => gives(0, 0, &[], &[GivenRule::CantAttack, GivenRule::CantBlock]),
        _ => gives(0, 0, &["Hexproof", "Haste"], &[]),
    };
    assert_eq!(Some(shown), expected);
}

/// The simple attachments the property draws from.
fn pool() -> Vec<Arc<CardDefinition>> {
    vec![
        bonesplitter(),
        mighty_wings(),
        umbra(),
        ward_blade(),
        aura(
            "Weak Aura",
            &[&on_host(r#"[{"ModifyPT":{"power":-1,"toughness":0}}]"#)],
        ),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn a_host_is_its_base_plus_what_its_attachments_give(picks in proptest::collection::vec(0..5usize, 0..5)) {
        let pool = pool();
        let driver = table(picks.iter().map(|i| Arc::clone(&pool[*i])).collect(), vec![]);
        let view = StateView::for_player(driver.state(), PlayerId::from_index(0));
        let host = view.objects.iter().find(|o| o.name == "Grizzly Bears").unwrap();
        let (mut power, mut toughness, mut keywords) = (2, 2, Vec::<String>::new());
        for id in &host.attachments {
            let shown = view.objects.iter().find(|o| o.id == *id).unwrap();
            let Some(given) = &shown.gives else { continue };
            power += given.power;
            toughness += given.toughness;
            for keyword in &given.keywords {
                let name = keyword.split(" from ").next().unwrap().to_owned();
                if !keywords.contains(&name) {
                    keywords.push(name);
                }
            }
        }
        prop_assert_eq!((host.power, host.toughness), (Some(power), Some(toughness)));
        let mut have = host.keywords.clone();
        have.sort();
        keywords.sort();
        prop_assert_eq!(have, keywords);
    }
}

#[test]
fn an_effect_the_attachment_made_counts_and_another_source_does_not() {
    let mut driver = table(vec![bonesplitter()], vec![trinket()]);
    let host = object(&driver, 0, "Grizzly Bears").id;
    let splitter = object(&driver, 0, "Splitter").id;
    let trinket = object(&driver, 0, "Trinket").id;
    let bears = Selector::Objects(vec![driver.state().object(host).unwrap().reference()]);
    let pump = Modification::ModifyPowerToughness {
        power: 1,
        toughness: 1,
    };
    for source in [splitter, trinket] {
        driver
            .add_effect_from(
                source,
                bears.clone(),
                pump.clone(),
                Duration::UntilEndOfTurn,
            )
            .unwrap();
    }
    assert_eq!(object(&driver, 1, "Splitter").gives, gives(3, 1, &[], &[]));
    assert_eq!(object(&driver, 1, "Grizzly Bears").power, Some(6));
}

/// The Auras on a player the property draws from.
fn curses() -> Vec<Arc<CardDefinition>> {
    vec![
        curse("Ward of Hexes", r#"[{"AddKeyword":"Hexproof"}]"#),
        curse("Veil", r#"[{"AddKeyword":"Shroud"}]"#),
        curse(
            "Twin Ward",
            r#"[{"AddKeyword":"Hexproof"},{"AddKeyword":"Shroud"}]"#,
        ),
        curse("Empty Curse", "[]"),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn a_player_has_the_keywords_their_auras_give(picks in proptest::collection::vec(0..4usize, 0..4)) {
        let pool = curses();
        let driver = cursed(picks.iter().map(|i| Arc::clone(&pool[*i])).collect());
        let view = StateView::for_player(driver.state(), PlayerId::from_index(0));
        let mut given: Vec<String> = view.table.players[1]
            .enchanted_by
            .iter()
            .filter_map(|id| view.objects.iter().find(|o| o.id == *id)?.gives.clone())
            .flat_map(|gives| gives.keywords)
            .collect();
        given.sort();
        given.dedup();
        let mut has: Vec<String> = player_keywords(driver.state(), PlayerId(1))
            .into_iter()
            .map(|keyword| keyword.name().to_owned())
            .collect();
        has.sort();
        prop_assert_eq!(given, has);
    }
}
