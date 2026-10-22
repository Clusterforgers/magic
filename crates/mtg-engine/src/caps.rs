//! What the engine runs, in the vocabulary of the card DSL's support records.
//!
//! `mtg_cards` computes whether a card is `Verified`, `Partial` or `Manual` from its rules and a
//! [`Caps`]; this is the one the running engine supplies (`docs/cards-dsl.md`, Support records). The engine
//! runs vanilla permanents, mana abilities that cost `{T}` and/or their source (plain or of several, counted or chosen mana, with effects that ask nothing), spells, abilities that cost `{T}`, mana
//! and moving their source (sacrificed, exiled or discarded), life, counters removed from the source and permanents and cards the player picks (sacrificed, discarded, exiled from a graveyard, tapped or returned to hand), abilities that work from a hand, a graveyard, exile, the
//! command zone or the stack, state triggers, and triggered abilities made of drawing, discarding, milling, sacrificing, destroying, exiling, countering,
//! scrying, looking at, revealing, searching for, moving and shuffling cards, tapping and untapping, winning the
//! game, life and damage (and delayed triggers), conditions, "may" and "pay" and choices and modal spells, continuous
//! effects and static abilities of every modification (types, subtypes, colours, power and toughness, keywords, abilities,
//! control, attack and block restrictions, characteristic-defining abilities and the power and toughness they define), the
//! rules of the game as effects and as static abilities (attack and block requirements, spells that can't be countered,
//! players who can't gain life, draw or search, a revealed library top, triggers that trigger again), the keywords of
//! combat, flash, hexproof, enchant, equip, reconfigure, kicker and multikicker, flashback, evoke and overload, cycling, permissions to cast (from another zone, without paying, as though with flash), cost modifiers, attaching and unattaching, and the keywords that are triggers (ward, prowess, battle cry, echo, cumulative upkeep, undying, persist), and nothing else.

