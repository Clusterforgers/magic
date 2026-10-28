//! Goal: probe.
use super::*;

#[test]
fn w6e_probe() {
    for (text, kind) in [
        (
            "Gain control of up to one target creature an opponent controls until end of turn. Untap those creatures. They gain haste until end of turn.",
            ClauseKind::Spell,
        ),
        (
            "{T}: Exchange control of two target nonlegendary creatures.",
            ClauseKind::Activated,
        ),
        (
            "{T}: Draw a card. You can't activate this ability during combat.",
            ClauseKind::Activated,
        ),
        (
            "For each opponent, gain control of up to one target creature that player controls until end of turn.",
            ClauseKind::Spell,
        ),
        (
            "Exchange control of two target creatures.",
            ClauseKind::Spell,
        ),
        (
            "At the beginning of your upkeep, gain control of enchanted creature until end of turn.",
            ClauseKind::Triggered,
        ),
        (
            "At the beginning of your upkeep, gain control of enchanted creature until end of turn. Untap that creature. It gains haste until end of turn.",
            ClauseKind::Triggered,
        ),
        (
            "Gain control of up to one target creature an opponent controls until end of turn. Untap that creature. It gains haste until end of turn.",
            ClauseKind::Spell,
        ),
        (
            "Gain control of target creature until end of turn.",
            ClauseKind::Spell,
        ),
        (
            "Gain control of target creature with power 2 or less.",
            ClauseKind::Spell,
        ),
        (
            "Untap target permanent and gain control of it until end of turn. It gains haste until end of turn.",
            ClauseKind::Spell,
        ),
        (
            "Gain control of target creature until end of turn. Untap that creature. It gains haste until end of turn.",
            ClauseKind::Spell,
        ),
        (
            "For each opponent, gain control of up to one target creature that player controls until end of turn. Untap those creatures. They gain haste until end of turn.",
            ClauseKind::Spell,
        ),
        (
            "For each opponent, gain control of target permanent that player controls.",
            ClauseKind::Spell,
        ),
        (
            "When this Aura enters and at the beginning of your upkeep, gain control of enchanted permanent until end of turn. Untap that permanent. It gains haste until end of turn.",
            ClauseKind::Triggered,
        ),
        (
            "{T}: Exchange control of two target nonlegendary creatures. You can't activate this ability during combat.",
            ClauseKind::Activated,
        ),
        (
            "When this creature enters, exchange control of target land you control and target land an opponent controls.",
            ClauseKind::Triggered,
        ),
        (
            "{T}: Each player gains control of all creatures they own.",
            ClauseKind::Activated,
        ),
        (
            "When Varchild leaves the battlefield, gain control of all Survivors.",
            ClauseKind::Triggered,
        ),
        (
            "Whenever this creature deals combat damage to a player, gain control of all artifacts that player controls.",
            ClauseKind::Triggered,
        ),
        (
            "{2}: Target player other than this artifact's owner gains control of it. Activate only during your turn.",
            ClauseKind::Activated,
        ),
        (
            "{U}{R}{W}: Target opponent gains control of target permanent you control.",
            ClauseKind::Activated,
        ),
        ("Gain control of all artifacts.", ClauseKind::Spell),
        (
            "When this creature enters, an opponent gains control of it.",
            ClauseKind::Triggered,
        ),
    ] {
        let tokens = super::super::tokens::tokenize(text);
        let mut r = super::super::tokens::Reader::new(&tokens, false);
        r.ability = !matches!(kind, ClauseKind::Spell);
        let raw = match kind {
            ClauseKind::Spell => super::super::abilities::spell(&mut r),
            ClauseKind::Triggered => super::super::abilities::triggered(&mut r),
            _ => super::super::abilities::activated(&mut r),
        };
        eprintln!(
            "  raw end={} {:?}",
            r.end(),
            raw.as_ref().map(|a| serde_json::to_string(a).unwrap())
        );
        if let Some(a) = raw {
            eprintln!("  written {:?}", render_variants(&a));
        }
        let read = clause(text, kind, false);
        eprintln!(
            "W6E {text}\n  => {}",
            read.map(|a| serde_json::to_string(&a).unwrap())
                .unwrap_or("NONE".into())
        );
    }
}
