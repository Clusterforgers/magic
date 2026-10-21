//! What the engine runs, in the vocabulary of the card DSL's support records.
//!
//! `mtg_cards` computes whether a card is `Verified`, `Partial` or `Manual` from its rules and a
//! [`Caps`]; this is the one the running engine supplies (`docs/cards-dsl.md`, Support records). The engine
//! runs vanilla permanents, the plain `{T}: Add {X}` of a creature or land, spells, abilities that cost `{T}`, mana
//! and moving their source (sacrificed, exiled or discarded), abilities that work from a hand, a graveyard, exile, the
//! command zone or the stack, state triggers, and triggered abilities made of drawing, discarding, milling, sacrificing, destroying, exiling, countering,
//! scrying, looking at, revealing, searching for, moving and shuffling cards, tapping and untapping, winning the
//! game, life and damage (and delayed triggers), conditions, "may" and "pay" and choices and modal spells, continuous
//! effects and static abilities of every modification (types, subtypes, colours, power and toughness, keywords, abilities,
//! control, attack and block restrictions, characteristic-defining abilities and the power and toughness they define), the
//! rules of the game as effects and as static abilities (attack and block requirements, spells that can't be countered,
//! players who can't gain life, draw or search, a revealed library top, triggers that trigger again), the keywords of
//! combat, flash, hexproof, enchant, equip, reconfigure, kicker and multikicker, flashback, evoke and overload, permissions to cast (from another zone, without paying, as though with flash), attaching and unattaching, and the keywords that are triggers (ward, prowess, battle cry), and nothing else.

use mtg_cards::Caps;
use mtg_cards::assemble::{PLAIN_MANA_ABILITY, VARIABLE_STATS, X_COST};

