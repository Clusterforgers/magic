//! Goal: the two triggers of Oblivion Ring, Fiend Hunter, the Wormfang cards, Icy Prison and the other exile-and-return cards stay two linked abilities, an exile for good (a target, a choice of the controller or of an opponent, the exile that keeps the card, a pick from a revealed hand, cards from a single graveyard, creatures from the battlefield and/or graveyards, or a target with its namesakes, beside the cast prohibition on the exiled name) and a return of what the card exiled to the battlefield, a hand or a graveyard, that read back as the printed text and are Verified, while "until ~ leaves" stays one exile, and a return trigger without such an exile before it stays with the table.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::ability::Ability;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::compile;
use mtg_cards::effect::Effect;
use mtg_cards::load::{Library, canonical};
use mtg_cards::modify::Duration;
use mtg_cards::proposal::{Verdict, check_exact, differing};
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};
use rstest::rstest;

const RETURN: &str = "When this permanent leaves the battlefield, return the exiled card to the battlefield under its owner's control.";

fn card(name: &str, oracle_id: &str, type_line: &str, text: &str) -> PrintedCard {
    PrintedCard {
        layout: "normal".into(),
        oracle_id: oracle_id.into(),
        name: name.into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

/// The durations of the exiles the abilities run, inside a `May` too.
fn exile_durations(abilities: &[Ability]) -> Vec<Option<Duration>> {
    fn walk(effects: &[Effect], found: &mut Vec<Option<Duration>>) {
        for effect in effects {
            match effect {
                Effect::Exile { duration, .. } => found.push(duration.clone()),
                Effect::May { effects, .. } => walk(effects, found),
                _ => {}
            }
        }
    }
    let mut found = Vec::new();
    for ability in abilities {
        walk(ability.effects(), &mut found);
    }
    found
}

#[rstest]
#[case::oblivion_ring(
    "Oblivion Ring",
    "bd9b9772-f5f9-4c6b-913e-7193bea5d0a7",
    "Enchantment",
    "When this enchantment enters, exile another target nonland permanent.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::journey_to_nowhere(
    "Journey to Nowhere",
    "2bfec482-be0d-4768-84e7-1c2c74ea1373",
    "Enchantment",
    "When this enchantment enters, exile target creature.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::fiend_hunter(
    "Fiend Hunter",
    "cb9d557a-fc06-428c-8be6-7d28add33028",
    "Creature — Human Cleric",
    "When this creature enters, you may exile another target creature.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::leonin_relic_warder(
    "Leonin Relic-Warder",
    "8e720216-a980-4a34-8aef-62ac3ef15a6e",
    "Creature — Cat Cleric",
    "When this creature enters, you may exile target artifact or enchantment.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::faceless_butcher(
    "Faceless Butcher",
    "5237a105-60c4-4597-b24e-84c12ce7b01f",
    "Creature — Nightmare Horror",
    "When this creature enters, exile another target creature.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::petravark(
    "Petravark",
    "aedf022c-6065-42cb-85bb-225ae1650a5c",
    "Creature — Nightmare Beast",
    "When this creature enters, exile target land.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::wormfang_newt(
    "Wormfang Newt",
    "be014a79-5550-4f79-8789-b4d6783d7ec8",
    "Creature — Nightmare Salamander Beast",
    "When this creature enters, exile a land you control.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::wormfang_turtle(
    "Wormfang Turtle",
    "771f9ba4-6c65-4497-ad48-869994435556",
    "Creature — Nightmare Turtle Beast",
    "When this creature enters, exile a land you control.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::wormfang_crab(
    "Wormfang Crab",
    "4216f9ce-bdd9-4213-8c3f-e2ef4cd848f9",
    "Creature — Nightmare Crab",
    "This creature can't be blocked.\nWhen this creature enters, an opponent chooses a permanent you control other than this creature and exiles it.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::wormfang_drake(
    "Wormfang Drake",
    "d37c4e77-5023-4d13-9cfe-a43f513364bf",
    "Creature — Nightmare Drake",
    "Flying\nWhen this creature enters, sacrifice it unless you exile a creature you control other than this creature.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::icy_prison(
    "Icy Prison",
    "d4e2cc1a-4212-4284-9ebe-d3c40931de50",
    "Enchantment",
    "When this enchantment enters, exile target creature.\nAt the beginning of your upkeep, sacrifice this enchantment unless any player pays {3}.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::petradon(
    "Petradon",
    "ef2e711f-7362-4eed-8f91-f3d653e7f434",
    "Creature — Nightmare Beast",
    "When this creature enters, exile two target lands.\nWhen this creature leaves the battlefield, return the exiled cards to the battlefield under their owners' control.\n{R}: This creature gets +1/+0 until end of turn."
)]
#[case::realm_razer(
    "Realm Razer",
    "90646d77-9b35-4f87-94ee-255c9f150456",
    "Creature — Beast",
    "When this creature enters, exile all lands.\nWhen this creature leaves the battlefield, return the exiled cards to the battlefield tapped under their owners' control."
)]
#[case::tidehollow_sculler(
    "Tidehollow Sculler",
    "51b55057-8b14-4277-894b-4813b4f7eb39",
    "Artifact Creature — Zombie",
    "When this creature enters, target opponent reveals their hand and you choose a nonland card from it. Exile that card.\nWhen this creature leaves the battlefield, return the exiled card to its owner's hand."
)]
#[case::mesmeric_fiend(
    "Mesmeric Fiend",
    "fb46b300-8a39-40d2-8bc7-31b90c822e22",
    "Creature — Nightmare Horror",
    "When this creature enters, target opponent reveals their hand and you choose a nonland card from it. Exile that card.\nWhen this creature leaves the battlefield, return the exiled card to its owner's hand."
)]
#[case::gravegouger(
    "Gravegouger",
    "2677c0d9-1251-478e-b157-072859c593a9",
    "Creature — Nightmare Horror",
    "When this creature enters, exile up to two target cards from a single graveyard.\nWhen this creature leaves the battlefield, return the exiled cards to their owner's graveyard."
)]
#[case::detention_sphere(
    "Detention Sphere",
    "0a0b376d-7824-4ba5-8bc9-bc0be12800c5",
    "Enchantment",
    "When this enchantment enters, you may exile target nonland permanent not named Detention Sphere and all other permanents with the same name as that permanent.\nWhen this enchantment leaves the battlefield, return the exiled cards to the battlefield under their owner's control."
)]
#[case::angel_of_serenity(
    "Angel of Serenity",
    "f4ce6078-8c7b-4f68-b324-13130c63a983",
    "Creature — Angel",
    "Flying\nWhen this creature enters, you may exile up to three other target creatures from the battlefield and/or creature cards from graveyards.\nWhen this creature leaves the battlefield, return the exiled cards to their owners' hands."
)]
#[case::ashioks_erasure(
    "Ashiok's Erasure",
    "13cf3ef7-d88e-4cc7-a4d8-cedc671f356a",
    "Enchantment",
    "Flash\nWhen this enchantment enters, exile target spell.\nYour opponents can't cast spells with the same name as the exiled card.\nWhen this enchantment leaves the battlefield, return the exiled card to its owner's hand."
)]
#[case::wormfang_behemoth(
    "Wormfang Behemoth",
    "77a000de-76d9-44f8-9789-875aa9f576e7",
    "Creature — Nightmare Fish Beast",
    "When this creature enters, exile all cards from your hand.\nWhen this creature leaves the battlefield, return the exiled cards to their owner's hand."
)]
fn the_enter_and_leave_triggers_are_two_linked_abilities_that_read_back_as_the_card(
    #[case] name: &str,
    #[case] oracle_id: &str,
    #[case] type_line: &str,
    #[case] text: &str,
) {
    let printed = card(name, oracle_id, type_line, text);
    let compiled = compile(&printed);
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_eq!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    assert!(compiled.clauses.iter().all(|clause| clause.read));
    let clauses = text.lines().count();
    assert_eq!(
        made.faces[0].abilities.len(),
        clauses,
        "{:?}",
        made.faces[0]
    );
    assert!(
        exile_durations(&made.faces[0].abilities)
            .iter()
            .all(Option::is_none)
    );
    let differs = differing(made, &printed);
    assert!(differs.is_empty(), "{differs:?}");
    verified(made, &printed);
}