use mtg_cards::Caps;
use mtg_cards::assemble::{FACES, PLAIN_MANA_ABILITY, VARIABLE_STATS, X_COST};

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
    "Trigger.AttacksPlayer",
    "Trigger.SpellCast",
    "Trigger.Sacrificed",
    "Trigger.StepBegins",
    "Trigger.BecomesTarget",
    "Trigger.AnyOf",
    "Trigger.DealsDamage",
    "Trigger.LifeGained",
    "Trigger.LifeLost",
    "Trigger.Drew",
    "Trigger.Discarded",
    "Trigger.Tapped",
    "Trigger.Untapped",
    "Trigger.TappedForMana",
    "Trigger.Blocks",
    "Trigger.BecomesBlocked",
    "Trigger.GroupAttacks",
    "Trigger.CountersPut",
    "Trigger.CountersRemoved",
    "Trigger.Attached",
    "Trigger.Cycled",
    "Trigger.BecomesMonarch",
    "Trigger.Batch",
    "Trigger.Nth",
    "Amount.Triggering",
    "Amount.Players",
    "Amount.LowestLife",
    "Amount.HighestLife",
    "Cost.Tap",
    "Cost.Mana",
    "Cost.Mana.X",
    "Cost.Loyalty.X",
    X_COST,
    FACES,
    "Cost.SacrificeSelf",
    "Cost.ExileSelf",
    "Cost.DiscardSelf",
    "Cost.PayLife",
    "Cost.PayEnergy",
    "Cost.Sacrifice",
    "Cost.Discard",
    "Cost.RemoveCounters",
    "Cost.ExileFromGraveyard",
    "Cost.TapPermanents",
    "Cost.ReturnToHand",
    "Cost.ExileFromHand",
    "Cost.TapPower",
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
    "Effect.ExileUntil",
    "Effect.ExileTop",
    "Effect.CastFromZone",
    "Effect.Destroy",
    "Effect.Exile",
    "Effect.Sacrifice",
    "Effect.Counter",
    "Effect.DealDamage",
    "Effect.GainLife",
    "Effect.LoseLife",
    "Effect.AddMana",
    "Effect.AddMana.OneOf",
    "Effect.AddMana.ChosenColor",
    "Effect.AddMana.Restricted",
    "Effect.AddMana.CommanderIdentity",
    "Effect.AddMana.ProducedBy",
    "Ability.ManaAbility.Complex",
    "Effect.PutCounters",
    "Effect.RemoveCounters",
    "Effect.MoveCounters",
    "Effect.Tap",
    "Effect.Untap",
    "Effect.Attach",
    "Effect.Unattach",
    "Effect.Transform",
    "Effect.WinGame",
    "Effect.Surveil",
    "Effect.BecomeMonarch",
    "Condition.Monarch",
    "Effect.CreateToken",
    "Effect.CreateToken.Copy",
    "Effect.CopySpell",
    "Effect.CopyAbility",
    "Effect.BecomeCopy",
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
    "Rule.NoMaxHandSize",
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
    "Mod.SetCreatureTypes",
    "Mod.AddAllCreatureTypes",
    "Mod.SetColors",
    "Mod.RemoveKeyword",
    "Mod.GrantAbility",
    "Mod.LoseAbilities",
    "Mod.SetController",
    "Mod.CantAttack",
    "Mod.CantBlock",
    "Mod.CantBeBlocked",
    "Mod.CantBeBlockedBy",
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
    "Keyword.EnchantPlayer",
    "Keyword.Equip",
    "Keyword.Reconfigure",
    "Keyword.LivingWeapon",
    "Keyword.Kicker",
    "Keyword.Multikicker",
    "Keyword.Buyback",
    "Keyword.Entwine",
    "Ability.AdditionalCost",
    "Condition.AdditionalCostPaid",
    "Keyword.Flashback",
    "Keyword.Evoke",
    "Keyword.Overload",
    "Keyword.Cycling",
    "Keyword.Cascade",
    "Keyword.Storm",
    "Keyword.Convoke",
    "Keyword.Delve",
    "Keyword.Madness",
    "Keyword.Suspend",
    "Keyword.Foretell",
    "Keyword.Daybound",
    "Keyword.Nightbound",
    "Keyword.Fuse",
    "Keyword.Crew",
    "Keyword.Changeling",
    "Keyword.Devoid",
    "Keyword.Infect",
    "Keyword.Wither",
    "Keyword.Toxic",
    "Keyword.Fear",
    "Keyword.Intimidate",
    "Keyword.Landwalk",
    "Keyword.Skulk",
    "Keyword.Shadow",
    "Condition.CastWith",
    "Rule.MayCast",
    "Condition.Kicked",
    "Condition.Performed",
    "Amount.Kicks",
    "Ability.Replacement",
    "Ability.CostModifier",
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
    "ReplaceBy.AsCopy",
    "Keyword.Partner",
    "Keyword.PartnerWith",
    "Keyword.PartnerGroup",
    "Keyword.DoctorsCompanion",
    "Keyword.ChooseABackground",
    "Keyword.Unearth",
    "Keyword.Escape",
    "Keyword.Rebound",
    "Keyword.Echo",
    "Keyword.CumulativeUpkeep",
    "Keyword.Undying",
    "Keyword.Persist",
    "Condition.NewlyControlled",
    "Rule.CanBeCommander",
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
    fn no_primitive_is_listed_twice() {
        // `Caps` keeps the names in a set, which would hide a repeated line.
        let mut seen = std::collections::BTreeSet::new();
        for name in RUNS {
            assert!(seen.insert(*name), "{name} is listed twice");
        }
    }

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
    #[case::moving_in_a_random_order_to_the_top("Effect.Move.Random.Top", false)]
    #[case::searching("Effect.Search", true)]
    #[case::sacrificing("Effect.Sacrifice", true)]
    #[case::damage("Effect.DealDamage", true)]
    #[case::gaining_life("Effect.GainLife", true)]
    #[case::losing_life("Effect.LoseLife", true)]
    #[case::mana_costs("Cost.Mana", true)]
    #[case::x_in_a_mana_cost("Cost.Mana.X", true)]
    #[case::paying_life("Cost.PayLife", true)]
    #[case::paying_a_counted_amount_of_life("Cost.PayLife.Counted", false)]
    #[case::once_a_turn("Ability.Limit", true)]
    #[case::other_zones("Ability.From", true)]
    #[case::sorcery_timing("Ability.Sorcery", true)]
    #[case::activation_conditions("Ability.Condition", true)]
    #[case::equip("Keyword.Equip", true)]
    #[case::reconfigure("Keyword.Reconfigure", true)]
    #[case::living_weapon("Keyword.LivingWeapon", true)]
    #[case::unattaching("Effect.Unattach", true)]
    #[case::transforming("Effect.Transform", true)]
    #[case::day_and_night("Keyword.Daybound", true)]
    #[case::the_night_face("Keyword.Nightbound", true)]
    #[case::fusing("Keyword.Fuse", true)]
    #[case::every_creature_type("Keyword.Changeling", true)]
    #[case::no_color("Keyword.Devoid", true)]
    #[case::poison("Keyword.Infect", true)]
    #[case::counters("Keyword.Wither", true)]
    #[case::toxic("Keyword.Toxic", true)]
    #[case::evasion_by_color("Keyword.Fear", true)]
    #[case::evasion_by_color_shared("Keyword.Intimidate", true)]
    #[case::evasion_by_land("Keyword.Landwalk", true)]
    #[case::evasion_by_power("Keyword.Skulk", true)]
    #[case::evasion_by_shadow("Keyword.Shadow", true)]
    #[case::all_creature_types("Mod.AddAllCreatureTypes", true)]
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
    #[case::no_maximum_hand_size("Rule.NoMaxHandSize", true)]
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
    #[case::divided_amounts_read_outside_damage_and_counters("Amount.Divided.Elsewhere", false)]
    #[case::even_shares_read_outside_damage_and_counters("Amount.EvenShare.Elsewhere", false)]
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
    #[case::creating_copies("Effect.CreateToken.Copy", true)]
    #[case::entering_as_a_copy("ReplaceBy.AsCopy", true)]
    #[case::an_exception_the_copy_cannot_hold("Copy.Except.SwitchPT", false)]
    #[case::copying_spells("Effect.CopySpell", true)]
    #[case::copying_abilities("Effect.CopyAbility", true)]
    #[case::becoming_a_copy("Effect.BecomeCopy", true)]
    #[case::a_copy_that_ends_with_another_duration("Copy.Duration.UntilYourNextTurn", false)]
    #[case::storming("Keyword.Storm", true)]
    #[case::sacrificing_the_source("Cost.SacrificeSelf", true)]
    #[case::sacrificing_a_permanent("Cost.Sacrifice", true)]
    #[case::paying_by_sacrificing_the_source("Effect.Pay.SacrificeSelf", false)]
    #[case::paying_by_exiling_the_source("Effect.Pay.ExileSelf", false)]
    #[case::paying_by_discarding_the_source("Effect.Pay.DiscardSelf", false)]
    #[case::mana_abilities_of_several_mana_or_with_more_effects(
        "Ability.ManaAbility.Complex",
        true
    )]
    #[case::mana_abilities_the_engine_does_not_activate_at_priority(
        "Ability.ManaAbility.Other",
        false
    )]
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
    #[case::surveilling("Effect.Surveil", true)]
    #[case::proliferating("Effect.Proliferate", false)]
    #[case::goading("Effect.Goad", false)]
    #[case::fighting("Effect.Fight", false)]
    #[case::becoming_the_monarch("Effect.BecomeMonarch", true)]
    #[case::amassing("Effect.Amass", false)]
    #[case::monstrosity("Effect.Monstrosity", false)]
    #[case::adapting("Effect.Adapt", false)]
    #[case::bolstering("Effect.Bolster", false)]
    #[case::exploring("Effect.Explore", false)]
    #[case::conniving("Effect.Connive", false)]
    #[case::exiling_until_a_card_matches("Effect.ExileUntil", true)]
    #[case::exiling_the_top_cards("Effect.ExileTop", true)]
    #[case::casting_during_a_resolution("Effect.CastFromZone", true)]
    #[case::cascading("Keyword.Cascade", true)]
    #[case::convoking("Keyword.Convoke", true)]
    #[case::delving("Keyword.Delve", true)]
    #[case::time_travelling("Effect.TimeTravel", false)]
    #[case::extra_turns("Effect.ExtraTurn", false)]
    #[case::a_choice_of_mana("Effect.AddMana.OneOf", true)]
    #[case::mana_of_the_chosen_color("Effect.AddMana.ChosenColor", true)]
    #[case::mana_restricted_to_what_it_may_be_spent_on("Effect.AddMana.Restricted", true)]
    #[case::mana_of_the_commanders_identity("Effect.AddMana.CommanderIdentity", true)]
    #[case::mana_a_land_could_produce("Effect.AddMana.ProducedBy", true)]
    #[case::devotion("Amount.Devotion", false)]
    #[case::the_greatest_value("Amount.Greatest", false)]
    #[case::totals("Amount.Total", false)]
    #[case::card_types_among_cards("Amount.CardTypes", false)]
    #[case::paying_energy("Cost.PayEnergy", true)]
    #[case::paying_a_counted_amount_of_energy("Cost.PayEnergy.Counted", false)]
    #[case::removing_counters_as_a_cost("Cost.RemoveCounters", true)]
    #[case::tapping_permanents_as_a_cost("Cost.TapPermanents", true)]
    #[case::exiling_from_a_graveyard_as_a_cost("Cost.ExileFromGraveyard", true)]
    #[case::returning_to_a_hand_as_a_cost("Cost.ReturnToHand", true)]
    #[case::dealing_damage_triggers("Trigger.DealsDamage", true)]
    #[case::life_gain_triggers("Trigger.LifeGained", true)]
    #[case::life_loss_triggers("Trigger.LifeLost", true)]
    #[case::draw_triggers("Trigger.Drew", true)]
    #[case::discard_triggers("Trigger.Discarded", true)]
    #[case::tap_triggers("Trigger.Tapped", true)]
    #[case::untap_triggers("Trigger.Untapped", true)]
    #[case::mana_triggers("Trigger.TappedForMana", true)]
    #[case::block_triggers("Trigger.Blocks", true)]
    #[case::blocked_triggers("Trigger.BecomesBlocked", true)]
    #[case::group_attack_triggers("Trigger.GroupAttacks", true)]
    #[case::counter_triggers("Trigger.CountersPut", true)]
    #[case::counter_removal_triggers("Trigger.CountersRemoved", true)]
    #[case::attach_triggers("Trigger.Attached", true)]
    #[case::cycle_triggers("Trigger.Cycled", true)]
    #[case::becoming_the_monarch_triggers("Trigger.BecomesMonarch", true)]
    #[case::batch_triggers("Trigger.Batch", true)]
    #[case::nth_triggers("Trigger.Nth", true)]
    #[case::event_amounts("Amount.Triggering", true)]
    #[case::if_you_do("Condition.Performed", true)]
    #[case::if_you_do_after_another_effect("Condition.Performed.Other", false)]
    #[case::the_monarch("Condition.Monarch", true)]
    #[case::the_citys_blessing("Condition.CitysBlessing", false)]
    #[case::unblockable_creatures("Mod.CantBeBlocked", true)]
    #[case::creatures_that_cant_be_blocked_by_some("Mod.CantBeBlockedBy", true)]
    #[case::looking_at_the_top_any_time("Rule.LookAtTopAnyTime", false)]
    #[case::additional_land_plays("Rule.AdditionalLandPlays", false)]
    #[case::commander_eligibility("Rule.CanBeCommander", true)]
    #[case::partner("Keyword.Partner", true)]
    #[case::partner_with("Keyword.PartnerWith", true)]
    #[case::partner_group("Keyword.PartnerGroup", true)]
    #[case::doctors_companion("Keyword.DoctorsCompanion", true)]
    #[case::choose_a_background("Keyword.ChooseABackground", true)]
    #[case::unearth("Keyword.Unearth", true)]
    #[case::escape("Keyword.Escape", true)]
    #[case::rebound("Keyword.Rebound", true)]
    #[case::echo("Keyword.Echo", true)]
    #[case::cumulative_upkeep("Keyword.CumulativeUpkeep", true)]
    #[case::undying("Keyword.Undying", true)]
    #[case::persist("Keyword.Persist", true)]
    #[case::coming_under_control_since_the_last_turn("Condition.NewlyControlled", true)]
    #[case::counting_players("Amount.Players", true)]
    #[case::the_lowest_life_total("Amount.LowestLife", true)]
    #[case::the_highest_life_total("Amount.HighestLife", true)]
    #[case::choosing_a_player_as_it_enters("ReplaceBy.Choose.Player", false)]
    #[case::choosing_an_opponent_as_it_enters("ReplaceBy.Choose.Opponent", false)]
    #[case::myriad("Keyword.Myriad", false)]
    #[case::crew("Keyword.Crew", true)]
    #[case::tapping_creatures_for_their_power_as_a_cost("Cost.TapPower", true)]
    #[case::mutate("Keyword.Mutate", false)]
    #[case::encore("Keyword.Encore", false)]
    #[case::squad("Keyword.Squad", false)]
    #[case::x_costs(X_COST, true)]
    #[case::x_read_after_resolving("Amount.X.Elsewhere", false)]
    #[case::paying_x_in_an_effect("Effect.Pay.X", false)]
    #[case::faces(FACES, true)]
    #[case::variable_stats(VARIABLE_STATS, true)]
    fn the_engine_runs_what_it_runs_today(#[case] name: &str, #[case] runs: bool) {
        assert_eq!(caps().runs(name), runs);
    }
}
