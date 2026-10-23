//! Goal: "This ability triggers only once each turn." after the effects of a triggered ability reads as its once-a-turn limit, is written back, and is read nowhere else.
use super::*;

const ONCE: &str = "This ability triggers only once each turn.";

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

/// The ability with the limit set.
fn limited(ability: Ability) -> Ability {
    match ability {
        Ability::Triggered {
            when,
            intervening_if,
            from,
            targets,
            effects,
            ..
        } => Ability::Triggered {
            when,
            intervening_if,
            limit: Some(crate::ability::Limit::OncePerTurn),
            from,
            targets,
            effects,
        },
        other => other,
    }
}

#[rstest]
#[case::creatures_die("Whenever another creature you control dies, draw a card.")]
#[case::a_cast(
    "Whenever you cast a noncreature spell, create a 1/1 white Spirit creature token with flying."
)]
#[case::an_attack("Whenever ~ attacks, draw a card.")]
#[case::life_gained("Whenever you gain life, draw a card.")]
#[case::counters("Whenever one or more +1/+1 counters are put on this creature, draw a card.")]
#[case::two_sentences("Whenever ~ attacks, draw a card. You lose 1 life.")]
fn the_sentence_sets_the_limit(#[case] base: &str) {
    let plain = golden(base, ClauseKind::Triggered, false);
    let text = format!("{base} {ONCE}");
    assert_eq!(
        golden(&text, ClauseKind::Triggered, false),
        limited(plain),
        "{text}"
    );
}

#[test]
fn the_limit_is_in_the_data() {
    reads_as(
        &format!("Whenever you gain life, draw a card. {ONCE}"),
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#,
    );
}

#[rstest]
#[case::another_sentence_after_it(format!("Whenever ~ attacks, draw a card. {ONCE} You lose 1 life."))]
#[case::twice("Whenever ~ attacks, draw a card. This ability triggers only twice each turn.".to_owned())]
#[case::in_the_middle(format!("Whenever ~ attacks, {ONCE} Draw a card."))]
#[case::alone(ONCE.to_owned())]
fn near_misses_stay_with_the_table(#[case] text: String) {
    assert_eq!(trigger(&text), None, "{text}");
}

#[test]
fn an_activated_ability_has_no_such_sentence() {
    let text = format!("{{T}}: Draw a card. {ONCE}");
    assert_eq!(clause(&text, ClauseKind::Activated, false), None);
}
