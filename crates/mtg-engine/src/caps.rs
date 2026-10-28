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
//! combat, flash, hexproof, enchant, equip, reconfigure, kicker and multikicker, flashback, evoke and overload, cycling, permissions to cast (from another zone, without paying, as though with flash), cost modifiers, attaching and unattaching, and the keywords that are triggers (ward, prowess, battle cry, echo, cumulative upkeep, undying, persist, exalted, melee, mentor, dethrone, annihilator, extort, evolve), and nothing else.

use mtg_cards::Caps;
use mtg_cards::assemble::{FACES, PLAIN_MANA_ABILITY, PREPARE, VARIABLE_STATS, X_COST};

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
    "Trigger.AttacksPlayerOrPlaneswalker",
    "Trigger.PlayerAttacked",
    "Trigger.PlayerAttacks",
    "Trigger.SpellCast",
    "Trigger.SpellCopied",
    "Trigger.Sacrificed",
    "Trigger.Milled",
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
    "Trigger.Unblocked",
    "Trigger.GroupAttacks",
    "Trigger.CountersPut",
    "Trigger.CountersRemoved",
    "Trigger.Chapter",
    "Trigger.Attached",
    "Trigger.Cycled",
    "Trigger.BecomesMonarch",
    "Trigger.Proliferated",
    "Trigger.CoinFlipWon",
    "Trigger.DiceRolled",
    "Trigger.Explored",
    "Trigger.Connived",
    "Trigger.Amassed",
    "Trigger.BecomesMonstrous",
    "Trigger.BecomesRenowned",
    "Trigger.Exploits",
    "Trigger.BecomesCrewed",
    "Trigger.BecomesSaddled",
    "Trigger.BecomesLevel",
    "Trigger.TurnedFaceUp",
    "Trigger.Mutates",
    "Trigger.Adapted",
    "Trigger.Bolstered",
    "Trigger.Surveilled",
    "Trigger.ZoneChangeExcept",
    "Trigger.Batch",
    "Trigger.Nth",
    "Amount.Triggering",
    "Amount.Players",
    "Amount.Greatest",
    "Amount.Total",
    "Amount.CardTypes",
    "Amount.Devotion",
    "Amount.LowestLife",
    "Amount.HighestLife",
    "Amount.LowestHand",
    "Amount.LifeLostThisWay",
    "Amount.Destroyed",
    "Amount.Distinct",
    "Amount.DieResult",
    "Cost.Tap",
    "Cost.Mana",
    "Cost.Mana.X",
    "Cost.Loyalty.X",
    X_COST,
    FACES,
    PREPARE,
    "Cost.SacrificeSelf",
    "Cost.ExileSelf",
    "Cost.DiscardSelf",
    "Cost.PayLife",
    "Cost.PayEnergy",
    "Cost.Mill",
    "Cost.Sacrifice",
    "Cost.Discard",
    "Cost.RemoveCounters",
    "Cost.ExileFromGraveyard",
    "Cost.TapPermanents",
    "Cost.ReturnToHand",
    "Cost.ExileFromHand",
    "Cost.RevealFromHand",
    "Cost.OneOf",
    "Cost.TapPower",
    "Cost.Changed",
    "Effect.Draw",
    "Effect.Discard",
    "Effect.Discard.Random",
    "Effect.Mill",
    "Effect.Scry",
    "Effect.LookAtTop",
    "Effect.Reveal",
    "Effect.Search",
    "Effect.Shuffle",
    "Effect.Move",
    "Effect.Move.Battlefield",
    "Effect.Move.Transformed",
    "Effect.ExileUntil",
    "Effect.RevealUntil",
    "Effect.ExileTop",
    "Effect.ExileHidden",
    "Effect.Encode",
    "Effect.CopyCard",
    "Effect.Manifest",
    "Effect.Cloak",
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
    "Effect.AddMana.OneOfLists",
    "Effect.AddMana.ChosenColor",
    "Effect.AddMana.OrChosenColor",
    "Effect.AddMana.Restricted",
    "Effect.AddMana.Lasts",
    "Effect.AddMana.CommanderIdentity",
    "Effect.AddMana.ProducedBy",
    "Effect.AddMana.OfTapped",
    "Effect.AddMana.AnyCombination",
    "Effect.AddMana.AnyColor",
    "Effect.AddMana.Instead",
    "Ability.ManaAbility.Complex",
    "Effect.PutCounters",
    "Effect.RemoveCounters",
    "Effect.MoveCounters",
    "Effect.MultiplyCounters",
    "Effect.Tap",
    "Effect.Untap",
    "Effect.TapOrUntap",
    "Effect.PhaseOut",
    "Effect.Attach",
    "Effect.Unattach",
    "Effect.JoinAttack",
    "Effect.Transform",
    "Effect.Flip",
    "Effect.Prepare",
    "Effect.WinGame",
    "Effect.ExtraTurn",
    "Effect.AddPhase",
    "Effect.Surveil",
    "Effect.Proliferate",
    "Effect.TimeTravel",
    "Effect.Goad",
    "Effect.Fight",
    "Effect.Explore",
    "Effect.Connive",
    "Effect.Amass",
    "Effect.Monstrosity",
    "Effect.Renown",
    "Effect.Exploit",
    "Effect.BecomeLevel",
    "Effect.Adapt",
    "Effect.Bolster",
    "Effect.Fabricate",
    "Effect.Learn",
    "Effect.Crewed",
    "Effect.BecomeMonarch",
    "Effect.CreateEmblem",
    "Condition.Monarch",
    "Condition.MonarchAtTurnStart",
    "Effect.BecomeDayNight",
    "Condition.IsDay",
    "Condition.IsNight",
    "Condition.NeitherDayNorNight",
    "Trigger.DayNightChanges",
    "ReplaceBy.DayIfNeither",
    "ReplaceBy.EntersPrepared",
    "Effect.CreateToken",
    "Effect.CreateToken.Copy",
    "Effect.CopySpell",
    "Effect.CopyAbility",
    "Effect.BecomeCopy",
    "Copy.Duration.UntilYourNextTurn",
    "Effect.Continuous",
    "Effect.ExchangeControl",
    "Duration.WhileYouControlSource",
    "Effect.Rule",
    "Effect.Regenerate",
    "Effect.Shield",
    "Ability.Static",
    "Ability.Static.Defining",
    "Ability.Static.SpellKeywords",
    "Ability.Rule",
    "Rule.MustAttack",
    "Rule.MustAttackPlayers",
    "Rule.AttackTax",
    "Rule.MustBlock",
    "Rule.CantBeCountered",
    "Rule.CantGainLife",
    "Rule.NoLifeLossFromDamage",
    "Rule.LifeTotalCantChange",
    "Rule.PlayersHave",
    "Rule.CantDraw",
    "Rule.CantSearch",
    "Rule.SkipExtraTurns",
    "Rule.RevealTop",
    "Rule.NoMaxHandSize",
    "Rule.SkipDrawStep",
    "Rule.LookAtTopAnyTime",
    "Rule.AdditionalLandPlays",
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
    "Mod.SetTypes",
    "Mod.RemoveSupertype",
    "Mod.AddSubtype",
    "Mod.SetCreatureTypes",
    "Mod.AddAllCreatureTypes",
    "Mod.SetColors",
    "Mod.RemoveKeyword",
    "Mod.GrantAbility",
    "Mod.LoseAbilities",
    "Mod.SetController",
    "Mod.CantAttack",
    "Mod.CantAttackYou",
    "Mod.CantBlock",
    "Mod.DoesntUntap",
    "Mod.CantBeBlocked",
    "Mod.CantBeBlockedBy",
    "Mod.CanBlockOnly",
    "Mod.MinBlockers",
    "Mod.MaxBlockers",
    "Mod.Goaded",
    "Effect.DelayedTrigger",
    "Effect.If",
    "Effect.May",
    "Effect.Pay",
    "Effect.Pay.Counted",
    "Effect.Pay.X",
    "Effect.Choose",
    "Effect.ChooseAtRandom",
    "Effect.Modal",
    "Effect.RepeatEach",
    "Effect.RollDice",
    "Target.PerPlayer",
    "Effect.FlipCoin",
    "Effect.Modal.AtRandom",
    "Effect.Modal.OnceEachTurn",
    "Effect.Modal.OnceEver",
    "Effect.Modal.AllIf",
    "Effect.Vote",
    "Effect.Planechase",
    "Effect.VillainousChoice",
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
    "Ability.FreeCast",
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
    "Keyword.Miracle",
    "Keyword.Dredge",
    "ReplaceBy.Dredge",
    "Keyword.Suspend",
    "Keyword.Foretell",
    "Keyword.Morph",
    "Keyword.Megamorph",
    "Keyword.Disguise",
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
    "Condition.ManaSpent",
    "Rule.MayCast",
    "Condition.Kicked",
    "Condition.KickedWith",
    "Condition.Performed",
    "Amount.Kicks",
    "Amount.Mutations",
    "Amount.Promised",
    "Amount.CastWith",
    "Amount.ColorsSpent",
    "Amount.PermanentsGraved",
    "Amount.CommanderCasts",
    "Amount.Tallied",
    "Amount.Largest",
    "Amount.StartingLife",
    "Amount.Party",
    "Ability.Replacement",
    "Ability.CostModifier",
    "ReplaceWhen.TokenCreation",
    "ReplaceWhen.Damage",
    "ReplaceWhen.ZoneChange",
    "ReplaceWhen.CountersPut",
    "ReplaceWhen.LifeGain",
    "ReplaceWhen.Draw",
    "ReplaceWhen.Draw.ExceptDrawStep",
    "ReplaceWhen.Enters",
    "ReplaceBy.Multiply",
    "ReplaceBy.Add",
    "ReplaceBy.Prevent",
    "ReplaceBy.PreventUpTo",
    "ReplaceBy.PreventRemovingCounter",
    "ReplaceBy.MoveTo",
    "ReplaceBy.Instead",
    "ReplaceBy.Tapped",
    "ReplaceBy.WithCounters",
    "ReplaceBy.Choose",
    "ReplaceBy.PayLifeOrTapped",
    "ReplaceBy.RevealOrTapped",
    "ReplaceBy.AsCopy",
    "Keyword.Partner",
    "Keyword.PartnerWith",
    "Keyword.PartnerGroup",
    "Keyword.Companion",
    "Keyword.DoctorsCompanion",
    "Keyword.ChooseABackground",
    "Keyword.Unearth",
    "Keyword.Escape",
    "Keyword.Rebound",
    "Keyword.Aftermath",
    "Keyword.Dash",
    "Keyword.Bestow",
    "Keyword.Mutate",
    "Keyword.Blitz",
    "Keyword.Prowl",
    "Keyword.Mayhem",
    "Keyword.Ninjutsu",
    "Keyword.Echo",
    "Keyword.CumulativeUpkeep",
    "Keyword.Undying",
    "Keyword.Persist",
    "Keyword.Exalted",
    "Keyword.Melee",
    "Keyword.Mentor",
    "Keyword.Dethrone",
    "Keyword.Annihilator",
    "Keyword.Extort",
    "Keyword.Evolve",
    "Trigger.AttacksAlone",
    "Keyword.Vanishing",
    "Keyword.Fading",
    "Keyword.Backup",
    "Keyword.Devour",
    "Keyword.Casualty",
    "Keyword.Offspring",
    "Keyword.Demonstrate",
    "Keyword.Hideaway",
    "Keyword.Gift",
    "ReplaceBy.Devour",
    "Keyword.LevelUp",
    "Keyword.Outlast",
    "Keyword.Saddle",
    "Keyword.Saddled",
    "Keyword.Station",
    "Keyword.Myriad",
    "Keyword.Squad",
    "Keyword.Ravenous",
    "Keyword.Riot",
    "Keyword.Decayed",
    "ReplaceBy.Riot",
    "Keyword.Fabricate",
    "Keyword.Ascend",
    "Keyword.ForMirrodin",
    "Keyword.JobSelect",
    "Keyword.EquipTo",
    "Condition.CitysBlessing",
    "Effect.MyriadCopies",
    "Keyword.JumpStart",
    "Keyword.Retrace",
    "Keyword.Embalm",
    "Keyword.Eternalize",
    "Keyword.Improvise",
    "Keyword.Affinity",
    "Keyword.Typecycling",
    "Keyword.SplitSecond",
    "Keyword.Undaunted",
    "Keyword.Encore",
    "Keyword.Scavenge",
    "Keyword.Transmute",
    "Keyword.Awaken",
    "Keyword.Gravestorm",
    "Keyword.Sunburst",
    "Keyword.Afterlife",
    "Keyword.Horsemanship",
    "Keyword.Spectacle",
    "Keyword.UmbraArmor",
    "Keyword.Cipher",
    "Keyword.Graft",
    "Keyword.Afflict",
    "Keyword.Replicate",
    "Keyword.Renown",
    "Keyword.Exploit",
    "Keyword.Emerge",
    "Keyword.Harmonize",
    "Keyword.ReadAhead",
    "ReplaceWhen.Destroyed",
    "ReplaceBy.UmbraArmor",
    "Condition.NewlyControlled",
    "Condition.Level",
    "Condition.Step",
    "Condition.BeforeAttackers",
    "Condition.InCombat",
    "Condition.FirstCombat",
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
    #[case::discarding_at_random("Effect.Discard.Random", true)]
    #[case::milling("Effect.Mill", true)]
    #[case::scrying("Effect.Scry", true)]
    #[case::looking_at_the_top("Effect.LookAtTop", true)]
    #[case::revealing("Effect.Reveal", true)]
    #[case::shuffling("Effect.Shuffle", true)]
    #[case::moving("Effect.Move", true)]
    #[case::moving_to_the_battlefield("Effect.Move.Battlefield", true)]
    #[case::moving_to_the_battlefield_transformed("Effect.Move.Transformed", true)]
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
    #[case::flipping("Effect.Flip", true)]
    #[case::preparing("Effect.Prepare", true)]
    #[case::entering_prepared("ReplaceBy.EntersPrepared", true)]
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
    #[case::preventing_with_a_counter_removed("ReplaceBy.PreventRemovingCounter", true)]
    #[case::moving_instead("ReplaceBy.MoveTo", true)]
    #[case::entering_tapped("ReplaceBy.Tapped", true)]
    #[case::entering_with_counters("ReplaceBy.WithCounters", true)]
    #[case::choosing_as_it_enters("ReplaceBy.Choose", true)]
    #[case::paying_life_or_entering_tapped("ReplaceBy.PayLifeOrTapped", true)]
    #[case::doing_something_instead("ReplaceBy.Instead", true)]
    #[case::asking_in_place_of_an_event("ReplaceBy.Instead.Asks", false)]
    #[case::tapping_or_untapping("Effect.TapOrUntap", true)]
    #[case::mana_abilities_from_other_zones("Ability.ManaAbility.From", false)]
    #[case::states("Trigger.State", true)]
    #[case::exiling_the_source("Cost.ExileSelf", true)]
    #[case::discarding_the_source("Cost.DiscardSelf", true)]
    #[case::triggers("Ability.Triggered", true)]
    #[case::zone_changes("Trigger.ZoneChange", true)]
    #[case::attacks("Trigger.Attacks", true)]
    #[case::casting("Trigger.SpellCast", true)]
    #[case::sacrifices("Trigger.Sacrificed", true)]
    #[case::mills("Trigger.Milled", true)]
    #[case::steps("Trigger.StepBegins", true)]
    #[case::becoming_a_target("Trigger.BecomesTarget", true)]
    #[case::either_of_two_events("Trigger.AnyOf", true)]
    #[case::delayed_triggers("Effect.DelayedTrigger", true)]
    #[case::destroying("Effect.Destroy", true)]
    #[case::exiling("Effect.Exile", true)]
    #[case::countering("Effect.Counter", true)]
    #[case::continuous_effects("Effect.Continuous", true)]
    #[case::control_for_as_long_as_you_control_the_source("Duration.WhileYouControlSource", true)]
    #[case::a_rule_for_as_long_as_you_control_the_source(
        "Rule.Duration.WhileYouControlSource",
        false
    )]
    #[case::pumps("Mod.ModifyPT", true)]
    #[case::granted_keywords("Mod.AddKeyword", true)]
    #[case::protection_from_a_chosen_colour("Mod.AddProtectionFromChosenColor", true)]
    #[case::shroud("Keyword.Shroud", true)]
    #[case::protection("Keyword.Protection", true)]
    #[case::setting_power_and_toughness("Mod.SetPT", true)]
    #[case::switching_power_and_toughness("Mod.SwitchPT", true)]
    #[case::types("Mod.AddType", true)]
    #[case::removed_types("Mod.RemoveType", true)]
    #[case::set_types("Mod.SetTypes", true)]
    #[case::removed_supertypes("Mod.RemoveSupertype", true)]
    #[case::subtypes("Mod.AddSubtype", true)]
    #[case::colours("Mod.SetColors", true)]
    #[case::granted_abilities("Mod.GrantAbility", true)]
    #[case::lost_abilities("Mod.LoseAbilities", true)]
    #[case::changes_of_control("Mod.SetController", true)]
    #[case::attack_restrictions("Mod.CantAttack", true)]
    #[case::attack_restrictions_on_you("Mod.CantAttackYou", true)]
    #[case::block_restrictions("Mod.CantBlock", true)]
    #[case::static_abilities("Ability.Static", true)]
    #[case::characteristic_defining_abilities("Ability.Static.Defining", true)]
    #[case::keywords_given_to_spells("Ability.Static.SpellKeywords", true)]
    #[case::rules_as_effects("Effect.Rule", true)]
    #[case::rules_as_static_abilities("Ability.Rule", true)]
    #[case::attack_requirements("Rule.MustAttack", true)]
    #[case::block_requirements("Rule.MustBlock", true)]
    #[case::spells_that_cant_be_countered("Rule.CantBeCountered", true)]
    #[case::no_life_gain("Rule.CantGainLife", true)]
    #[case::no_life_loss_from_damage("Rule.NoLifeLossFromDamage", true)]
    #[case::no_drawing("Rule.CantDraw", true)]
    #[case::no_searching("Rule.CantSearch", true)]
    #[case::a_revealed_top("Rule.RevealTop", true)]
    #[case::no_maximum_hand_size("Rule.NoMaxHandSize", true)]
    #[case::no_draw_step("Rule.SkipDrawStep", true)]
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
    #[case::multiplying_counters("Effect.MultiplyCounters", true)]
    #[case::divided_amounts_read_outside_damage_and_counters("Amount.Divided.Elsewhere", false)]
    #[case::even_shares_read_outside_damage_and_counters("Amount.EvenShare.Elsewhere", false)]
    #[case::loyalty_abilities("Ability.Loyalty", true)]
    #[case::loyalty_costs_with_an_x("Cost.Loyalty.X", true)]
    #[case::being_kicked("Condition.Kicked", true)]
    #[case::being_kicked_with_one_cost("Condition.KickedWith", true)]
    #[case::spending_mana_to_cast("Condition.ManaSpent", true)]
    #[case::exiling_for_another_duration("Exile.Duration.UntilYourNextTurn", false)]
    #[case::the_number_of_kicks("Amount.Kicks", true)]
    #[case::the_number_of_mutations("Amount.Mutations", true)]
    #[case::the_size_of_a_party("Amount.Party", true)]
    #[case::cast_for_a_way("Amount.CastWith", true)]
    #[case::colors_spent("Amount.ColorsSpent", true)]
    #[case::destroyed_this_way("Amount.Destroyed", true)]
    #[case::distinct_values("Amount.Distinct", true)]
    #[case::permanents_graved("Amount.PermanentsGraved", true)]
    #[case::commander_casts("Amount.CommanderCasts", true)]
    #[case::may("Effect.May", true)]
    #[case::choosing("Effect.Choose", true)]
    #[case::choosing_at_random("Effect.ChooseAtRandom", true)]
    #[case::modes("Effect.Modal", true)]
    #[case::repeating_for_each("Effect.RepeatEach", true)]
    #[case::rolling_dice("Effect.RollDice", true)]
    #[case::targets_for_each_player("Target.PerPlayer", true)]
    #[case::flipping_coins("Effect.FlipCoin", true)]
    #[case::modes_at_random("Effect.Modal.AtRandom", true)]
    #[case::modes_not_chosen_twice_in_a_turn("Effect.Modal.OnceEachTurn", true)]
    #[case::modes_not_chosen_twice_ever("Effect.Modal.OnceEver", true)]
    #[case::every_mode_under_a_condition("Effect.Modal.AllIf", true)]
    #[case::voting("Effect.Vote", true)]
    #[case::c10_vp_planechase("Effect.Planechase", true)]
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
    #[case::a_copy_until_your_next_turn("Copy.Duration.UntilYourNextTurn", true)]
    #[case::a_copy_that_ends_with_another_duration("Copy.Duration.UntilEndOfCombat", false)]
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
    #[case::c6_eg_paying_counted_life_in_an_effect("Effect.Pay.Counted", true)]
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
    #[case::proliferating("Effect.Proliferate", true)]
    #[case::goading("Effect.Goad", true)]
    #[case::fighting("Effect.Fight", true)]
    #[case::becoming_the_monarch("Effect.BecomeMonarch", true)]
    #[case::amassing("Effect.Amass", true)]
    #[case::monstrosity("Effect.Monstrosity", true)]
    #[case::becoming_a_level("Effect.BecomeLevel", true)]
    #[case::adapting("Effect.Adapt", true)]
    #[case::bolstering("Effect.Bolster", true)]
    #[case::fabricating("Effect.Fabricate", true)]
    #[case::learning("Effect.Learn", true)]
    #[case::crewing("Effect.Crewed", true)]
    #[case::exploring("Effect.Explore", true)]
    #[case::conniving("Effect.Connive", true)]
    #[case::exiling_until_a_card_matches("Effect.ExileUntil", true)]
    #[case::revealing_until_a_card_matches("Effect.RevealUntil", true)]
    #[case::exiling_the_top_cards("Effect.ExileTop", true)]
    #[case::exiling_face_down("Effect.ExileHidden", true)]
    #[case::encoding("Effect.Encode", true)]
    #[case::copying_a_card("Effect.CopyCard", true)]
    #[case::manifesting("Effect.Manifest", true)]
    #[case::cloaking("Effect.Cloak", true)]
    #[case::casting_during_a_resolution("Effect.CastFromZone", true)]
    #[case::cascading("Keyword.Cascade", true)]
    #[case::convoking("Keyword.Convoke", true)]
    #[case::delving("Keyword.Delve", true)]
    #[case::time_travelling("Effect.TimeTravel", true)]
    #[case::extra_turns("Effect.ExtraTurn", true)]
    #[case::added_phases("Effect.AddPhase", true)]
    #[case::a_choice_of_mana("Effect.AddMana.OneOf", true)]
    #[case::a_choice_of_lists_of_mana("Effect.AddMana.OneOfLists", true)]
    #[case::mana_of_the_chosen_color("Effect.AddMana.ChosenColor", true)]
    #[case::mana_of_a_type_or_the_chosen_color("Effect.AddMana.OrChosenColor", true)]
    #[case::mana_restricted_to_what_it_may_be_spent_on("Effect.AddMana.Restricted", true)]
    #[case::mana_kept_past_the_step("Effect.AddMana.Lasts", true)]
    #[case::mana_of_the_commanders_identity("Effect.AddMana.CommanderIdentity", true)]
    #[case::mana_a_land_could_produce("Effect.AddMana.ProducedBy", true)]
    #[case::devotion("Amount.Devotion", true)]
    #[case::mana_the_tapped_land_produced("Effect.AddMana.OfTapped", true)]
    #[case::mana_in_any_combination("Effect.AddMana.AnyCombination", true)]
    #[case::mana_of_one_color_counted("Effect.AddMana.AnyColor", true)]
    #[case::mana_replaced_when_a_condition_holds("Effect.AddMana.Instead", true)]
    #[case::the_greatest_value("Amount.Greatest", true)]
    #[case::totals("Amount.Total", true)]
    #[case::card_types_among_cards("Amount.CardTypes", true)]
    #[case::milling_as_a_cost("Cost.Mill", true)]
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
    #[case::w2e_unblocked_triggers("Trigger.Unblocked", true)]
    #[case::w2e_copy_triggers("Trigger.SpellCopied", true)]
    #[case::group_attack_triggers("Trigger.GroupAttacks", true)]
    #[case::counter_triggers("Trigger.CountersPut", true)]
    #[case::counter_removal_triggers("Trigger.CountersRemoved", true)]
    #[case::chapter_triggers("Trigger.Chapter", true)]
    #[case::attach_triggers("Trigger.Attached", true)]
    #[case::cycle_triggers("Trigger.Cycled", true)]
    #[case::becoming_the_monarch_triggers("Trigger.BecomesMonarch", true)]
    #[case::proliferating_triggers("Trigger.Proliferated", true)]
    #[case::exploring_triggers("Trigger.Explored", true)]
    #[case::conniving_triggers("Trigger.Connived", true)]
    #[case::amassing_triggers("Trigger.Amassed", true)]
    #[case::becoming_monstrous_triggers("Trigger.BecomesMonstrous", true)]
    #[case::becoming_crewed_triggers("Trigger.BecomesCrewed", true)]
    #[case::becoming_saddled_triggers("Trigger.BecomesSaddled", true)]
    #[case::becoming_a_level_triggers("Trigger.BecomesLevel", true)]
    #[case::turning_face_up_triggers("Trigger.TurnedFaceUp", true)]
    #[case::adapting_triggers("Trigger.Adapted", true)]
    #[case::bolstering_triggers("Trigger.Bolstered", true)]
    #[case::surveil_triggers("Trigger.Surveilled", true)]
    #[case::zone_changes_from_elsewhere("Trigger.ZoneChangeExcept", true)]
    #[case::batch_triggers("Trigger.Batch", true)]
    #[case::nth_triggers("Trigger.Nth", true)]
    #[case::event_amounts("Amount.Triggering", true)]
    #[case::if_you_do("Condition.Performed", true)]
    #[case::if_you_do_after_another_effect("Condition.Performed.Other", false)]
    #[case::the_monarch("Condition.Monarch", true)]
    #[case::the_monarch_as_the_turn_began("Condition.MonarchAtTurnStart", true)]
    #[case::the_citys_blessing("Condition.CitysBlessing", true)]
    #[case::becoming_day_or_night("Effect.BecomeDayNight", true)]
    #[case::it_is_day("Condition.IsDay", true)]
    #[case::it_is_night("Condition.IsNight", true)]
    #[case::it_is_neither_day_nor_night("Condition.NeitherDayNorNight", true)]
    #[case::day_becoming_night_triggers("Trigger.DayNightChanges", true)]
    #[case::becoming_day_as_it_enters("ReplaceBy.DayIfNeither", true)]
    #[case::unblockable_creatures("Mod.CantBeBlocked", true)]
    #[case::creatures_that_cant_be_blocked_by_some("Mod.CantBeBlockedBy", true)]
    #[case::creatures_that_can_block_only_some("Mod.CanBlockOnly", true)]
    #[case::creatures_that_need_several_blockers("Mod.MinBlockers", true)]
    #[case::creatures_that_are_blocked_by_few("Mod.MaxBlockers", true)]
    #[case::goaded_creatures("Mod.Goaded", true)]
    #[case::looking_at_the_top_any_time("Rule.LookAtTopAnyTime", true)]
    #[case::additional_land_plays("Rule.AdditionalLandPlays", true)]
    #[case::commander_eligibility("Rule.CanBeCommander", true)]
    #[case::partner("Keyword.Partner", true)]
    #[case::partner_with("Keyword.PartnerWith", true)]
    #[case::partner_group("Keyword.PartnerGroup", true)]
    #[case::doctors_companion("Keyword.DoctorsCompanion", true)]
    #[case::choose_a_background("Keyword.ChooseABackground", true)]
    #[case::unearth("Keyword.Unearth", true)]
    #[case::escape("Keyword.Escape", true)]
    #[case::rebound("Keyword.Rebound", true)]
    #[case::aftermath("Keyword.Aftermath", true)]
    #[case::dash("Keyword.Dash", true)]
    #[case::bestow("Keyword.Bestow", true)]
    #[case::dredge("Keyword.Dredge", true)]
    #[case::dredging("ReplaceBy.Dredge", true)]
    #[case::blitz("Keyword.Blitz", true)]
    #[case::prowl("Keyword.Prowl", true)]
    #[case::mayhem("Keyword.Mayhem", true)]
    #[case::a_conditional_free_cast("Ability.FreeCast", true)]
    #[case::ninjutsu("Keyword.Ninjutsu", true)]
    #[case::joining_an_attack("Effect.JoinAttack", true)]
    #[case::echo("Keyword.Echo", true)]
    #[case::cumulative_upkeep("Keyword.CumulativeUpkeep", true)]
    #[case::undying("Keyword.Undying", true)]
    #[case::persist("Keyword.Persist", true)]
    #[case::exalted("Keyword.Exalted", true)]
    #[case::melee("Keyword.Melee", true)]
    #[case::mentor("Keyword.Mentor", true)]
    #[case::dethrone("Keyword.Dethrone", true)]
    #[case::annihilator("Keyword.Annihilator", true)]
    #[case::extort("Keyword.Extort", true)]
    #[case::evolve("Keyword.Evolve", true)]
    #[case::attacks_alone("Trigger.AttacksAlone", true)]
    #[case::vanishing("Keyword.Vanishing", true)]
    #[case::fading("Keyword.Fading", true)]
    #[case::backup("Keyword.Backup", true)]
    #[case::devour("Keyword.Devour", true)]
    #[case::casualty("Keyword.Casualty", true)]
    #[case::demonstrate("Keyword.Demonstrate", true)]
    #[case::hideaway("Keyword.Hideaway", true)]
    #[case::gift("Keyword.Gift", true)]
    #[case::offspring("Keyword.Offspring", true)]
    #[case::sacrificing_as_it_enters("ReplaceBy.Devour", true)]
    #[case::level_up("Keyword.LevelUp", true)]
    #[case::outlast("Keyword.Outlast", true)]
    #[case::saddle("Keyword.Saddle", true)]
    #[case::saddled("Keyword.Saddled", true)]
    #[case::station("Keyword.Station", true)]
    #[case::ravenous("Keyword.Ravenous", true)]
    #[case::riot("Keyword.Riot", true)]
    #[case::decayed("Keyword.Decayed", true)]
    #[case::choosing_a_counter_or_haste_as_it_enters("ReplaceBy.Riot", true)]
    #[case::copies_for_the_other_defenders("Effect.MyriadCopies", true)]
    #[case::jump_start("Keyword.JumpStart", true)]
    #[case::retrace("Keyword.Retrace", true)]
    #[case::embalm("Keyword.Embalm", true)]
    #[case::eternalize("Keyword.Eternalize", true)]
    #[case::improvise("Keyword.Improvise", true)]
    #[case::affinity("Keyword.Affinity", true)]
    #[case::typecycling("Keyword.Typecycling", true)]
    #[case::for_mirrodin("Keyword.ForMirrodin", true)]
    #[case::job_select("Keyword.JobSelect", true)]
    #[case::equip_to("Keyword.EquipTo", true)]
    #[case::split_second("Keyword.SplitSecond", true)]
    #[case::undaunted("Keyword.Undaunted", true)]
    #[case::coming_under_control_since_the_last_turn("Condition.NewlyControlled", true)]
    #[case::a_class_level("Condition.Level", true)]
    #[case::a_step_of_the_turn("Condition.Step", true)]
    #[case::before_attackers("Condition.BeforeAttackers", true)]
    #[case::in_combat("Condition.InCombat", true)]
    #[case::the_first_combat("Condition.FirstCombat", true)]
    #[case::counting_players("Amount.Players", true)]
    #[case::the_lowest_life_total("Amount.LowestLife", true)]
    #[case::the_highest_life_total("Amount.HighestLife", true)]
    #[case::the_fewest_cards_in_a_hand("Amount.LowestHand", true)]
    #[case::the_life_lost_this_way("Amount.LifeLostThisWay", true)]
    #[case::c11e_a_die_result("Amount.DieResult", true)]
    #[case::c11e_a_roll_trigger("Trigger.DiceRolled", true)]
    #[case::myriad("Keyword.Myriad", true)]
    #[case::crew("Keyword.Crew", true)]
    #[case::an_ability_that_costs_less_to_activate("Cost.Changed", true)]
    #[case::tapping_creatures_for_their_power_as_a_cost("Cost.TapPower", true)]
    #[case::mutate("Keyword.Mutate", true)]
    #[case::mutating_triggers("Trigger.Mutates", true)]
    #[case::encore("Keyword.Encore", true)]
    #[case::scavenge("Keyword.Scavenge", true)]
    #[case::transmute("Keyword.Transmute", true)]
    #[case::awaken("Keyword.Awaken", true)]
    #[case::gravestorm("Keyword.Gravestorm", true)]
    #[case::sunburst("Keyword.Sunburst", true)]
    #[case::afterlife("Keyword.Afterlife", true)]
    #[case::horsemanship("Keyword.Horsemanship", true)]
    #[case::spectacle("Keyword.Spectacle", true)]
    #[case::umbra_armor("Keyword.UmbraArmor", true)]
    #[case::cipher("Keyword.Cipher", true)]
    #[case::graft("Keyword.Graft", true)]
    #[case::afflict("Keyword.Afflict", true)]
    #[case::replicate("Keyword.Replicate", true)]
    #[case::renown("Keyword.Renown", true)]
    #[case::exploit("Keyword.Exploit", true)]
    #[case::emerge("Keyword.Emerge", true)]
    #[case::harmonize("Keyword.Harmonize", true)]
    #[case::read_ahead("Keyword.ReadAhead", true)]
    #[case::becoming_renowned_triggers("Trigger.BecomesRenowned", true)]
    #[case::exploiting_triggers("Trigger.Exploits", true)]
    #[case::replacing_destruction("ReplaceWhen.Destroyed", true)]
    #[case::saving_by_umbra_armor("ReplaceBy.UmbraArmor", true)]
    #[case::pairing_attackers_with_players("Rule.MustAttackPlayers", true)]
    #[case::taxing_attackers("Rule.AttackTax", true)]
    #[case::squad("Keyword.Squad", true)]
    #[case::x_costs(X_COST, true)]
    #[case::x_read_after_resolving("Amount.X.Elsewhere", false)]
    #[case::paying_x_in_an_effect("Effect.Pay.X", true)]
    #[case::paying_a_part_that_counts_x_in_an_effect("Effect.Pay.X.Parts", false)]
    #[case::faces(FACES, true)]
    #[case::the_prepared_cast(PREPARE, true)]
    #[case::variable_stats(VARIABLE_STATS, true)]
    fn the_engine_runs_what_it_runs_today(#[case] name: &str, #[case] runs: bool) {
        assert_eq!(caps().runs(name), runs);
    }
}