/// The primitives the engine runs, and the only list of them. A task that teaches the engine one adds its
/// name here in the same change as the code: the test of the interpreter fails when a name is listed for an
/// effect that is still a stub, and when an effect that runs is not listed.
const RUNS: &[&str] = &[
    PLAIN_MANA_ABILITY,
    "Ability.Spell",
    "Ability.Activated",
    "Ability.Triggered",
    "Ability.Loyalty",
    "Ability.Limit",
    "Ability.From",
    "Ability.Sorcery",
    "Ability.Condition",
    "Trigger.ZoneChange",
    "Trigger.State",
    "Trigger.Attacks",
    "Trigger.SpellCast",
    "Trigger.Sacrificed",
    "Trigger.StepBegins",
    "Trigger.BecomesTarget",
    "Trigger.AnyOf",
    "Cost.Tap",
    "Cost.Mana",
    "Cost.Mana.X",
    "Cost.Loyalty.X",
    X_COST,
    "Cost.SacrificeSelf",
    "Cost.ExileSelf",
    "Cost.DiscardSelf",
    "Effect.Draw",
    "Effect.Discard",
    "Effect.Mill",
    "Effect.Scry",
    "Effect.LookAtTop",
    "Effect.Reveal",
    "Effect.Search",
    "Effect.Shuffle",
    "Effect.Move",
    "Effect.Move.Battlefield",
    "Effect.Destroy",
    "Effect.Exile",
    "Effect.Sacrifice",
    "Effect.Counter",
    "Effect.DealDamage",
    "Effect.GainLife",
    "Effect.LoseLife",
    "Effect.AddMana",
    "Effect.PutCounters",
    "Effect.RemoveCounters",
    "Effect.MoveCounters",
    "Effect.Tap",
    "Effect.Untap",
    "Effect.Attach",
    "Effect.Unattach",
    "Effect.WinGame",
    "Effect.CreateToken",
    "Effect.Continuous",
    "Effect.Rule",
    "Effect.Regenerate",
    "Effect.Shield",
    "Ability.Static",
    "Ability.Static.Defining",
    "Ability.Rule",
    "Rule.MustAttack",
    "Rule.MustBlock",
    "Rule.CantBeCountered",
    "Rule.CantGainLife",
    "Rule.CantDraw",
    "Rule.CantSearch",
    "Rule.RevealTop",
    "Rule.TriggerAgain",
    "Rule.DamageCantBePrevented",
    VARIABLE_STATS,
    "Mod.ModifyPT",
    "Mod.AddKeyword",
    "Mod.AddProtectionFromChosenColor",
    "Mod.SetPT",
    "Mod.SwitchPT",
    "Mod.AddType",
    "Mod.RemoveType",
    "Mod.AddSubtype",
    "Mod.SetColors",
    "Mod.RemoveKeyword",
    "Mod.GrantAbility",
    "Mod.LoseAbilities",
    "Mod.SetController",
    "Mod.CantAttack",
    "Mod.CantBlock",
    "Effect.DelayedTrigger",
    "Effect.If",
    "Effect.May",
    "Effect.Pay",
    "Effect.Choose",
    "Effect.Modal",
    "Keyword.Flash",
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
    "Keyword.Hexproof",
    "Keyword.Shroud",
    "Keyword.Protection",
    "Keyword.Prowess",
    "Keyword.BattleCry",
    "Keyword.Ward",
    "Keyword.Enchant",
    "Keyword.Equip",
    "Keyword.Reconfigure",
    "Keyword.LivingWeapon",
    "Keyword.Kicker",
    "Keyword.Multikicker",
    "Keyword.Flashback",
    "Keyword.Evoke",
    "Keyword.Overload",
    "Condition.CastWith",
    "Rule.MayCast",
    "Condition.Kicked",
    "Amount.Kicks",
    "Ability.Replacement",
    "ReplaceWhen.TokenCreation",
    "ReplaceWhen.Damage",
    "ReplaceWhen.ZoneChange",
    "ReplaceWhen.CountersPut",
    "ReplaceWhen.LifeGain",
    "ReplaceWhen.Draw",
    "ReplaceWhen.Enters",
    "ReplaceBy.Multiply",
    "ReplaceBy.Add",
    "ReplaceBy.Prevent",
    "ReplaceBy.PreventUpTo",
    "ReplaceBy.MoveTo",
    "ReplaceBy.Tapped",
    "ReplaceBy.WithCounters",
    "ReplaceBy.Choose",
    "ReplaceBy.PayLifeOrTapped",
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
    #[case::discarding("Effect.Discard", true)]
    #[case::discarding_at_random("Effect.Discard.Random", false)]
    #[case::milling("Effect.Mill", true)]
    #[case::scrying("Effect.Scry", true)]
    #[case::looking_at_the_top("Effect.LookAtTop", true)]
    #[case::revealing("Effect.Reveal", true)]
    #[case::shuffling("Effect.Shuffle", true)]
    #[case::moving("Effect.Move", true)]
    #[case::moving_to_the_battlefield("Effect.Move.Battlefield", true)]
    #[case::moving_in_a_random_order("Effect.Move.Random", false)]
    #[case::searching("Effect.Search", true)]
    #[case::sacrificing("Effect.Sacrifice", true)]
    #[case::damage("Effect.DealDamage", true)]
    #[case::gaining_life("Effect.GainLife", true)]
    #[case::losing_life("Effect.LoseLife", true)]
    #[case::mana_costs("Cost.Mana", true)]
    #[case::x_in_a_mana_cost("Cost.Mana.X", true)]
    #[case::paying_life("Cost.PayLife", false)]
    #[case::once_a_turn("Ability.Limit", true)]
    #[case::other_zones("Ability.From", true)]
    #[case::sorcery_timing("Ability.Sorcery", true)]
    #[case::activation_conditions("Ability.Condition", true)]
    #[case::equip("Keyword.Equip", true)]
    #[case::reconfigure("Keyword.Reconfigure", true)]
    #[case::living_weapon("Keyword.LivingWeapon", true)]
    #[case::unattaching("Effect.Unattach", true)]
    #[case::replacement_effects("Ability.Replacement", true)]
    #[case::replacing_token_creation("ReplaceWhen.TokenCreation", true)]
    #[case::replacing_damage("ReplaceWhen.Damage", true)]
    #[case::replacing_zone_changes("ReplaceWhen.ZoneChange", true)]
    #[case::replacing_counters_put("ReplaceWhen.CountersPut", true)]
    #[case::replacing_life_gain("ReplaceWhen.LifeGain", true)]
    #[case::replacing_draws("ReplaceWhen.Draw", true)]
    #[case::replacing_entering("ReplaceWhen.Enters", true)]
    #[case::multiplying("ReplaceBy.Multiply", true)]
    #[case::adding("ReplaceBy.Add", true)]
    #[case::moving_instead("ReplaceBy.MoveTo", true)]
    #[case::entering_tapped("ReplaceBy.Tapped", true)]
    #[case::entering_with_counters("ReplaceBy.WithCounters", true)]
    #[case::choosing_as_it_enters("ReplaceBy.Choose", true)]
    #[case::paying_life_or_entering_tapped("ReplaceBy.PayLifeOrTapped", true)]
    #[case::doing_something_instead("ReplaceBy.Instead", false)]
    #[case::mana_abilities_from_other_zones("Ability.ManaAbility.From", false)]
    #[case::states("Trigger.State", true)]
    #[case::exiling_the_source("Cost.ExileSelf", true)]
    #[case::discarding_the_source("Cost.DiscardSelf", true)]
    #[case::triggers("Ability.Triggered", true)]
    #[case::zone_changes("Trigger.ZoneChange", true)]
    #[case::attacks("Trigger.Attacks", true)]
    #[case::casting("Trigger.SpellCast", true)]
    #[case::sacrifices("Trigger.Sacrificed", true)]
    #[case::steps("Trigger.StepBegins", true)]
    #[case::becoming_a_target("Trigger.BecomesTarget", true)]
    #[case::either_of_two_events("Trigger.AnyOf", true)]
    #[case::delayed_triggers("Effect.DelayedTrigger", true)]
    #[case::destroying("Effect.Destroy", true)]
    #[case::exiling("Effect.Exile", true)]
    #[case::countering("Effect.Counter", true)]
    #[case::continuous_effects("Effect.Continuous", true)]
    #[case::pumps("Mod.ModifyPT", true)]
    #[case::granted_keywords("Mod.AddKeyword", true)]
    #[case::protection_from_a_chosen_colour("Mod.AddProtectionFromChosenColor", true)]
    #[case::shroud("Keyword.Shroud", true)]
    #[case::protection("Keyword.Protection", true)]
    #[case::setting_power_and_toughness("Mod.SetPT", true)]
    #[case::switching_power_and_toughness("Mod.SwitchPT", true)]
    #[case::types("Mod.AddType", true)]
    #[case::removed_types("Mod.RemoveType", true)]
    #[case::subtypes("Mod.AddSubtype", true)]
    #[case::colours("Mod.SetColors", true)]
    #[case::granted_abilities("Mod.GrantAbility", true)]
    #[case::lost_abilities("Mod.LoseAbilities", true)]
    #[case::changes_of_control("Mod.SetController", true)]
    #[case::attack_restrictions("Mod.CantAttack", true)]
    #[case::block_restrictions("Mod.CantBlock", true)]
    #[case::static_abilities("Ability.Static", true)]
    #[case::characteristic_defining_abilities("Ability.Static.Defining", true)]
    #[case::rules_as_effects("Effect.Rule", true)]
    #[case::rules_as_static_abilities("Ability.Rule", true)]
    #[case::attack_requirements("Rule.MustAttack", true)]
    #[case::block_requirements("Rule.MustBlock", true)]
    #[case::spells_that_cant_be_countered("Rule.CantBeCountered", true)]
    #[case::no_life_gain("Rule.CantGainLife", true)]
    #[case::no_drawing("Rule.CantDraw", true)]
    #[case::no_searching("Rule.CantSearch", true)]
    #[case::a_revealed_top("Rule.RevealTop", true)]
    #[case::triggers_again("Rule.TriggerAgain", true)]
    #[case::damage_that_cant_be_prevented("Rule.DamageCantBePrevented", true)]
    #[case::regenerating("Effect.Regenerate", true)]
    #[case::shields("Effect.Shield", true)]
    #[case::preventing("ReplaceBy.Prevent", true)]
    #[case::preventing_in_part("ReplaceBy.PreventUpTo", true)]
    #[case::removed_keywords("Mod.RemoveKeyword", true)]
    #[case::conditions("Effect.If", true)]
    #[case::putting_counters("Effect.PutCounters", true)]
    #[case::removing_counters("Effect.RemoveCounters", true)]
    #[case::moving_counters("Effect.MoveCounters", true)]
    #[case::divided_amounts("Amount.Divided", false)]
    #[case::even_shares("Amount.EvenShare", false)]
    #[case::loyalty_abilities("Ability.Loyalty", true)]
    #[case::loyalty_costs_with_an_x("Cost.Loyalty.X", true)]
    #[case::being_kicked("Condition.Kicked", true)]
    #[case::the_number_of_kicks("Amount.Kicks", true)]
    #[case::may("Effect.May", true)]
    #[case::choosing("Effect.Choose", true)]
    #[case::modes("Effect.Modal", true)]
    #[case::adding_mana("Effect.AddMana", true)]
    #[case::tapping_permanents("Effect.Tap", true)]
    #[case::untapping_permanents("Effect.Untap", true)]
    #[case::attaching("Effect.Attach", true)]
    #[case::enchanting("Keyword.Enchant", true)]
    #[case::enchanting_cards_elsewhere("Keyword.Enchant.Elsewhere", false)]
    #[case::winning_the_game("Effect.WinGame", true)]
    #[case::creating_tokens("Effect.CreateToken", true)]
    #[case::creating_copies("Effect.CreateToken.Copy", false)]
    #[case::sacrificing_the_source("Cost.SacrificeSelf", true)]
    #[case::sacrificing_a_permanent("Cost.Sacrifice", false)]
    #[case::paying_by_sacrificing_the_source("Effect.Pay.SacrificeSelf", false)]
    #[case::paying_by_exiling_the_source("Effect.Pay.ExileSelf", false)]
    #[case::paying_by_discarding_the_source("Effect.Pay.DiscardSelf", false)]
    #[case::mana_abilities_of_other_shapes("Ability.ManaAbility.Complex", false)]
    #[case::paying_or_not("Effect.Pay", true)]
    #[case::flash("Keyword.Flash", true)]
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
    #[case::hexproof("Keyword.Hexproof", true)]
    #[case::prowess("Keyword.Prowess", true)]
    #[case::battle_cry("Keyword.BattleCry", true)]
    #[case::ward("Keyword.Ward", true)]
    #[case::kicker("Keyword.Kicker", true)]
    #[case::multikicker("Keyword.Multikicker", true)]
    #[case::surveilling("Effect.Surveil", false)]
    #[case::proliferating("Effect.Proliferate", false)]
    #[case::goading("Effect.Goad", false)]
    #[case::fighting("Effect.Fight", false)]
    #[case::becoming_the_monarch("Effect.BecomeMonarch", false)]
    #[case::amassing("Effect.Amass", false)]
    #[case::monstrosity("Effect.Monstrosity", false)]
    #[case::adapting("Effect.Adapt", false)]
    #[case::bolstering("Effect.Bolster", false)]
    #[case::exploring("Effect.Explore", false)]
    #[case::conniving("Effect.Connive", false)]
    #[case::discovering("Effect.Discover", false)]
    #[case::time_travelling("Effect.TimeTravel", false)]
    #[case::extra_turns("Effect.ExtraTurn", false)]
    #[case::a_choice_of_mana("Effect.AddMana.OneOf", false)]
    #[case::mana_of_the_chosen_color("Effect.AddMana.ChosenColor", false)]
    #[case::mana_of_the_commanders_identity("Effect.AddMana.CommanderIdentity", false)]
    #[case::mana_a_land_could_produce("Effect.AddMana.ProducedBy", false)]
    #[case::devotion("Amount.Devotion", false)]
    #[case::the_greatest_value("Amount.Greatest", false)]
    #[case::totals("Amount.Total", false)]
    #[case::card_types_among_cards("Amount.CardTypes", false)]
    #[case::paying_energy("Cost.PayEnergy", false)]
    #[case::removing_counters_as_a_cost("Cost.RemoveCounters", false)]
    #[case::tapping_permanents_as_a_cost("Cost.TapPermanents", false)]
    #[case::exiling_from_a_graveyard_as_a_cost("Cost.ExileFromGraveyard", false)]
    #[case::returning_to_a_hand_as_a_cost("Cost.ReturnToHand", false)]
    #[case::dealing_damage_triggers("Trigger.DealsDamage", false)]
    #[case::life_gain_triggers("Trigger.LifeGained", false)]
    #[case::life_loss_triggers("Trigger.LifeLost", false)]
    #[case::draw_triggers("Trigger.Drew", false)]
    #[case::discard_triggers("Trigger.Discarded", false)]
    #[case::tap_triggers("Trigger.Tapped", false)]
    #[case::mana_triggers("Trigger.TappedForMana", false)]
    #[case::block_triggers("Trigger.Blocks", false)]
    #[case::blocked_triggers("Trigger.BecomesBlocked", false)]
    #[case::group_attack_triggers("Trigger.GroupAttacks", false)]
    #[case::counter_triggers("Trigger.CountersPut", false)]
    #[case::if_you_do("Condition.Performed", false)]
    #[case::the_monarch("Condition.Monarch", false)]
    #[case::the_citys_blessing("Condition.CitysBlessing", false)]
    #[case::x_costs(X_COST, true)]
    #[case::x_read_after_resolving("Amount.X.Elsewhere", false)]
    #[case::paying_x_in_an_effect("Effect.Pay.X", false)]
    #[case::faces(FACES, false)]
    #[case::variable_stats(VARIABLE_STATS, true)]
    fn the_engine_runs_what_it_runs_today(#[case] name: &str, #[case] runs: bool) {
        assert_eq!(caps().runs(name), runs);
    }
}