/// The compiled rules of `printed` pass the exact check whole: they read back as the card and are `Verified`.
fn verified(made: &mtg_cards::card::Card, printed: &PrintedCard) {
    let names = BTreeSet::from([printed.name.to_lowercase()]);
    let report = check_exact(
        &canonical(made),
        printed,
        &Library::default(),
        &names,
        &Caps::everything(),
    );
    assert!(
        report.verdict() == Verdict::Accepted && report.summary() == "ACCEPTED Verified",
        "{}",
        report.render(true)
    );
}

#[test]
fn an_exile_until_the_card_leaves_stays_one_exile_with_its_duration() {
    let printed = card(
        "Banisher Priest",
        "f9c1c6a3-4d8a-4b4e-8a0a-1f0b6f2b1c3e",
        "Creature — Human Cleric",
        "When this creature enters, exile target creature an opponent controls until this creature leaves the battlefield.",
    );
    let compiled = compile(&printed);
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_eq!(made.faces[0].abilities.len(), 1, "{:?}", made.faces[0]);
    assert_eq!(
        exile_durations(&made.faces[0].abilities),
        vec![Some(Duration::WhileSourceOnBattlefield)]
    );
    verified(made, &printed);
}

#[rstest]
#[case::no_exile_before_it(
    "Enchantment",
    "When this enchantment enters, draw a card.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::the_return_alone(
    "Enchantment",
    "When this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::an_exile_that_is_not_on_entering(
    "Enchantment",
    "Whenever you cast a spell, exile target creature.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::an_exile_that_already_lasts(
    "Enchantment",
    "When this enchantment enters, exile target creature until this enchantment leaves the battlefield.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
