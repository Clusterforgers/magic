//! Goal: a Contraption's crank ability reads as the `P170bCranked` trigger over any effect the reader knows, "assemble a Contraption" and "assemble two Contraptions" read as `P170bAssemble` with their count, every such clause writes back to its own words, and a near miss stays unread.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::{compile, render_variants};
use mtg_cards::effect::Effect;
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use rstest::rstest;

fn card(type_line: &str, text: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
        name: "Gizmo".into(),
        faces: vec![PrintedFace {
            name: "Gizmo".into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// The abilities of a card whose every clause reads, else `None`.
fn read_whole(type_line: &str, text: &str) -> Option<Vec<Ability>> {
    let compiled = compile(&card(type_line, text));
    compiled
        .clauses
        .iter()
        .all(|clause| clause.read)
        .then_some(())?;
    Some(compiled.card?.faces.into_iter().next()?.abilities)
}

const CONTRAPTION: &str = "Artifact \u{2014} Contraption";

#[rstest]
#[case::tokens(
    "Whenever you crank this Contraption, create two 1/1 colorless Gnome artifact creature tokens."
)]
#[case::life("Whenever you crank this Contraption, target player loses 2 life.")]
#[case::pump("Whenever you crank this Contraption, target creature gets +3/+3 until end of turn.")]
#[case::destroy("Whenever you crank this Contraption, destroy target artifact.")]
#[case::discard("Whenever you crank this Contraption, target player discards a card.")]
#[case::counters("Whenever you crank this Contraption, put two +1/+1 counters on target creature.")]
fn a_crank_ability_reads_as_the_crank_trigger(#[case] text: &str) {
    let abilities = read_whole(CONTRAPTION, text).unwrap_or_else(|| panic!("{text}"));
    let [ability] = &abilities[..] else {
        panic!("{abilities:?}");
    };
    let Ability::Triggered { when, .. } = ability else {
        panic!("{ability:?}");
    };
    assert_eq!(*when, TriggerPattern::P170bCranked {});
    let texts = render_variants(ability);
    assert!(texts.iter().any(|written| written == text), "{texts:?}");
}

#[rstest]
#[case::one("Assemble a Contraption.", 1)]
#[case::two("Assemble two Contraptions.", 2)]
#[case::three("Assemble three Contraptions.", 3)]
fn an_assembly_reads_with_its_count(#[case] text: &str, #[case] count: i32) {
    let abilities = read_whole("Sorcery", text).unwrap_or_else(|| panic!("{text}"));
    let found = format!("{abilities:?}");
    let wanted = format!(
        "{:?}",
        Effect::P170bAssemble {
            who: mtg_cards::refs::Ref::You,
            count: Amount::Fixed(count),
        }
    );
    assert!(found.contains(&wanted), "{found}");
    let texts: Vec<String> = abilities.iter().flat_map(render_variants).collect();
    assert!(texts.iter().any(|written| written == text), "{texts:?}");
}

#[rstest]
#[case::another_object("Whenever you crank target Contraption, draw a card.")]
#[case::an_opponent("Whenever an opponent cranks this Contraption, draw a card.")]
#[case::no_count("Assemble Contraptions.")]
#[case::wrong_noun("Assemble two Attractions.")]
fn a_near_miss_stays_unread(#[case] text: &str) {
    assert!(read_whole(CONTRAPTION, text).is_none(), "{text}");
}
