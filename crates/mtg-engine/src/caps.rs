//! What the engine runs, in the vocabulary of the card DSL's support records.
//!
//! `mtg_cards` computes whether a card is `Verified`, `Partial` or `Manual` from its rules and a
//! [`Caps`]; this is the one the running engine supplies (`docs/cards-dsl.md`, Support records). The engine
//! runs vanilla permanents, the plain `{T}: Add {X}` of a creature or land, spells and `{T}` abilities made of
//! drawing, life and damage, the keywords of combat, and nothing else.

use mtg_cards::Caps;
use mtg_cards::assemble::PLAIN_MANA_ABILITY;

/// The primitives the engine runs, and the only list of them. A task that teaches the engine one adds its
/// name here in the same change as the code: the test of the interpreter fails when a name is listed for an
/// effect that is still a stub, and when an effect that runs is not listed.
const RUNS: &[&str] = &[
    PLAIN_MANA_ABILITY,
    "Ability.Spell",
    "Ability.Activated",
    "Cost.Tap",
    "Effect.Draw",
    "Effect.DealDamage",
    "Effect.GainLife",
    "Effect.LoseLife",
    "Keyword.Flying",
    "Keyword.Reach",
    "Keyword.Menace",
    "Keyword.Defender",
    "Keyword.Haste",
    "Keyword.Vigilance",
    "Keyword.FirstStrike",
    "Keyword.DoubleStrike",
    "Keyword.Deathtouch",
    "Keyword.Trample",
    "Keyword.Lifelink",
    "Keyword.Indestructible",
];

/// The capabilities of this engine.
pub fn caps() -> Caps {
    Caps::from_names(RUNS.iter().copied())
}

#[cfg(test)]
mod tests {
    //! Goal: every ability of the embedded rules that the engine's capabilities cover is carried into the definition, so a Verified card never loses an ability on its way into the game, and the capabilities pin what the engine runs today.
    use super::*;
    use mtg_cards::assemble::{FACES, VARIABLE_STATS, X_COST, lower};
    use mtg_cards::builtin;
    use mtg_cards::validate::used_primitives;
    use rstest::rstest;

    #[test]
    fn what_the_capabilities_cover_is_carried() {
        let caps = caps();
        let library = builtin::library().unwrap_or_else(|errors| panic!("{errors}"));
        let mut covered = 0;
        for card in library.cards() {
            for (_, ability) in card.abilities() {
                if ability.is_manual() || !caps.missing(&used_primitives(ability)).is_empty() {
                    continue;
                }
                covered += 1;
                assert!(
                    lower(ability).is_some(),
                    "{}: the engine says it runs {:?} but the definition cannot carry it",
                    card.name,
                    used_primitives(ability)
                );
            }
        }
        assert!(covered > 0, "the corpus has a mana ability the engine runs");
    }

    #[rstest]
    #[case::plain_mana_ability(PLAIN_MANA_ABILITY, true)]
    #[case::spells("Ability.Spell", true)]
    #[case::activated_abilities("Ability.Activated", true)]
    #[case::tapping("Cost.Tap", true)]
    #[case::drawing("Effect.Draw", true)]
    #[case::damage("Effect.DealDamage", true)]
    #[case::gaining_life("Effect.GainLife", true)]
    #[case::losing_life("Effect.LoseLife", true)]
    #[case::mana_costs_of_abilities("Cost.Mana", false)]
    #[case::once_a_turn("Ability.Limit", false)]
    #[case::other_zones("Ability.From", false)]
    #[case::triggers("Ability.Triggered", false)]
    #[case::destroy("Effect.Destroy", false)]
    #[case::flying("Keyword.Flying", true)]
    #[case::reach("Keyword.Reach", true)]
    #[case::menace("Keyword.Menace", true)]
    #[case::defender("Keyword.Defender", true)]
    #[case::haste("Keyword.Haste", true)]
    #[case::vigilance("Keyword.Vigilance", true)]
    #[case::first_strike("Keyword.FirstStrike", true)]
    #[case::double_strike("Keyword.DoubleStrike", true)]
    #[case::deathtouch("Keyword.Deathtouch", true)]
    #[case::trample("Keyword.Trample", true)]
    #[case::lifelink("Keyword.Lifelink", true)]
    #[case::indestructible("Keyword.Indestructible", true)]
    #[case::hexproof("Keyword.Hexproof", false)]
    #[case::kicker("Keyword.Kicker", false)]
    #[case::x_costs(X_COST, false)]
    #[case::faces(FACES, false)]
    #[case::variable_stats(VARIABLE_STATS, false)]
    fn the_engine_runs_what_it_runs_today(#[case] name: &str, #[case] runs: bool) {
        assert_eq!(caps().runs(name), runs);
    }
}
