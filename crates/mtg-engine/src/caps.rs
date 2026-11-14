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
    "CostModifier.Exhaust",
    "CostModifier.Boast",
    "CostModifier.Buyback",
    "CostModifier.ManaLife",
    "Ability.By",
    "Ability.From",
    "Ability.Sorcery",
    "Ability.Condition",
    "Trigger.ZoneChange",
    "Trigger.GroupAttacksAt.Walkers",
    "Trigger.DealtExcess",
    "Trigger.State",
    "Trigger.Attacks",
    "Trigger.AttacksPlayer",
    "Trigger.AttacksPlayerOrPlaneswalker",
    "Trigger.PlayerAttacked",
    "Trigger.PlayerAttacks",
    "Trigger.SpellCast",
    "Trigger.SpellCopied",
    "Trigger.PutOnStack",
    "Trigger.Sacrificed",
    "Trigger.Milled",
    "Trigger.StepBegins",
    "Trigger.BecomesTarget",
    "Trigger.PlayerBecomesTarget",
    "Trigger.AnyOf",
    "Trigger.DealsDamage",
    "Trigger.LifeGained",
    "Trigger.DamagePrevented",
    "Trigger.LifeLost",
    "Trigger.Drew",
    "Trigger.FirstDrawRevealed",
    "Trigger.Discarded",
    "Trigger.Tapped",
    "Trigger.TappedNotAttacking",
    "Trigger.Untapped",
    "Trigger.PhasedOut",
    "Trigger.PhasedIn",
    "Trigger.TappedForMana",
    "Trigger.TappedForMana.ChosenColor",
    "Trigger.Blocks",
    "Trigger.BecomesBlocked",
    "Trigger.Unblocked",
    "Trigger.GroupAttacks",
    "Trigger.GroupAttacksAt",
    "Trigger.CountersPut",
    "Trigger.CountersRemoved",
    "Trigger.PlayerCountersPut",
    "Trigger.Chapter",
    "Trigger.NthCounterPut",
    "Trigger.Attached",
    "Trigger.Unattached",
    "Trigger.Cycled",
    "Trigger.BecomesMonarch",
    "Trigger.Proliferated",
    "Trigger.RingTempts",
    "Trigger.CoinFlipWon",
    "Trigger.W15cClashWon",
    "Trigger.DiceRolled",
    "Trigger.Explored",
    "Trigger.Connived",
    "Trigger.Exerted",
    "Trigger.Amassed",
    "Trigger.BecomesMonstrous",
    "Trigger.BecomesRenowned",
    "Trigger.Exploits",
    "Trigger.BecomesCrewed",
    "Trigger.BecomesSaddled",
    "Trigger.FinalChapter",
    "Trigger.BecomesLevel",
    "Trigger.TurnedFaceUp",
    "Trigger.Mutates",
    "Trigger.Adapted",
    "Trigger.Bolstered",
    "Trigger.Expend",
    "Trigger.W11cCrime",
    "Trigger.Surveilled",
    "Trigger.ZoneChangeExcept",
    "Trigger.ResolvedToGraveyard",
    "Trigger.Batch",
    "Trigger.Nth",
    "Trigger.Caused",
    "Trigger.Did",
    "Ability.OpeningHand",
    "Ability.OpeningHandWith",
    "Ability.Sticker",
    "Ability.OpeningReveal",
    "Amount.Triggering",
    "Amount.Players",
    "Amount.Greatest",
    "Amount.Total",
    "Amount.CardTypes",
    "Amount.Devotion",
    "Amount.CardsAbove",
    "Amount.LowestLife",
    "Amount.HighestLife",
    "Amount.LowestHand",
    "Amount.LifeLostThisWay",
    "Amount.DamageDealtThisWay",
    "Amount.Destroyed",
    "Amount.Exiled",
    "Amount.PhasedOut",
    "Amount.Distinct",
    "Amount.DieResult",
    "Amount.DieResult.Distinct",
    "Effect.RollDice.Several",
    "Cost.Tap",
    "Cost.Untap",
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
    "Cost.Craft",
    "Cost.TapPermanents",
    "Cost.UntapPermanents",
    "Cost.PutCounters",
    "Cost.Exert",
    "Cost.OwnManaCost",
    "Cost.ManaCostOf",
    "Cost.OwnManaValue",
    "Cost.OwnManaCostLess",
    "Cost.RemoveCountersAmong",
    "Cost.Discard.Hand",
    "Cost.RevealFromHand.Hand",
    "Cost.Sacrifice.All",
    "Cost.PayLife.Counted.Activated",
    "Cost.PayEnergy.Counted.Activated",
    "Cost.PayLife.Counted.Rather",
    "Cost.PayEnergy.Counted.Rather",
    "Cost.Mill.Counted.Activated",
    "Cost.RemoveCounters.Counted.Activated",
    "Cost.Sacrifice.Counted.Activated",
    "Cost.Sacrifice.Counted.Required",
    "Cost.Discard.Counted.Activated",
    "Cost.ExileFromGraveyard.Counted.Activated",
    "Cost.Craft.Counted.Activated",
    "Cost.ExileFromHand.Counted.Activated",
    "Cost.RevealFromHand.Counted.Activated",
    "Cost.TapPermanents.Counted.Activated",
    "Cost.ReturnToHand.Counted.Activated",
    "Cost.ExilePermanents.Counted.Activated",
    "Cost.PutCounters.Counted.Activated",
    "Cost.UntapPermanents.Counted.Activated",
    "Cost.RemoveCountersAmong.Counted.Activated",
    "Cost.ReturnToHand",
    "Cost.ExilePermanents",
    "Cost.ExileSpell",
    "Cost.ExileFromHand",
    "Cost.Pick.FilterX",
    "Cost.RevealFromHand",
    "Cost.AlikeFromHand",
    "Cost.OneOf",
    "Cost.TapPower",
    "Cost.Changed",
    "Cost.DamageToYou",
    "Cost.OthersGainLife",
    "Cost.GetPoison",
    "Cost.Unattach",
    "Cost.ExileTop",
    "Cost.Draw",
    "Cost.DiscardRandom",
    "Cost.CollectEvidence",
    "Cost.ExileCardTypes",
    "Cost.GenericOf",
    "Cost.Blight",
    "Cost.Behold",
    "Cost.ToLibrary",
    "Cost.Forage",
    "Cost.Waterbend",
    "Cost.XAtLeast",
    "Cost.XColor",
    "Cost.ChosenColorOnly",
    "Stat.R374Types",
    "Stat.R374CreatureTypes",
    "Stat.R374Bushido",
    "Stat.R374Toxic",
    "Stat.R374ManaCost",
    "Stat.R374ColorSet",
    "Stat.R4700ChosenColors",
    "Stat.R6018PermanentTypes",
    "Amount.R374AtMost",
    "Cost.Alternative.X",
    "Cost.XAtMost",
    "Cost.XAtMost.Spell",
    "Cost.ExiledToGraveyard",
    "Cost.DamageToYou.Counted.Activated",
    "Cost.ExileTop.Counted.Activated",
    "Cost.Draw.Counted.Activated",
    "Cost.DiscardRandom.Counted.Activated",
    "Cost.Behold.Counted.Activated",
    "Cost.ToLibrary.Counted.Activated",
    "Effect.Draw",
    "Effect.Discard",
    "Effect.Discard.Random",
    "Effect.RollDice.Highest",
    "Effect.Choose.Differ",
    "Effect.Discard.Filter",
    "Effect.Discard.Chooser",
    "Effect.Mill",
    "Effect.Scry",
    "Effect.LookAtTop",
    "Effect.LookAtHand",
    "Effect.LookAtFaceDown",
    "Effect.Reveal",
    "Effect.Search",
    "Search.Zones",
    "RevealUntil.Most",
    "LifeGain.CausedOwn",
    "ReplaceBy.UnderYourControl",
    "Search.Of",
    "Search.OfZones",
    "Effect.Shuffle",
    "Effect.Move",
    "Effect.Move.Battlefield",
    "Effect.Move.Transformed",
    "Effect.Move.Attached",
    "Effect.Move.FaceDown",
    "Effect.Move.Enchanting",
    "Effect.Move.Command",
    "Effect.Move.Random.Top",
    "Effect.ExileUntil",
    "ExileUntil.Total",
    "Effect.RevealUntil",
    "Effect.ExileTop",
    "Effect.ExileHidden",
    "Effect.Encode",
    "Effect.Remember",
    "Effect.Forget",
    "Effect.RememberValue",
    "Effect.CopyCard",
    "Effect.Manifest",
    "Effect.Cloak",
    "Effect.FaceDownPile",
    "Effect.BecomeForetold",
    "Effect.CastFromZone",
    "Effect.CastFromZone.Transformed",
    "Effect.Destroy",
    "Effect.Exile",
    "Effect.Sacrifice",
    "Effect.SacrificeUpTo",
    "Effect.Counter",
    "Effect.DealDamage",
    "Effect.DealDamage.Excess",
    "Effect.DealDamage.Unpreventable",
    "Effect.GainLife",
    "Effect.LoseLife",
    "Effect.AddMana",
    "Effect.AddMana.OneOf",
    "Effect.AddMana.OneOfLists",
    "Effect.AddMana.ChosenColor",
    "Effect.AddMana.OrChosenColor",
    "Effect.AddMana.Restricted",
    "Effect.AddMana.Uncounterable",
    "Effect.AddMana.Lasts",
    "Effect.AddMana.WhenSpent",
    "Effect.AddMana.CommanderIdentity",
    "Effect.AddMana.CostOf",
    "Effect.AddMana.ProducedBy",
    "Effect.AddMana.EachColorAmong",
    "Effect.AddMana.AnyColorAmong",
    "Effect.AddMana.DraftedColors",
    "Effect.AddMana.Devotion",
    "Effect.AddMana.Enters",
    "Effect.AddMana.OfTapped",
    "Effect.AddMana.AnyCombination",
    "Effect.AddMana.AnyColor",
    "Effect.AddMana.Instead",
    "Ability.ManaAbility.Complex",
    "Cost.OneOf.Pay",
    "Ability.ManaAbility.Other",
    "Ability.ManaAbility.From",
    "Effect.PutCounters",
    "Effect.RemoveCounters",
    "Effect.RemoveAllCounters",
    "Effect.MoveCounters",
    "Effect.SpellEntersWith",
    "Effect.MultiplyCounters",
    "Effect.PutCountersOf",
    "Effect.Tap",
    "Effect.Untap",
    "Effect.TapOrUntap",
    "Effect.PhaseOut",
    "PhaseOut.Held",
    "Effect.Attach",
    "Effect.Unattach",
    "Effect.Pair",
    "Effect.JoinAttack",
    "Effect.Transform",
    "Effect.Flip",
    "Effect.TurnFaceUp",
    "Effect.TurnFaceDown",
    "Effect.Prepare",
    "Effect.Unprepare",
    "Effect.Suspect",
    "Effect.Unsuspect",
    "Effect.Solve",
    "Effect.Harness",
    "Effect.Plot",
    "Effect.WinGame",
    "Effect.LoseGame",
    "Effect.DrawGame",
    "Effect.SetLife",
    "Effect.ExchangeLife",
    "Effect.ExchangeLifeWithToughness",
    "Effect.RemoveDamage",
    "Effect.RepeatWhile",
    "Effect.SeparatePiles",
    "Effect.Balance",
    "Effect.ExtraTurn",
    "Effect.EndTurn",
    "Effect.AddPhase",
    "Effect.AddStep",
    "Effect.Surveil",
    "Effect.Proliferate",
    "Effect.TimeTravel",
    "Effect.GainSuspend",
    "Effect.Goad",
    "Effect.Fight",
    "Effect.Explore",
    "Effect.Connive",
    "Effect.Amass",
    "Effect.Empower",
    "Effect.ChooseWithin",
    "Effect.Incubate",
    "Effect.Conjure",
    "Effect.Seek",
    "Effect.Monstrosity",
    "Effect.Renown",
    "Effect.Exploit",
    "Effect.Enlist",
    "Effect.BecomeLevel",
    "Effect.Adapt",
    "Effect.Bolster",
    "Effect.Fabricate",
    "Effect.Learn",
    "Effect.Performed",
    "Effect.Crewed",
    "Effect.BecomeMonarch",
    "Effect.Revive",
    "Effect.ReduceSpeed",
    "Effect.ControlPlayer",
    "Effect.ControlPlayer.By",
    "Effect.RestartGame",
    "Effect.RingTempts",
    "Effect.Venture",
    "Effect.OpenAttraction",
    "Effect.RollToVisit",
    "Effect.PutSticker",
    "Effect.Parade",
    "Effect.ClaimPrize",
    "Trigger.VisitedAttraction",
    "Trigger.Prize",
    "Trigger.ClaimedPrize",
    "Trigger.Ventured",
    "Trigger.CompletedDungeon",
    "Trigger.Visit",
    "Trigger.RolledToVisit",
    "Trigger.OpenedAttraction",
    "Trigger.DoorUnlocked",
    "Effect.UnlockDoor",
    "Effect.LockOrUnlockDoor",
    "Condition.CompletedDungeon",
    "Effect.TakeInitiative",
    "Trigger.TakesInitiative",
    "Condition.Initiative",
    "Effect.CreateEmblem",
    "Condition.Monarch",
    "Condition.ExtraTurn",
    "Condition.MonarchAtTurnStart",
    "Condition.AttackedYouLastTurn",
    "Effect.BecomeDayNight",
    "Condition.IsDay",
    "Condition.IsNight",
    "Condition.NeitherDayNorNight",
    "Trigger.DayNightChanges",
    "ReplaceBy.DayIfNeither",
    "ReplaceBy.EntersPrepared",
    "ReplaceBy.ChooseWord",
    "ReplaceBy.OpponentChooses",
    "ReplaceBy.BecomesChoice",
    "ReplaceBy.Mill",
    "ReplaceBy.UnderChosenOpponent",
    "ReplaceBy.AddAmount",
    "ReplaceBy.MayAddAmount",
    "ReplaceBy.PreventAmount",
    "ReplaceBy.PreventAtMost",
    "Effect.CreateToken",
    "Effect.CreateToken.Copy",
    "Effect.CreateToken.Card",
    "Effect.CreateToken.R4700OfChosen",
    "Effect.CopySpell",
    "Effect.CopyAbility",
    "Effect.ChangeTargets",
    "Effect.BecomeCopy",
    "Copy.Duration.UntilYourNextTurn",
    "Copy.Duration.WhileSourceAttached",
    "Copy.Except.ThisAbility",
    "Copy.Except.SetPT.Amount",
    "Effect.Continuous",
    "Effect.ExchangeControl",
    "Duration.WhileYouControlSource",
    "Rule.Duration.WhileYouControlSource",
    "Rule.Duration.UntilTheirNextTurn",
    "Rule.Duration.UntilItsControllersNextCombatEnds",
    "Duration.WhileSourceTapped",
    "Duration.WhileYouControlSourceTapped",
    "Duration.DuringItsControllersNextTurn",
    "Duration.WhileHasCounter",
    "Duration.WhileEnchanted",
    "Duration.WhileTapped",
    "Rule.Duration.WhileTapped",
    "Duration.WhileMonarch",
    "Duration.UntilItsControllersNextUntapStep",
    "Duration.UntilPaid",
    "Duration.UntilCastFromExile",
    "Duration.UntilCreatureSpellCast",
    "Effect.Rule",
    "Effect.Regenerate",
    "Effect.RemoveFromCombat",
    "Effect.BecomeBlocked",
    "Effect.PutBeneathTop",
    "Effect.LoseUnspentMana",
    "Effect.Shield",
    "Ability.Static",
    "Ability.Static.Defining",
    "Ability.Static.SpellKeywords",
    "Ability.Rule",
    "Rule.MustAttack",
    "Rule.MustAttackPlayers",
    "Rule.MustAttackAt",
    "Rule.MustAttackWithOne",
    "Rule.AttackTax",
    "Rule.BlockTax",
    "Rule.AttackLimit",
    "Rule.AttackLimitAt",
    "Rule.BlockLimit",
    "Rule.BlockLimitEach",
    "Rule.MustBlock",
    "Rule.MustBlockAttacker",
    "Rule.Combat",
    "Rule.Combat.AttackIfAnyAttacks",
    "Rule.Combat.MustBeBlockedByOne",
    "Rule.Combat.R6001MustBeBlockedByAtLeast",
    "Rule.IgnoreHexproof",
    "Rule.CantBeCountered",
    "Rule.CantBeRegenerated",
    "Rule.CantGainLife",
    "Rule.NoLifeLossFromDamage",
    "Rule.LifeFloor",
    "Rule.RadiationGainsLife",
    "Rule.ManaProduced",
    "Rule.MayCastRather",
    "Rule.LifeTotalCantChange",
    "Rule.PlayersHave",
    "Rule.CantDraw",
    "Rule.DrawFromBottom",
    "Rule.CantSearch",
    "Rule.OwnSpellsCantSearch",
    "Rule.SearchTop",
    "Rule.UntapRemovesCounters",
    "ReplaceBy.HealFirst",
    "Rule.VoteAgain",
    "Rule.ExtraVote",
    "Rule.ReverseTurnOrder",
    "Rule.AttackOnlyNearest",
    "Rule.SkipExtraTurns",
    "Rule.PowerUpAgain",
    "Limit.PowerUp",
    "Rule.FlipTwoCoins",
    "Rule.ExtraDie",
    "Rule.ExtraPlanarDie",
    "Rule.PlaneswalkLookTwo",
    "Rule.KeepUnspentMana",
    "Rule.DrawLimit",
    "Rule.AbilitiesAsThoughHaste",
    "Rule.PayLifeForMana",
    "Rule.PayLifeForOneSymbol",
    "Rule.OpponentsCantMakeSacrifice",
    "Rule.OpponentsCantMakeDiscardOrSacrifice",
    "Rule.SpendManaAs",
    "Rule.UnspentManaBecomes",
    "Rule.RevealTop",
    "Rule.RevealHand",
    "Rule.ControlSearches",
    "Rule.ExileFound",
    "Rule.NoMaxHandSize",
    "Rule.LoyaltyTwice",
    "Rule.EchoRather",
    "Rule.RevealFirstDraw",
    "Rule.BoastTwice",
    "Rule.LoyaltyAnyTime",
    "Rule.LoyaltyCostsMore",
    "Rule.MaxHandSize",
    "Rule.HandSizeChange",
    "Rule.NoLegendRule",
    "Rule.NoLegendRuleFor",
    "Rule.UntapDuring",
    "Rule.UntapLimit",
    "Rule.SkipDrawStep",
    "Rule.R6007ExhaustAgain",
    "Rule.LookAtTopAnyTime",
    "Rule.LookAtExiled",
    "Rule.LookAtFaceDown",
    "Rule.AdditionalLandPlays",
    "Rule.TriggerAgain",
    "Rule.NextSpell",
    "Rule.SpellCost",
    "Rule.DamageCantBePrevented",
    "Rule.GraveyardCardsUntargetable",
    "Rule.CantLoseGame",
    "Rule.CantCastSpells",
    "Rule.ToughnessAssignsDamage",
    "Rule.LethalByPower",
    "Rule.DevotionIncreased",
    "Rule.DamageAsInfect",
    "Rule.VentureLimit",
    "Rule.DamageAsWither",
    "Rule.ManaBurn",
    "Rule.AttackAsThoughHasteAt",
    "Rule.CountersRemain",
    "Rule.Cant",
    "Rule.Cant.Activate",
    "Rule.Cant.BeTargeted",
    "Rule.Cant.BeTargetedByWallsOnly",
    "Rule.Cant.PlayLand",
    "Rule.Cant.PlayLands",
    "Rule.Cant.Sacrifice",
    "Rule.Cant.PutCounter",
    "Rule.Cant.PutCounterOf",
    "Rule.Cant.RemoveCounterOf",
    "Rule.Cant.Trigger",
    "Rule.Cant.Attach",
    "Rule.Cant.BeAttachedBy",
    "Rule.Cant.Discard",
    "Rule.Cant.Exile",
    "Rule.Cant.Transform",
    "Rule.Cant.PhaseOut",
    "Rule.Cant.Untap",
    "Rule.Cant.Crew",
    "Rule.Cant.BecomeSuspected",
    "Rule.Cant.BecomeMonarch",
    "Rule.Cant.PhaseIn",
    "Rule.Cant.EnterBattlefield",
    "Rule.Cant.SkipStep",
    "Rule.Cant.SkipTurn",
    "Rule.Cant.SkipCombat",
    "Rule.Cant.Cast",
    "Rule.Cant.Attack",
    "Rule.Cant.ActivateKind",
    "Rule.Cant.PayCosts",
    VARIABLE_STATS,
    "Mod.ModifyPT",
    "Mod.AddKeyword",
    "Mod.AddProtectionFromChosenColor",
    "Mod.AddProtectionFromChosenCardType",
    "Mod.SetPT",
    "Mod.SetPower",
    "Mod.SetToughness",
    "Mod.SwitchPT",
    "Mod.AddType",
    "Mod.RemoveType",
    "Mod.SetTypes",
    "Mod.RemoveSupertype",
    "Mod.AddSupertype",
    "Mod.SetName",
    "Mod.ThisAbility",
    "Mod.SetLoyalty",
    "Mod.AddSubtype",
    "Mod.SetCreatureTypes",
    "Mod.SetLandTypes",
    "Mod.AddAllCreatureTypes",
    "Mod.SetColors",
    "Mod.AddColors",
    "Mod.SetColorsChosen",
    "Mod.SetLandTypesChosen",
    "Mod.SetCreatureTypesOf",
    "Mod.KeywordsOf",
    "Mod.SetLandTypesNthChosen",
    "Mod.ChangeTextChosen",
    "Mod.SetCreatureTypesChosen",
    "Mod.AddCreatureTypeChosen",
    "Mod.AddLandTypeChosen",
    "Mod.AddChosenSubtype",
    "Mod.SetChosenColor",
    "Mod.AddChosenColor",
    "Mod.R6001AddChosenAbilities",
    "Mod.SetCommanderColor",
    "Mod.RemoveKeyword",
    "Mod.RemoveKeywordKind",
    "Mod.LockKeyword",
    "Mod.GrantAbility",
    "Mod.GainAbilitiesOf",
    "Mod.LoseAbilities",
    "Mod.LoseAbilitiesExceptMana",
    "Mod.LoseThisAbility",
    "Mod.LosePrinted",
    "Mod.SetController",
    "Mod.CantAttack",
    "Mod.CantAttackYou",
    "Mod.CantAttackOwner",
    "Mod.CantBeBlockedUnlessAttackingOwner",
    "Mod.CantBlock",
    "Mod.DoesntUntap",
    "Mod.CantBeTurnedFaceUp",
    "Mod.CantBeBlocked",
    "Mod.CantBeBlockedBy",
    "Mod.CanBlockOnly",
    "Mod.MinBlockers",
    "Mod.MaxBlockers",
    "Mod.BlockedByAll",
    "Mod.BlocksAdditional",
    "Mod.BlocksAdditionalPer",
    "Mod.Goaded",
    "Effect.DelayedTrigger",
    "Effect.Reflexive",
    "Effect.If",
    "Effect.May",
    "Effect.Together",
    "Effect.Pay",
    "Effect.PayAnyTimes",
    "Effect.StillResolves",
    "Effect.Rephase",
    "Effect.Reattack",
    "Effect.Pay.Counted",
    "Effect.Pay.Took",
    "Effect.Pay.X",
    "Effect.Pay.X.Energy",
    "Effect.Pay.X.Among",
    "Effect.Pay.X.Life",
    "Effect.Pay.SacrificeSelf",
    "Effect.Pay.ExileSelf",
    "Effect.Choose",
    "Effect.ChooseAtRandom",
    "Effect.ChooseValue",
    "Effect.Modal",
    "Effect.RepeatEach",
    "Effect.RollDice",
    "Target.PerPlayer",
    "Target.Together",
    "Target.TotalManaValue",
    "Target.TotalPower",
    "Target.Another",
    "Target.ChosenBy",
    "Target.AtRandom",
    "Target.PlayerAmong",
    "Effect.FlipCoin",
    "Effect.Clash",
    "Effect.Modal.AtRandom",
    "Effect.Modal.OnceEachTurn",
    "Effect.Modal.OnceEver",
    "Effect.Modal.AllIf",
    "Effect.Vote",
    "Effect.ChooseNumbers",
    "Effect.ChooseWords",
    "Effect.ChooseWords.About",
    "Choose.DifferentColorSets",
    "Effect.BidLife",
    "Effect.Let",
    "Effect.Bind",
    "Effect.Modal.Who",
    "Effect.Modal.ModeCosts",
    "Effect.Modal.ExtraModeCost",
    "Effect.Modal.Pawprints",
    "Effect.Modal.DifferentPlayers",
    "Effect.Planechase",
    "Effect.VillainousChoice",
    "Effect.Either",
    "Effect.OneOf",
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
    "Keyword.ProtectionSparing",
    "Keyword.HexproofFrom",
    "Keyword.Prowess",
    "Keyword.BattleCry",
    "Keyword.Ward",
    "Keyword.Enchant",
    "Keyword.Enchant.Graveyard",
    "Keyword.EnchantPlayer",
    "Keyword.Equip",
    "Keyword.Fortify",
    "Keyword.Reconfigure",
    "Keyword.LivingWeapon",
    "Keyword.Kicker",
    "Keyword.Multikicker",
    "Keyword.Buyback",
    "Keyword.Entwine",
    "Ability.AdditionalCost",
    "Ability.FreeCast",
    "Ability.FreeActivation",
    "Ability.InstantActivation",
    "Ability.AltCast",
    "Ability.AltCast.X",
    "Ability.CastOnly",
    "Ability.CantTargetAsCast",
    "Condition.AdditionalCostPaid",
    "Keyword.Flashback",
    "Keyword.Flashback.Cost.Discard.X",
    "Keyword.Flashback.Cost.Sacrifice.X",
    "Keyword.Flashback.Cost.ExileFromGraveyard.X",
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
    "Keyword.Suspend.X",
    "Keyword.Kicker.X",
    "Keyword.Devour.X",
    "Keyword.TurnUp.X",
    "Keyword.Foretell",
    "Keyword.Plot",
    "Keyword.Morph",
    "Keyword.Megamorph",
    "Keyword.Disguise",
    "Keyword.TurnUp.Cost.PayLife",
    "Keyword.TurnUp.Cost.Mill",
    "Keyword.TurnUp.Cost.Draw",
    "Keyword.TurnUp.Cost.DiscardRandom",
    "Keyword.TurnUp.Cost.RemoveCounters",
    "Keyword.TurnUp.Cost.Sacrifice",
    "Keyword.TurnUp.Cost.Discard",
    "Keyword.TurnUp.Cost.ExileFromGraveyard",
    "Keyword.TurnUp.Cost.ExileFromHand",
    "Keyword.TurnUp.Cost.RevealFromHand",
    "Keyword.TurnUp.Cost.TapPermanents",
    "Keyword.TurnUp.Cost.ReturnToHand",
    "Keyword.TurnUp.Cost.DamageToYou",
    "Keyword.TurnUp.Cost.ExileTop",
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
    "Condition.CastOffSorceryTiming",
    "Condition.AsCast",
    "Amount.CountedAsCast",
    "Condition.ManaSpent",
    "Rule.MayCast",
    "NewTargets.Random",
    "CastPermission.limit",
    "MayCast.AnyTurn",
    "CastRider.EntersTapped",
    "CastRider.EntersWithCounter",
    "CastRider.GainsThisTurn",
    "CastRider.AlsoSubtype",
    "CastRider.ExiledInstead",
    "Condition.R401OnCast",
    "Condition.R401Targets",
    "Condition.Kicked",
    "Condition.KickedWith",
    "Condition.Performed",
    "Condition.ChosenWord",
    "Condition.EnteredAs",
    "Amount.Kicks",
    "Amount.AdditionalPaid",
    "Amount.Mutations",
    "Amount.Promised",
    "Amount.ResolvedThisTurn",
    "Amount.CastWith",
    "Amount.ColorsSpent",
    "Amount.PermanentsGraved",
    "Amount.CommanderCasts",
    "Amount.CastsFromCommand",
    "Amount.Speed",
    "Amount.Tallied",
    "Amount.Largest",
    "Amount.Smallest",
    "Amount.HighestCounters",
    "Amount.StartingLife",
    "Amount.TurnsTaken",
    "Amount.LifeAsTurnBegan",
    "Amount.CastLastTurn",
    "Amount.TalliedLastTurn",
    "Amount.Party",
    "Amount.Tally",
    "Amount.SourcesDealtDamage",
    "Amount.MostDamageBySource",
    "Amount.DamageToFromPlayers",
    "Rule.SpendManaAs.AbilitiesOnly",
    "Amount.Drafted",
    "Amount.CommanderColors",
    "Amount.Remembered",
    "Amount.DamageDealtByTo",
    "Amount.TurnNumber",
    "Amount.Stickers",
    "Amount.StickerVowels",
    "Amount.TallyLastTurn",
    "Amount.CastSameNameThisGame",
    "Amount.CycledSameNameThisGame",
    "Condition.DamagedThisGameBy",
    "Amount.TallyTotal",
    "Amount.TriggeringTargets",
    "Amount.PlayersLost",
    "Amount.CastTargets",
    "Amount.CastTargetsOf",
    "Amount.When",
    "Amount.ManaSpent",
    "Amount.ManaColors",
    "Amount.MostCommon",
    "Amount.CountOf",
    "Amount.Product",
    "Amount.Random",
    "Amount.ManaInPool",
    "Amount.ManaInPoolOf",
    "Amount.Doors",
    "Condition.StartingPlayer",
    "Condition.Among",
    "Ability.Replacement",
    "Ability.CostModifier",
    "ReplaceWhen.TokenCreation",
    "ReplaceWhen.Damage",
    "ReplaceWhen.R6001DamageBetweenChosen",
    "ReplaceWhen.ZoneChange",
    "ReplaceWhen.Countered",
    "ReplaceWhen.BecomesAttached",
    "ReplaceWhen.Discarded",
    "ReplaceWhen.TurnedFaceUp",
    "ReplaceWhen.CountersPut",
    "ReplaceWhen.LifeGain",
    "ReplaceWhen.LifeLoss",
    "ReplaceWhen.LoseGame",
    "ReplaceWhen.Mill",
    "ReplaceWhen.Draw",
    "ReplaceWhen.Draw.ExceptDrawStep",
    "ReplaceWhen.Enters",
    "ReplaceWhen.Explores",
    "ReplaceWhen.Connives",
    "ReplaceWhen.Proliferates",
    "ReplaceWhen.Scries",
    "Scries.Add",
    "ReplaceWhen.Surveils",
    "ReplaceWhen.CopiesSpell",
    "ReplaceWhen.Learns",
    "ReplaceBy.MayInstead",
    "ReplaceBy.IntoLibrary.Optional",
    "ReplaceBy.Multiply",
    "ReplaceBy.LossTimes",
    "ReplaceBy.Add",
    "ReplaceBy.Half",
    "ReplaceBy.Subtract",
    "ReplaceBy.Cap",
    "ReplaceBy.Prevent",
    "ReplaceBy.PreventNext",
    "ReplaceBy.PreventNextAnd",
    "ReplaceBy.PreventUpTo",
    "ReplaceBy.PreventCounted",
    "ReplaceBy.PreventCountedAnd",
    "ReplaceBy.PreventHalf",
    "ReplaceBy.PreventRemovingCounter",
    "ReplaceBy.PreventByCounters",
    "RollDice.Count",
    "ReplaceBy.PreventHalfNext",
    "ReplaceBy.PreventAllButNext",
    "ReplaceBy.MoveTo",
    "ReplaceBy.Untapped",
    "ReplaceBy.IntoLibrary",
    "ReplaceBy.Instead",
    "ReplaceBy.Plus",
    "ReplaceBy.InsteadNext",
    "ReplaceBy.RedirectUpTo",
    "ReplaceBy.RedirectCounted",
    "ReplaceBy.Redirect",
    "ReplaceBy.PreventAnd",
    "ReplaceBy.PreventAllBut",
    "ReplaceBy.Tapped",
    "ReplaceBy.WithCounters",
    "ReplaceBy.WithCountersAnd",
    "ReplaceBy.WithCountersAnd.Also",
    "ReplaceBy.Tribute",
    "ReplaceBy.Amplify",
    "ReplaceBy.MayWithCounters",
    "ReplaceBy.Choose",
    "ReplaceBy.Choose.NoncreatureNonlandCardName",
    "ReplaceBy.Choose.NonbasicLandCardName",
    "ReplaceBy.Choose.RevealedNonlandName",
    "ReplaceBy.Choose.R6001Abilities",
    "ReplaceBy.ChooseAfter",
    "ReplaceBy.PayLifeOrTapped",
    "ReplaceBy.TappedForCounters",
    "ReplaceBy.RevealOrTapped",
    "ReplaceBy.R6007RevealOrControl",
    "ReplaceBy.Instead.PaysLife",
    "ReplaceBy.BeholdOrTapped",
    "ReplaceBy.PayOrGraveyard",
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
    "Keyword.Paradigm",
    "Keyword.Aftermath",
    "Keyword.Dash",
    "Keyword.Bestow",
    "Keyword.Mutate",
    "Keyword.Blitz",
    "Keyword.Prowl",
    "Keyword.Mayhem",
    "Keyword.Disturb",
    "Keyword.MoreThanMeetsTheEye",
    "Keyword.Ninjutsu",
    "Keyword.CommanderNinjutsu",
    "Keyword.Echo",
    "Keyword.CumulativeUpkeep",
    "Keyword.CumulativeUpkeepActs",
    "Keyword.Compleated",
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
    "Keyword.Bargain",
    "Keyword.Conspire",
    "Keyword.Offspring",
    "Keyword.Demonstrate",
    "Keyword.Hideaway",
    "Keyword.Gift",
    "ReplaceBy.Devour",
    "ReplaceBy.AsEnters",
    "ReplaceBy.AsEntersWith",
    "Keyword.LevelUp",
    "Keyword.Outlast",
    "Keyword.Saddle",
    "Keyword.Saddled",
    "Keyword.Exert",
    "Keyword.FlashSacrifice",
    "Keyword.Macro",
    "Keyword.MacroX",
    "Keyword.Station",
    "Keyword.Myriad",
    "Keyword.Squad",
    "Keyword.Ravenous",
    "Keyword.Riot",
    "Keyword.Decayed",
    "ReplaceBy.Riot",
    "ReplaceBy.WithCounterOf",
    "ReplaceBy.WithCountersOf",
    "Keyword.Fabricate",
    "Keyword.Ascend",
    "Keyword.Storied",
    "Keyword.ForMirrodin",
    "Keyword.StartYourEngines",
    "Keyword.JobSelect",
    "Keyword.EquipTo",
    "Keyword.CrewOnce",
    "Condition.CitysBlessing",
    "Condition.EnduringStory",
    "Condition.VotedAs",
    "Amount.GreatestCast",
    "Condition.MaxSpeed",
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
    "Keyword.Assist",
    "Keyword.Craft",
    "Keyword.Prototype",
    "Keyword.Gravestorm",
    "Keyword.Sunburst",
    "Keyword.Afterlife",
    "Keyword.Horsemanship",
    "Keyword.Phasing",
    "Amount.Devoured",
    "Amount.RemovedThisWay",
    "Keyword.Spectacle",
    "Keyword.Freerunning",
    "Keyword.UmbraArmor",
    "Keyword.Cipher",
    "Keyword.Haunt",
    "Keyword.Graft",
    "Keyword.Afflict",
    "Keyword.Replicate",
    "Keyword.Renown",
    "Keyword.Exploit",
    "Keyword.Soulbond",
    "Keyword.Emerge",
    "Keyword.Offering",
    "Keyword.Harmonize",
    "Keyword.ReadAhead",
    "Keyword.Recover",
    "Keyword.Ripple",
    "Keyword.Epic",
    "Keyword.Banding",
    "Keyword.BandsWithOther",
    "Keyword.Splice",
    "Keyword.Cleave",
    "ReplaceWhen.Destroyed",
    "ReplaceBy.UmbraArmor",
    "ReplaceBy.Regenerate",
    "Condition.NewlyControlled",
    "Condition.Level",
    "Condition.Searched",
    "Condition.Step",
    "Condition.BeforeAttackers",
    "Condition.AfterBlockers",
    "Condition.InCombat",
    "Condition.FirstCombat",
    "Condition.FirstStep",
    "Condition.Drafted",
    "Condition.AttackedThisTurnBy",
    "Condition.Exploited",
    "Condition.RingTempted",
    "Condition.FastestPlayer",
    "Condition.CompletedNamedDungeon",
    "Condition.AttackedThisCombat",
    "Condition.SorceryTiming",
    "Condition.AfterCombat",
    "Condition.TriggeringSticker",
    "Rule.CanBeCommander",
    "Rule.AnyNumberInDeck",
    "Rule.UpToInDeck",
    "Rule.Draft",
    "Rule.CantBeCopied",
    "Rule.KeepsDamage",
    "Rule.Flagbearers",
    "Rule.CountsAsNamed",
    "Rule.CantSpendMana",
    "Rule.TaxAsLife",
    "Rule.R6018PlotLess",
    "Nth.R6018HitPlayer",
    "Rule.SpendOnlyFrom",
    "Pred.Modified",
    "Pred.RingBearer",
    "Pred.HasAdventure",
    "Pred.HasAbility",
    "Pred.Drafted",
    "Pred.Apart",
    "Pred.TotalAtMost",
    "Pred.FaceDown",
    "Pred.TargetsPlayer",
    "Pred.SpentFrom",
    "Pred.ManaFrom",
    "Pred.GravedThisTurn",
    "Pred.GravedThisCombat",
    "Pred.SharesColorWithLastCast",
    "Pred.CouldAttackThisTurn",
    "Pred.ArrivedFrom",
    "Pred.ArrivedThisTurn",
    "Pred.DidThisTurn",
    "Pred.BlockedThisTurn",
    "Pred.DealtDamageThisTurn",
    "Pred.R6100DamagedThisGameBy",
    "Pred.EnteredSinceLastTurn",
    "Condition.CombatDamagedSinceLastTurn",
    "Pred.WasCast",
    "Pred.CastWith",
    "Pred.Test",
    "Pred.TopOfLibrary",
    "Pred.BottomOfLibrary",
    "Pred.TopOfGraveyard",
    "Pred.TopCreatureOfGraveyard",
    "Pred.R4700TopOfGraveyard",
    "Ref.R4700OtherChosen",
    "Pred.BlockedBy",
    "Pred.BlockedThisTurnBy",
    "Pred.CastBefore",
    "Amount.TallyTypes",
    "Pred.BandedWith",
    "Pred.Blocks",
    "Pred.TargetedBy",
    "Pred.FromSource",
    "Pred.SingleTarget",
    "Pred.GreatestAmong",
    "Pred.LeastAmong",
    "Pred.HasKeywordNamed",
    "Pred.CounterCount",
    "Pred.Suspended",
    "Pred.Foretold",
    "Pred.HasName",
    "Pred.Kicked",
    "Pred.AdditionalCostPaid",
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
    #[case::r5505_a_player_controlled_by_another("Effect.ControlPlayer.By", true)]
    #[case::r5518_dice_keeping_the_highest("Effect.RollDice.Highest", true)]
    #[case::r5518_alike_cards_from_hand("Cost.AlikeFromHand", true)]
    #[case::choosing_apart("Effect.Choose.Differ", true)]
    #[case::discarding_matching_cards("Effect.Discard.Filter", true)]
    #[case::discarding_cards_another_player_chooses("Effect.Discard.Chooser", true)]
    #[case::milling("Effect.Mill", true)]
    #[case::scrying("Effect.Scry", true)]
    #[case::looking_at_the_top("Effect.LookAtTop", true)]
    #[case::looking_at_a_hand("Effect.LookAtHand", true)]
    #[case::looking_at_a_face_down_permanent("Effect.LookAtFaceDown", true)]
    #[case::revealing("Effect.Reveal", true)]
    #[case::shuffling("Effect.Shuffle", true)]
    #[case::moving("Effect.Move", true)]
    #[case::moving_to_the_battlefield("Effect.Move.Battlefield", true)]
    #[case::moving_to_the_battlefield_transformed("Effect.Move.Transformed", true)]
    #[case::moving_to_the_battlefield_attached("Effect.Move.Attached", true)]
    #[case::moving_to_the_battlefield_enchanting_only_some("Effect.Move.Enchanting", true)]
    #[case::r5515_power_up_again("Rule.PowerUpAgain", true)]
    #[case::r5515_power_up_limit("Limit.PowerUp", true)]
    #[case::moving_to_the_battlefield_face_down("Effect.Move.FaceDown", true)]
    #[case::moving_in_a_random_order_to_the_top("Effect.Move.Random.Top", true)]
    #[case::searching("Effect.Search", true)]
    #[case::searching_several_zones("Search.Zones", true)]
    #[case::r5517_revealing_until_a_most("RevealUntil.Most", true)]
    #[case::r5517_life_its_own_spell_gives("LifeGain.CausedOwn", true)]
    #[case::r5517_under_your_control_instead("ReplaceBy.UnderYourControl", true)]
    #[case::searching_another_library("Search.Of", true)]
    #[case::searching_another_players_zones("Search.OfZones", true)]
    #[case::sacrificing("Effect.Sacrifice", true)]
    #[case::damage("Effect.DealDamage", true)]
    #[case::gaining_life("Effect.GainLife", true)]
    #[case::losing_life("Effect.LoseLife", true)]
    #[case::mana_costs("Cost.Mana", true)]
    #[case::x_in_a_mana_cost("Cost.Mana.X", true)]
    #[case::paying_life("Cost.PayLife", true)]
    #[case::paying_a_counted_amount_of_life("Cost.PayLife.Counted", false)]
    #[case::r303c_discarding_the_hand_to_cast("Cost.Discard.Hand", true)]
    #[case::r400_revealing_the_hand_to_cast("Cost.RevealFromHand.Hand", true)]
    #[case::r400_following_another_attacker("Rule.Combat.AttackIfAnyAttacks", true)]
    #[case::r5502_blocked_by_one_of_a_kind("Rule.Combat.MustBeBlockedByOne", true)]
    #[case::r303c_sacrificing_all_of_a_kind_to_cast("Cost.Sacrifice.All", true)]
    #[case::once_a_turn("Ability.Limit", true)]
    #[case::other_activators("Ability.By", true)]
    #[case::other_zones("Ability.From", true)]
    #[case::sorcery_timing("Ability.Sorcery", true)]
    #[case::activation_conditions("Ability.Condition", true)]
    #[case::equip("Keyword.Equip", true)]
    #[case::fortify("Keyword.Fortify", true)]
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
    #[case::land_types("Mod.SetLandTypes", true)]
    #[case::replacement_effects("Ability.Replacement", true)]
    #[case::replacing_token_creation("ReplaceWhen.TokenCreation", true)]
    #[case::replacing_damage("ReplaceWhen.Damage", true)]
    #[case::replacing_zone_changes("ReplaceWhen.ZoneChange", true)]
    #[case::replacing_discards("ReplaceWhen.Discarded", true)]
    #[case::replacing_counters_put("ReplaceWhen.CountersPut", true)]
    #[case::replacing_life_gain("ReplaceWhen.LifeGain", true)]
    #[case::replacing_a_loss("ReplaceWhen.LoseGame", true)]
    #[case::r406_replacing_an_exploration("ReplaceWhen.Explores", true)]
    #[case::r406_replacing_a_connive("ReplaceWhen.Connives", true)]
    #[case::r406_replacing_a_proliferation("ReplaceWhen.Proliferates", true)]
    #[case::r5507_preventing_by_counters("ReplaceBy.PreventByCounters", true)]
    #[case::r5507_rolling_a_number_of_dice("RollDice.Count", true)]
    #[case::r5507_preventing_half_the_next("ReplaceBy.PreventHalfNext", true)]
    #[case::r5507_preventing_all_but_of_the_next("ReplaceBy.PreventAllButNext", true)]
    #[case::r4708_replacing_a_scry("ReplaceWhen.Scries", true)]
    #[case::r4708_replacing_a_surveil("ReplaceWhen.Surveils", true)]
    #[case::r5516_replacing_a_copy_of_a_spell("ReplaceWhen.CopiesSpell", true)]
    #[case::r5516_replacing_a_learn("ReplaceWhen.Learns", true)]
    #[case::r5516_steps_its_controller_may_choose("ReplaceBy.MayInstead", true)]
    #[case::r6008_optional_library("ReplaceBy.IntoLibrary.Optional", true)]
    #[case::r6008_words_for_each_player("Effect.ChooseWords.About", true)]
    #[case::r6008_one_card_for_each_color_set("Choose.DifferentColorSets", true)]
    #[case::r6008_most_damage_by_one_source("Amount.MostDamageBySource", true)]
    #[case::r6008_damage_to_from_players("Amount.DamageToFromPlayers", true)]
    #[case::r6008_spend_on_abilities_only("Rule.SpendManaAs.AbilitiesOnly", true)]
    #[case::r4708_more_cards_to_surveil("ReplaceBy.MayAddAmount", true)]
    #[case::replacing_mills("ReplaceWhen.Mill", true)]
    #[case::replacing_draws("ReplaceWhen.Draw", true)]
    #[case::replacing_entering("ReplaceWhen.Enters", true)]
    #[case::multiplying("ReplaceBy.Multiply", true)]
    #[case::adding("ReplaceBy.Add", true)]
    #[case::preventing_with_a_counter_removed("ReplaceBy.PreventRemovingCounter", true)]
    #[case::moving_instead("ReplaceBy.MoveTo", true)]
    #[case::entering_tapped("ReplaceBy.Tapped", true)]
    #[case::entering_with_counters("ReplaceBy.WithCounters", true)]
    #[case::entering_with_counters_and_a_change("ReplaceBy.WithCountersAnd", true)]
    #[case::entering_with_counters_of_two_kinds("ReplaceBy.WithCountersAnd.Also", true)]
    #[case::tribute("ReplaceBy.Tribute", true)]
    #[case::amplify("ReplaceBy.Amplify", true)]
    #[case::entering_with_counters_by_choice("ReplaceBy.MayWithCounters", true)]
    #[case::choosing_as_it_enters("ReplaceBy.Choose", true)]
    #[case::paying_life_or_entering_tapped("ReplaceBy.PayLifeOrTapped", true)]
    #[case::doing_something_instead("ReplaceBy.Instead", true)]
    #[case::modes_in_place_of_an_event("ReplaceBy.Instead.Modal", false)]
    #[case::waiting_in_place_of_an_event("ReplaceBy.Instead.Waits", false)]
    #[case::casting_in_place_of_an_event("ReplaceBy.Instead.Casts", false)]
    #[case::paying_in_place_of_an_event("ReplaceBy.Instead.Pays", false)]
    #[case::paying_life_in_place_of_an_event("ReplaceBy.Instead.PaysLife", true)]
    #[case::tapping_or_untapping("Effect.TapOrUntap", true)]
    #[case::mana_abilities_from_other_zones("Ability.ManaAbility.From", true)]
    #[case::states("Trigger.State", true)]
    #[case::exiling_the_source("Cost.ExileSelf", true)]
    #[case::discarding_the_source("Cost.DiscardSelf", true)]
    #[case::triggers("Ability.Triggered", true)]
    #[case::zone_changes("Trigger.ZoneChange", true)]
    #[case::r5502_attacks_at_a_player_or_their_planeswalkers(
        "Trigger.GroupAttacksAt.Walkers",
        true
    )]
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
    #[case::continuous_for_as_long_as_it_has_a_counter("Duration.WhileHasCounter", true)]
    #[case::control_for_as_long_as_it_is_enchanted("Duration.WhileEnchanted", true)]
    #[case::control_for_as_long_as_they_are_the_monarch("Duration.WhileMonarch", true)]
    #[case::continuous_for_as_long_as_the_source_is_tapped("Duration.WhileSourceTapped", true)]
    #[case::continuous_while_you_control_the_tapped_source(
        "Duration.WhileYouControlSourceTapped",
        true
    )]
    #[case::a_rule_while_you_control_the_tapped_source(
        "Rule.Duration.WhileYouControlSourceTapped",
        false
    )]
    #[case::r5502_a_rule_until_its_controllers_next_combat(
        "Rule.Duration.UntilItsControllersNextCombatEnds",
        true
    )]
    #[case::a_rule_for_as_long_as_you_control_the_source(
        "Rule.Duration.WhileYouControlSource",
        true
    )]
    #[case::pumps("Mod.ModifyPT", true)]
    #[case::granted_keywords("Mod.AddKeyword", true)]
    #[case::protection_from_a_chosen_colour("Mod.AddProtectionFromChosenColor", true)]
    #[case::protection_from_a_chosen_card_type("Mod.AddProtectionFromChosenCardType", true)]
    #[case::shroud("Keyword.Shroud", true)]
    #[case::protection("Keyword.Protection", true)]
    #[case::hexproof_from("Keyword.HexproofFrom", true)]
    #[case::setting_power_and_toughness("Mod.SetPT", true)]
    #[case::switching_power_and_toughness("Mod.SwitchPT", true)]
    #[case::types("Mod.AddType", true)]
    #[case::removed_types("Mod.RemoveType", true)]
    #[case::set_types("Mod.SetTypes", true)]
    #[case::removed_supertypes("Mod.RemoveSupertype", true)]
    #[case::added_supertypes("Mod.AddSupertype", true)]
    #[case::copy_names("Mod.SetName", true)]
    #[case::copy_keeps_this_ability("Copy.Except.ThisAbility", true)]
    #[case::the_ability_kept_as_a_modification("Mod.ThisAbility", true)]
    #[case::r371_a_copy_with_a_starting_loyalty("Mod.SetLoyalty", true)]
    #[case::subtypes("Mod.AddSubtype", true)]
    #[case::colours("Mod.SetColors", true)]
    #[case::chosen_colours("Mod.SetColorsChosen", true)]
    #[case::chosen_land_types("Mod.SetLandTypesChosen", true)]
    #[case::chosen_land_type_added("Mod.AddLandTypeChosen", true)]
    #[case::until_its_controllers_next_untap_step(
        "Duration.UntilItsControllersNextUntapStep",
        true
    )]
    #[case::chosen_creature_types("Mod.SetCreatureTypesChosen", true)]
    #[case::the_type_chosen_as_it_entered("Mod.AddChosenSubtype", true)]
    #[case::the_color_chosen_as_it_entered("Mod.SetChosenColor", true)]
    #[case::the_color_chosen_for_a_commander("Mod.SetCommanderColor", true)]
    #[case::granted_abilities("Mod.GrantAbility", true)]
    #[case::abilities_of_other_cards("Mod.GainAbilitiesOf", true)]
    #[case::lost_abilities("Mod.LoseAbilities", true)]
    #[case::changes_of_control("Mod.SetController", true)]
    #[case::attack_restrictions("Mod.CantAttack", true)]
    #[case::attack_restrictions_on_you("Mod.CantAttackYou", true)]
    #[case::r371_attacks_on_the_owner("Mod.CantAttackOwner", true)]
    #[case::r371_blocks_unless_at_the_owner("Mod.CantBeBlockedUnlessAttackingOwner", true)]
    #[case::block_restrictions("Mod.CantBlock", true)]
    #[case::no_turning_face_up("Mod.CantBeTurnedFaceUp", true)]
    #[case::static_abilities("Ability.Static", true)]
    #[case::characteristic_defining_abilities("Ability.Static.Defining", true)]
    #[case::keywords_given_to_spells("Ability.Static.SpellKeywords", true)]
    #[case::rules_as_effects("Effect.Rule", true)]
    #[case::rules_as_static_abilities("Ability.Rule", true)]
    #[case::attack_requirements("Rule.MustAttack", true)]
    #[case::block_requirements("Rule.MustBlock", true)]
    #[case::provoke("Rule.MustBlockAttacker", true)]
    #[case::combat_quirks("Rule.Combat", true)]
    #[case::ignored_hexproof("Rule.IgnoreHexproof", true)]
    #[case::spells_that_cant_be_countered("Rule.CantBeCountered", true)]
    #[case::no_regeneration("Rule.CantBeRegenerated", true)]
    #[case::no_life_gain("Rule.CantGainLife", true)]
    #[case::no_life_loss_from_damage("Rule.NoLifeLossFromDamage", true)]
    #[case::radiation_gains_life("Rule.RadiationGainsLife", true)]
    #[case::mana_produced("Rule.ManaProduced", true)]
    #[case::may_cast_rather("Rule.MayCastRather", true)]
    #[case::no_drawing("Rule.CantDraw", true)]
    #[case::no_searching("Rule.CantSearch", true)]
    #[case::a_revealed_top("Rule.RevealTop", true)]
    #[case::no_maximum_hand_size("Rule.NoMaxHandSize", true)]
    #[case::a_set_maximum_hand_size("Rule.MaxHandSize", true)]
    #[case::a_changed_maximum_hand_size("Rule.HandSizeChange", true)]
    #[case::no_legend_rule("Rule.NoLegendRule", true)]
    #[case::no_legend_rule_for_some("Rule.NoLegendRuleFor", true)]
    #[case::untapping_in_other_steps("Rule.UntapDuring", true)]
    #[case::untapping_at_most_some_permanents("Rule.UntapLimit", true)]
    #[case::no_draw_step("Rule.SkipDrawStep", true)]
    #[case::triggers_again("Rule.TriggerAgain", true)]
    #[case::damage_that_cant_be_prevented("Rule.DamageCantBePrevented", true)]
    #[case::regenerating("Effect.Regenerate", true)]
    #[case::leaving_combat("Effect.RemoveFromCombat", true)]
    #[case::shields("Effect.Shield", true)]
    #[case::preventing("ReplaceBy.Prevent", true)]
    #[case::preventing_in_part("ReplaceBy.PreventUpTo", true)]
    #[case::preventing_half("ReplaceBy.PreventHalf", true)]
    #[case::preventing_the_next_damage("ReplaceBy.PreventNext", true)]
    #[case::removed_keywords("Mod.RemoveKeyword", true)]
    #[case::locked_keywords("Mod.LockKeyword", true)]
    #[case::conditions("Effect.If", true)]
    #[case::putting_counters("Effect.PutCounters", true)]
    #[case::removing_counters("Effect.RemoveCounters", true)]
    #[case::removing_all_counters("Effect.RemoveAllCounters", true)]
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
    #[case::a_logged_tally("Amount.Tally", true)]
    #[case::r5502_sources_that_dealt_damage("Amount.SourcesDealtDamage", true)]
    #[case::commander_colors("Amount.CommanderColors", true)]
    #[case::turn_number("Amount.TurnNumber", true)]
    #[case::damage_dealt_by_to("Amount.DamageDealtByTo", true)]
    #[case::triggering_targets("Amount.TriggeringTargets", true)]
    #[case::starting_player("Condition.StartingPlayer", true)]
    #[case::among_players("Condition.Among", true)]
    #[case::cast_for_a_way("Amount.CastWith", true)]
    #[case::colors_spent("Amount.ColorsSpent", true)]
    #[case::destroyed_this_way("Amount.Destroyed", true)]
    #[case::distinct_values("Amount.Distinct", true)]
    #[case::permanents_graved("Amount.PermanentsGraved", true)]
    #[case::commander_casts("Amount.CommanderCasts", true)]
    #[case::speed("Amount.Speed", true)]
    #[case::may("Effect.May", true)]
    #[case::together("Effect.Together", true)]
    #[case::choosing("Effect.Choose", true)]
    #[case::choosing_at_random("Effect.ChooseAtRandom", true)]
    #[case::choosing_a_value("Effect.ChooseValue", true)]
    #[case::modes("Effect.Modal", true)]
    #[case::repeating_for_each("Effect.RepeatEach", true)]
    #[case::rolling_dice("Effect.RollDice", true)]
    #[case::targets_for_each_player("Target.PerPlayer", true)]
    #[case::targets_that_share_a_property("Target.Together", true)]
    #[case::targets_another_player_chooses("Target.ChosenBy", true)]
    #[case::flipping_coins("Effect.FlipCoin", true)]
    #[case::modes_at_random("Effect.Modal.AtRandom", true)]
    #[case::modes_not_chosen_twice_in_a_turn("Effect.Modal.OnceEachTurn", true)]
    #[case::e14c_modes_target_different_players("Effect.Modal.DifferentPlayers", true)]
    #[case::modes_not_chosen_twice_ever("Effect.Modal.OnceEver", true)]
    #[case::modes_with_costs("Effect.Modal.ModeCosts", true)]
    #[case::modes_with_an_extra_cost("Effect.Modal.ExtraModeCost", true)]
    #[case::r302_modes_counted_in_pawprints("Effect.Modal.Pawprints", true)]
    #[case::r302_a_spell_that_cant_be_copied("Rule.CantBeCopied", true)]
    #[case::r5516_damage_kept_through_cleanup("Rule.KeepsDamage", true)]
    #[case::r400_a_spell_cast_without_mana("Rule.CantSpendMana", true)]
    #[case::every_mode_under_a_condition("Effect.Modal.AllIf", true)]
    #[case::voting("Effect.Vote", true)]
    #[case::a_kept_number("Effect.Let", true)]
    #[case::modes_chosen_by_another_player("Effect.Modal.Who", true)]
    #[case::c10_vp_planechase("Effect.Planechase", true)]
    #[case::adding_mana("Effect.AddMana", true)]
    #[case::tapping_permanents("Effect.Tap", true)]
    #[case::untapping_permanents("Effect.Untap", true)]
    #[case::attaching("Effect.Attach", true)]
    #[case::enchanting("Keyword.Enchant", true)]
    #[case::enchanting_cards_elsewhere("Keyword.Enchant.Elsewhere", false)]
    #[case::enchanting_cards_in_a_graveyard("Keyword.Enchant.Graveyard", true)]
    #[case::winning_the_game("Effect.WinGame", true)]
    #[case::creating_tokens("Effect.CreateToken", true)]
    #[case::creating_copies("Effect.CreateToken.Copy", true)]
    #[case::entering_as_a_copy("ReplaceBy.AsCopy", true)]
    #[case::an_exception_the_copy_cannot_hold("Copy.Except.SwitchPT", false)]
    #[case::copying_spells("Effect.CopySpell", true)]
    #[case::copying_abilities("Effect.CopyAbility", true)]
    #[case::changing_targets("Effect.ChangeTargets", true)]
    #[case::becoming_a_copy("Effect.BecomeCopy", true)]
    #[case::a_copy_until_your_next_turn("Copy.Duration.UntilYourNextTurn", true)]
    #[case::a_copy_that_ends_with_another_duration("Copy.Duration.UntilEndOfCombat", false)]
    #[case::storming("Keyword.Storm", true)]
    #[case::sacrificing_the_source("Cost.SacrificeSelf", true)]
    #[case::sacrificing_a_permanent("Cost.Sacrifice", true)]
    #[case::paying_by_sacrificing_the_source("Effect.Pay.SacrificeSelf", true)]
    #[case::paying_by_exiling_the_source("Effect.Pay.ExileSelf", true)]
    #[case::paying_by_discarding_the_source("Effect.Pay.DiscardSelf", false)]
    #[case::mana_abilities_of_several_mana_or_with_more_effects(
        "Ability.ManaAbility.Complex",
        true
    )]
    #[case::mana_abilities_the_engine_activates_at_priority_and_resolves_at_once(
        "Ability.ManaAbility.Other",
        true
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
    #[case::the_ring_tempting("Effect.RingTempts", true)]
    #[case::the_ring_tempting_triggers("Trigger.RingTempts", true)]
    #[case::the_ring_bearer("Pred.RingBearer", true)]
    #[case::did_this_turn("Pred.DidThisTurn", true)]
    #[case::r379_blocked_either_way("Pred.BlockedThisTurn", true)]
    #[case::r379_entered_since_your_last_turn("Pred.EnteredSinceLastTurn", true)]
    #[case::r379_hurt_since_your_last_turn("Condition.CombatDamagedSinceLastTurn", true)]
    #[case::adventure_cards("Pred.HasAdventure", true)]
    #[case::amassing("Effect.Amass", true)]
    #[case::r5508_empowering("Effect.Empower", true)]
    #[case::r5508_choosing_within("Effect.ChooseWithin", true)]
    #[case::monstrosity("Effect.Monstrosity", true)]
    #[case::becoming_a_level("Effect.BecomeLevel", true)]
    #[case::adapting("Effect.Adapt", true)]
    #[case::bolstering("Effect.Bolster", true)]
    #[case::fabricating("Effect.Fabricate", true)]
    #[case::learning("Effect.Learn", true)]
    #[case::performing("Effect.Performed", true)]
    #[case::crewing("Effect.Crewed", true)]
    #[case::exploring("Effect.Explore", true)]
    #[case::conniving("Effect.Connive", true)]
    #[case::exiling_until_a_card_matches("Effect.ExileUntil", true)]
    #[case::exiling_until_a_total_mana_value("ExileUntil.Total", true)]
    #[case::revealing_until_a_card_matches("Effect.RevealUntil", true)]
    #[case::exiling_the_top_cards("Effect.ExileTop", true)]
    #[case::exiling_face_down("Effect.ExileHidden", true)]
    #[case::encoding("Effect.Encode", true)]
    #[case::copying_a_card("Effect.CopyCard", true)]
    #[case::manifesting("Effect.Manifest", true)]
    #[case::cloaking("Effect.Cloak", true)]
    #[case::r5509_face_down_pile("Effect.FaceDownPile", true)]
    #[case::r5509_become_foretold("Effect.BecomeForetold", true)]
    #[case::r5509_flagbearers("Rule.Flagbearers", true)]
    #[case::casting_during_a_resolution("Effect.CastFromZone", true)]
    #[case::casting_during_a_resolution_transformed("Effect.CastFromZone.Transformed", true)]
    #[case::cascading("Keyword.Cascade", true)]
    #[case::convoking("Keyword.Convoke", true)]
    #[case::delving("Keyword.Delve", true)]
    #[case::time_travelling("Effect.TimeTravel", true)]
    #[case::r372_gaining_suspend("Effect.GainSuspend", true)]
    #[case::extra_turns("Effect.ExtraTurn", true)]
    #[case::added_phases("Effect.AddPhase", true)]
    #[case::ending_the_turn("Effect.EndTurn", true)]
    #[case::cant_activate("Rule.Cant.Activate", true)]
    #[case::cant_be_targeted("Rule.Cant.BeTargeted", true)]
    #[case::r377_cant_be_targeted_by_walls_only("Rule.Cant.BeTargetedByWallsOnly", true)]
    #[case::cant_play_land("Rule.Cant.PlayLand", true)]
    #[case::cant_sacrifice("Rule.Cant.Sacrifice", true)]
    #[case::cant_put_counter("Rule.Cant.PutCounter", true)]
    #[case::cant_put_counter_of("Rule.Cant.PutCounterOf", true)]
    #[case::cant_trigger("Rule.Cant.Trigger", true)]
    #[case::cant_attach("Rule.Cant.Attach", true)]
    #[case::cant_be_attached_by("Rule.Cant.BeAttachedBy", true)]
    #[case::cant_discard("Rule.Cant.Discard", true)]
    #[case::cant_exile("Rule.Cant.Exile", true)]
    #[case::cant_transform("Rule.Cant.Transform", true)]
    #[case::r301_cant_phase_out("Rule.Cant.PhaseOut", true)]
    #[case::cant_skip_step("Rule.Cant.SkipStep", true)]
    #[case::cant_skip_turn("Rule.Cant.SkipTurn", true)]
    #[case::cant_skip_combat("Rule.Cant.SkipCombat", true)]
    #[case::cant_cast("Rule.Cant.Cast", true)]
    #[case::cant_attack("Rule.Cant.Attack", true)]
    #[case::cant_activate_kind("Rule.Cant.ActivateKind", true)]
    #[case::a_choice_of_mana("Effect.AddMana.OneOf", true)]
    #[case::a_choice_of_lists_of_mana("Effect.AddMana.OneOfLists", true)]
    #[case::paying_one_of_several_costs("Cost.OneOf.Pay", true)]
    #[case::choosing_between_costs_elsewhere("Cost.OneOf.Elsewhere", false)]
    #[case::mana_of_the_chosen_color("Effect.AddMana.ChosenColor", true)]
    #[case::mana_of_a_type_or_the_chosen_color("Effect.AddMana.OrChosenColor", true)]
    #[case::mana_restricted_to_what_it_may_be_spent_on("Effect.AddMana.Restricted", true)]
    #[case::mana_that_keeps_a_spell_from_being_countered("Effect.AddMana.Uncounterable", true)]
    #[case::mana_kept_past_the_step("Effect.AddMana.Lasts", true)]
    #[case::mana_that_sets_something_off_when_spent("Effect.AddMana.WhenSpent", true)]
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
    #[case::gn1m_putting_counters_as_a_cost("Cost.PutCounters", true)]
    #[case::gn1m_exerting_as_a_cost("Cost.Exert", true)]
    #[case::gn1m_untapping_as_a_cost("Cost.UntapPermanents", true)]
    #[case::gn1m_removing_counters_among_as_a_cost("Cost.RemoveCountersAmong", true)]
    #[case::gn1m_a_counted_sacrifice_of_an_ability("Cost.Sacrifice.Counted.Activated", true)]
    #[case::gn1m_a_counted_sacrifice_elsewhere("Cost.Sacrifice.Counted", false)]
    #[case::exiling_from_a_graveyard_as_a_cost("Cost.ExileFromGraveyard", true)]
    #[case::returning_to_a_hand_as_a_cost("Cost.ReturnToHand", true)]
    #[case::dealing_damage_triggers("Trigger.DealsDamage", true)]
    #[case::life_gain_triggers("Trigger.LifeGained", true)]
    #[case::life_loss_triggers("Trigger.LifeLost", true)]
    #[case::draw_triggers("Trigger.Drew", true)]
    #[case::r4708_first_draws_revealed("Trigger.FirstDrawRevealed", true)]
    #[case::r4708_reveals_of_first_draws("Rule.RevealFirstDraw", true)]
    #[case::discard_triggers("Trigger.Discarded", true)]
    #[case::tap_triggers("Trigger.Tapped", true)]
    #[case::untap_triggers("Trigger.Untapped", true)]
    #[case::mana_triggers("Trigger.TappedForMana", true)]
    #[case::mana_triggers_of_the_chosen_color("Trigger.TappedForMana.ChosenColor", true)]
    #[case::block_triggers("Trigger.Blocks", true)]
    #[case::blocked_triggers("Trigger.BecomesBlocked", true)]
    #[case::w2e_unblocked_triggers("Trigger.Unblocked", true)]
    #[case::w2e_copy_triggers("Trigger.SpellCopied", true)]
    #[case::group_attack_triggers("Trigger.GroupAttacks", true)]
    #[case::group_attack_at_triggers("Trigger.GroupAttacksAt", true)]
    #[case::counter_triggers("Trigger.CountersPut", true)]
    #[case::counter_removal_triggers("Trigger.CountersRemoved", true)]
    #[case::e16c_player_counter_triggers("Trigger.PlayerCountersPut", true)]
    #[case::chapter_triggers("Trigger.Chapter", true)]
    #[case::attach_triggers("Trigger.Attached", true)]
    #[case::dk26_unattach_triggers("Trigger.Unattached", true)]
    #[case::cycle_triggers("Trigger.Cycled", true)]
    #[case::becoming_the_monarch_triggers("Trigger.BecomesMonarch", true)]
    #[case::proliferating_triggers("Trigger.Proliferated", true)]
    #[case::exploring_triggers("Trigger.Explored", true)]
    #[case::conniving_triggers("Trigger.Connived", true)]
    #[case::exerted_triggers("Trigger.Exerted", true)]
    #[case::amassing_triggers("Trigger.Amassed", true)]
    #[case::becoming_monstrous_triggers("Trigger.BecomesMonstrous", true)]
    #[case::becoming_crewed_triggers("Trigger.BecomesCrewed", true)]
    #[case::becoming_saddled_triggers("Trigger.BecomesSaddled", true)]
    #[case::r288_final_chapter_triggers("Trigger.FinalChapter", true)]
    #[case::becoming_a_level_triggers("Trigger.BecomesLevel", true)]
    #[case::turning_face_up_triggers("Trigger.TurnedFaceUp", true)]
    #[case::adapting_triggers("Trigger.Adapted", true)]
    #[case::bolstering_triggers("Trigger.Bolstered", true)]
    #[case::expend_triggers("Trigger.Expend", true)]
    #[case::w11c_crime_triggers("Trigger.W11cCrime", true)]
    #[case::surveil_triggers("Trigger.Surveilled", true)]
    #[case::zone_changes_from_elsewhere("Trigger.ZoneChangeExcept", true)]
    #[case::resolved_to_graveyard("Trigger.ResolvedToGraveyard", true)]
    #[case::batch_triggers("Trigger.Batch", true)]
    #[case::nth_triggers("Trigger.Nth", true)]
    #[case::caused_triggers("Trigger.Caused", true)]
    #[case::r3p1_actions_taken_by_a_player("Trigger.Did", true)]
    #[case::r3p1_beginning_the_game_on_the_battlefield("Ability.OpeningHand", true)]
    #[case::revealed_from_the_opening_hand("Ability.OpeningReveal", true)]
    #[case::r3p1_untapping_the_source_as_a_cost("Cost.Untap", true)]
    #[case::r5g_sacrificing_any_number("Effect.SacrificeUpTo", true)]
    #[case::event_amounts("Amount.Triggering", true)]
    #[case::if_you_do("Condition.Performed", true)]
    #[case::a_chosen_word("Condition.ChosenWord", true)]
    #[case::e15f_entered_as_a_word("Condition.EnteredAs", true)]
    #[case::e15f_a_word_chosen_as_it_enters("ReplaceBy.ChooseWord", true)]
    #[case::if_you_do_after_another_effect("Condition.Performed.Other", false)]
    #[case::the_monarch("Condition.Monarch", true)]
    #[case::the_monarch_as_the_turn_began("Condition.MonarchAtTurnStart", true)]
    #[case::the_citys_blessing("Condition.CitysBlessing", true)]
    #[case::rmt6_an_enduring_story("Condition.EnduringStory", true)]
    #[case::r379_how_players_voted("Condition.VotedAs", true)]
    #[case::r379_greatest_among_spells_cast("Amount.GreatestCast", true)]
    #[case::rmt6_a_draft_rule("Rule.Draft", true)]
    #[case::max_speed("Condition.MaxSpeed", true)]
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
    #[case::r377_blocked_by_all("Mod.BlockedByAll", true)]
    #[case::goaded_creatures("Mod.Goaded", true)]
    #[case::looking_at_the_top_any_time("Rule.LookAtTopAnyTime", true)]
    #[case::looking_at_face_down_cards_in_exile("Rule.LookAtExiled", true)]
    #[case::additional_land_plays("Rule.AdditionalLandPlays", true)]
    #[case::commander_eligibility("Rule.CanBeCommander", true)]
    #[case::r5g_any_number_in_a_deck("Rule.AnyNumberInDeck", true)]
    #[case::fb27_up_to_in_a_deck("Rule.UpToInDeck", true)]
    #[case::w17b_untargetable_graveyards("Rule.GraveyardCardsUntargetable", true)]
    #[case::w17b_cant_lose("Rule.CantLoseGame", true)]
    #[case::w17b_cant_cast("Rule.CantCastSpells", true)]
    #[case::w17b_toughness_damage("Rule.ToughnessAssignsDamage", true)]
    #[case::r377_lethal_by_power("Rule.LethalByPower", true)]
    #[case::r377_devotion_increased("Rule.DevotionIncreased", true)]
    #[case::r377_damage_as_infect("Rule.DamageAsInfect", true)]
    #[case::r377_venture_limit("Rule.VentureLimit", true)]
    #[case::w17b_counters_remain("Rule.CountersRemain", true)]
    #[case::partner("Keyword.Partner", true)]
    #[case::partner_with("Keyword.PartnerWith", true)]
    #[case::partner_group("Keyword.PartnerGroup", true)]
    #[case::doctors_companion("Keyword.DoctorsCompanion", true)]
    #[case::choose_a_background("Keyword.ChooseABackground", true)]
    #[case::unearth("Keyword.Unearth", true)]
    #[case::escape("Keyword.Escape", true)]
    #[case::rebound("Keyword.Rebound", true)]
    #[case::paradigm("Keyword.Paradigm", true)]
    #[case::aftermath("Keyword.Aftermath", true)]
    #[case::dash("Keyword.Dash", true)]
    #[case::bestow("Keyword.Bestow", true)]
    #[case::dredge("Keyword.Dredge", true)]
    #[case::dredging("ReplaceBy.Dredge", true)]
    #[case::blitz("Keyword.Blitz", true)]
    #[case::prowl("Keyword.Prowl", true)]
    #[case::mayhem("Keyword.Mayhem", true)]
    #[case::disturb("Keyword.Disturb", true)]
    #[case::a_conditional_free_cast("Ability.FreeCast", true)]
    #[case::an_alternative_cost_of_the_table("Ability.AltCast", true)]
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
    #[case::bargain("Keyword.Bargain", true)]
    #[case::conspire("Keyword.Conspire", true)]
    #[case::demonstrate("Keyword.Demonstrate", true)]
    #[case::hideaway("Keyword.Hideaway", true)]
    #[case::gift("Keyword.Gift", true)]
    #[case::offspring("Keyword.Offspring", true)]
    #[case::sacrificing_as_it_enters("ReplaceBy.Devour", true)]
    #[case::level_up("Keyword.LevelUp", true)]
    #[case::outlast("Keyword.Outlast", true)]
    #[case::saddle("Keyword.Saddle", true)]
    #[case::saddled("Keyword.Saddled", true)]
    #[case::exert("Keyword.Exert", true)]
    #[case::macro_keyword("Keyword.Macro", true)]
    #[case::station("Keyword.Station", true)]
    #[case::ravenous("Keyword.Ravenous", true)]
    #[case::riot("Keyword.Riot", true)]
    #[case::decayed("Keyword.Decayed", true)]
    #[case::choosing_a_counter_or_haste_as_it_enters("ReplaceBy.Riot", true)]
    #[case::choosing_the_kind_of_a_counter_as_it_enters("ReplaceBy.WithCounterOf", true)]
    #[case::choosing_different_counters_as_it_enters("ReplaceBy.WithCountersOf", true)]
    #[case::copies_for_the_other_defenders("Effect.MyriadCopies", true)]
    #[case::jump_start("Keyword.JumpStart", true)]
    #[case::retrace("Keyword.Retrace", true)]
    #[case::embalm("Keyword.Embalm", true)]
    #[case::eternalize("Keyword.Eternalize", true)]
    #[case::improvise("Keyword.Improvise", true)]
    #[case::affinity("Keyword.Affinity", true)]
    #[case::typecycling("Keyword.Typecycling", true)]
    #[case::for_mirrodin("Keyword.ForMirrodin", true)]
    #[case::start_your_engines("Keyword.StartYourEngines", true)]
    #[case::job_select("Keyword.JobSelect", true)]
    #[case::equip_to("Keyword.EquipTo", true)]
    #[case::split_second("Keyword.SplitSecond", true)]
    #[case::undaunted("Keyword.Undaunted", true)]
    #[case::coming_under_control_since_the_last_turn("Condition.NewlyControlled", true)]
    #[case::a_class_level("Condition.Level", true)]
    #[case::r5513_attacked_this_turn_by("Condition.AttackedThisTurnBy", true)]
    #[case::r5513_exploited("Condition.Exploited", true)]
    #[case::a_search_of_the_library_this_way("Condition.Searched", true)]
    #[case::a_step_of_the_turn("Condition.Step", true)]
    #[case::before_attackers("Condition.BeforeAttackers", true)]
    #[case::in_combat("Condition.InCombat", true)]
    #[case::the_first_combat("Condition.FirstCombat", true)]
    #[case::counting_players("Amount.Players", true)]
    #[case::the_lowest_life_total("Amount.LowestLife", true)]
    #[case::the_highest_life_total("Amount.HighestLife", true)]
    #[case::the_fewest_cards_in_a_hand("Amount.LowestHand", true)]
    #[case::the_life_lost_this_way("Amount.LifeLostThisWay", true)]
    #[case::the_damage_dealt_this_way("Amount.DamageDealtThisWay", true)]
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
    #[case::prototype("Keyword.Prototype", true)]
    #[case::assist("Keyword.Assist", true)]
    #[case::craft("Keyword.Craft", true)]
    #[case::gravestorm("Keyword.Gravestorm", true)]
    #[case::sunburst("Keyword.Sunburst", true)]
    #[case::afterlife("Keyword.Afterlife", true)]
    #[case::horsemanship("Keyword.Horsemanship", true)]
    #[case::phasing("Keyword.Phasing", true)]
    #[case::spectacle("Keyword.Spectacle", true)]
    #[case::freerunning("Keyword.Freerunning", true)]
    #[case::umbra_armor("Keyword.UmbraArmor", true)]
    #[case::cipher("Keyword.Cipher", true)]
    #[case::haunt("Keyword.Haunt", true)]
    #[case::flash_sacrifice("Keyword.FlashSacrifice", true)]
    #[case::cast_off_sorcery_timing("Condition.CastOffSorceryTiming", true)]
    #[case::graft("Keyword.Graft", true)]
    #[case::afflict("Keyword.Afflict", true)]
    #[case::replicate("Keyword.Replicate", true)]
    #[case::renown("Keyword.Renown", true)]
    #[case::exploit("Keyword.Exploit", true)]
    #[case::soulbond("Keyword.Soulbond", true)]
    #[case::pairing("Effect.Pair", true)]
    #[case::emerge("Keyword.Emerge", true)]
    #[case::harmonize("Keyword.Harmonize", true)]
    #[case::read_ahead("Keyword.ReadAhead", true)]
    #[case::recover("Keyword.Recover", true)]
    #[case::ripple("Keyword.Ripple", true)]
    #[case::epic("Keyword.Epic", true)]
    #[case::banding("Keyword.Banding", true)]
    #[case::bands_with_other("Keyword.BandsWithOther", true)]
    #[case::splice("Keyword.Splice", true)]
    #[case::becoming_renowned_triggers("Trigger.BecomesRenowned", true)]
    #[case::exploiting_triggers("Trigger.Exploits", true)]
    #[case::replacing_destruction("ReplaceWhen.Destroyed", true)]
    #[case::saving_by_umbra_armor("ReplaceBy.UmbraArmor", true)]
    #[case::pairing_attackers_with_players("Rule.MustAttackPlayers", true)]
    #[case::r5502_attacking_with_at_least_one("Rule.MustAttackWithOne", true)]
    #[case::taxing_attackers("Rule.AttackTax", true)]
    #[case::taxing_blockers("Rule.BlockTax", true)]
    #[case::w8a_attack_limit("Rule.AttackLimit", true)]
    #[case::w10a_attack_limit_at("Rule.AttackLimitAt", true)]
    #[case::w8a_block_limit("Rule.BlockLimit", true)]
    #[case::w14a_block_limit_each("Rule.BlockLimitEach", true)]
    #[case::squad("Keyword.Squad", true)]
    #[case::x_costs(X_COST, true)]
    #[case::x_read_after_resolving("Amount.X.Elsewhere", false)]
    #[case::r379_x_of_an_alternative_cost("Cost.Alternative.X", true)]
    #[case::paying_x_in_an_effect("Effect.Pay.X", true)]
    #[case::paying_energy_of_a_chosen_amount_in_an_effect("Effect.Pay.X.Energy", true)]
    #[case::paying_a_part_that_counts_x_in_an_effect("Effect.Pay.X.Parts", false)]
    #[case::faces(FACES, true)]
    #[case::the_prepared_cast(PREPARE, true)]
    #[case::variable_stats(VARIABLE_STATS, true)]
    fn the_engine_runs_what_it_runs_today(#[case] name: &str, #[case] runs: bool) {
        assert_eq!(caps().runs(name), runs);
    }
}