fn a_return_trigger_without_the_exile_of_the_card_stays_with_the_table(
    #[case] type_line: &str,
    #[case] text: &str,
) {
    let printed = card(
        "Test Warden",
        "4457ed35-7c10-48c8-9776-456485fdf070",
        type_line,
        text,
    );
    let compiled = compile(&printed);
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_ne!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    let returns = compiled
        .clauses
        .iter()
        .filter(|clause| clause.text.contains("return the exiled card"))
        .collect::<Vec<_>>();
    assert_eq!(returns.len(), 1, "{compiled:?}");
    assert!(!returns[0].read, "{compiled:?} ({RETURN})");
}

#[rstest]
#[case::battlefield(
    "the exiled card to the battlefield under its owner's control",
    r#"{"Battlefield":{}}"#
)]
#[case::battlefield_theirs(
    "the exiled cards to the battlefield under their owners' control",
    r#"{"Battlefield":{}}"#
)]
#[case::battlefield_tapped(
    "the exiled cards to the battlefield tapped under their owners' control",
    r#"{"Battlefield":{"tapped":true}}"#
)]
#[case::hand("the exiled card to its owner's hand", r#""Hand""#)]
#[case::hands("the exiled cards to their owners' hands", r#""Hand""#)]
#[case::graveyard("the exiled cards to their owner's graveyard", r#""Graveyard""#)]
fn the_return_moves_the_exiled_cards_where_the_wording_says(#[case] words: &str, #[case] to: &str) {
    let text = format!(
        "When this creature enters, exile target creature.\nWhen this creature leaves the battlefield, return {words}."
    );
    let printed = card(
        "Test Warden",
        "4457ed35-7c10-48c8-9776-456485fdf070",
        "Creature — Spirit",
        &text,
    );
    let compiled = compile(&printed);
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_eq!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    let Effect::Move {
        what, to: moved, ..
    } = &made.faces[0].abilities[1].effects()[0]
    else {
        panic!("{:?}", made.faces[0].abilities[1]);
    };
    assert_eq!(*moved, serde_json::from_str(to).unwrap());
    assert_eq!(
        serde_json::to_string(what).unwrap(),
        r#"{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]}"#
    );
    verified(made, &printed);
}

#[rstest]
#[case::namesakes_destroyed(
    "Sorcery",
    "Destroy target nonland permanent and all other permanents with the same name as that permanent."
)]
#[case::creature_namesakes_exiled(
    "Instant",
    "Exile target creature and all other creatures with the same name as that creature."
)]
#[case::up_to_three_from_one_graveyard(
    "Sorcery",
    "Exile up to three target cards from a single graveyard."
)]
#[case::two_creature_cards_from_one_graveyard(
    "Sorcery",
    "Exile two target creature cards from a single graveyard."
)]
#[case::several_from_graveyards("Sorcery", "Exile up to two target cards from graveyards.")]
#[case::hand_pick_in_three_sentences(
    "Sorcery",
    "Target opponent reveals their hand. You choose a nonland card from it. Exile that card."
)]
#[case::hand_pick_joined(
    "Sorcery",
    "Target player reveals their hand and you choose a creature card from it. Exile that card."
)]
#[case::battlefield_and_graveyards(
    "Sorcery",
    "Exile up to two target creatures from the battlefield and/or creature cards from graveyards."
)]
#[case::players_cannot_cast_the_name(
    "Enchantment",
    "When this enchantment enters, exile target nonland permanent.\nPlayers can't cast spells with the same name as the exiled card."
)]
#[case::a_card_exiled_with_it(
    "Enchantment",
    "When this enchantment enters, exile target creature an opponent controls until this enchantment leaves the battlefield.\nYour opponents can't cast spells with the same name as a card exiled with this enchantment."
)]
fn the_linked_wordings_read_whole_and_back_as_printed(#[case] type_line: &str, #[case] text: &str) {
    let printed = card(
        "Test Warden",
        "4457ed35-7c10-48c8-9776-456485fdf070",
        type_line,
        text,
    );
    let compiled = compile(&printed);
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_eq!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    let differs = differing(made, &printed);
    assert!(differs.is_empty(), "{differs:?}");
    verified(made, &printed);
}

#[rstest]
#[case::namesakes_of_another_kind(
    "Destroy target nonland permanent and all other creatures with the same name as that permanent."
)]
#[case::namesakes_of_several_targets(
    "Destroy two target creatures and all other creatures with the same name as that creature."
)]
#[case::a_single_library("Exile up to two target cards from a single library.")]
#[case::one_from_a_single_graveyard("Exile target card from a single graveyard.")]
#[case::the_battlefield_alone(
    "Exile up to two target creatures from the battlefield and/or creatures."
)]
#[case::the_hand_pick_misworded(
    "Target opponent reveals their hand and you choose a nonland card from it. Exile this card."
)]
fn a_near_miss_of_the_linked_wordings_stays_with_the_table(#[case] text: &str) {
    let printed = card(
        "Test Warden",
        "4457ed35-7c10-48c8-9776-456485fdf070",
        "Sorcery",
        text,
    );
    let compiled = compile(&printed);
    assert!(
        compiled.clauses.iter().any(|clause| !clause.read),
        "{compiled:?}"
    );
}
