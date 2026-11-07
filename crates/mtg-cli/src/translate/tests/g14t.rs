//! Goal: round 14 pack t reads the player facts and trigger shapes of the tail: the opponents picked by their hands, creatures,
//! attacks or discards, counts of what happened this turn, enchanted-player statics and triggers, the attacker named inside its
//! own trigger, and lasting changes that wait on a counter.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

#[rstest]
#[case::each_opponent_attacking_the_enchanted_player(
    "Enchantment Aura Curse",
    r#"K:Enchant:Player
SVar:AttachAILogic:Curse
T:Mode$ AttackersDeclared | AttackedTarget$ Player.EnchantedBy | Execute$ TrigDraw | TriggerZones$ Battlefield | TriggerDescription$ Whenever enchanted player is attacked, draw a card. Each opponent attacking that player does the same.
SVar:TrigDraw:DB$ Draw | Defined$ You | SubAbility$ DBDraw
SVar:DBDraw:DB$ Draw | Defined$ TriggeredAttackingPlayer.Opponent+controlsCreature.attacking Player.EnchantedBy
Oracle:Enchant player\nWhenever enchanted player is attacked, you draw a card. Each opponent attacking that player does the same."#,
    r#""PlayersWhere""#
)]
#[case::more_cards_in_hand_than_the_asker(
    "Creature Angel Detective",
    r#"K:Flying
K:Vigilance
T:Mode$ Phase | Phase$ Upkeep | TriggerZones$ Battlefield | Execute$ TrigInvestigate | ValidPlayer$ You | TriggerDescription$ At the beginning of your upkeep, investigate once for each opponent who has more cards in hand than you. (To investigate, create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.")
SVar:TrigInvestigate:DB$ Investigate | Num$ X
SVar:X:PlayerCountOpponents$HasPropertyHasCardsInHand_Card_GTY
SVar:Y:Count$CardsInYourHand
Oracle:Flying, vigilance\nAt the beginning of your upkeep, investigate once for each opponent who has more cards in hand than you. (To investigate, create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.")"#,
    r#""op":"Gt","right":{"Count":[{"In":"Hand"},{"OwnedBy":"Asker"}]}"#
)]
#[case::opponents_with_a_big_creature(
    "Enchantment Creature Saga Samurai",
    r#"K:Chapter:4:DBExile,DBEffect,DBEffect,DBToken
SVar:DBExile:DB$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Artifact.OppCtrl,Enchantment.OppCtrl,Creature.OppCtrl+tapped | TgtPrompt$ Select artifact, enchantment, or tapped creature an opponent controls | SpellDescription$ Exile target artifact, enchantment, or tapped creature an opponent controls.
SVar:DBEffect:DB$ Effect | StaticAbilities$ YojimboCantAttack | Duration$ UntilYourNextTurn | SpellDescription$ Until your next turn, creatures can't attack you unless their controller pays {2} for each of those creatures.
SVar:YojimboCantAttack:Mode$ CantAttackUnless | ValidCard$ Creature | Target$ You | Cost$ 2 | Description$ Until your next turn, creatures can't attack you unless their controller pays {2} for each of those creatures.
SVar:DBToken:DB$ Token | TokenAmount$ PlayerCountOpponents$HasPropertycontrolsCreature.powerGE4 | TokenScript$ c_a_treasure_sac | TokenOwner$ You | SpellDescription$ Create X Treasure tokens, where X is the number of opponents who control a creature with power 4 or greater.
K:Vigilance
Oracle:(As this Saga enters and after your draw step, add a lore counter. Sacrifice after IV.)\nI — Exile target artifact, enchantment, or tapped creature an opponent controls.\nII, III — Until your next turn, creatures can't attack you unless their controller pays {2} for each of those creatures.\nIV — Create X Treasure tokens, where X is the number of opponents who control a creature with power 4 or greater.\nVigilance"#,
    r#""to":4"#
)]
#[case::creatures_that_attacked_this_turn(
    "Instant",
    r#"S:Mode$ ReduceCost | ValidCard$ Card.Self | Type$ Spell | Amount$ X | EffectZone$ All | Description$ This spell costs {1} less to cast for each creature that attacked this turn.
SVar:X:PlayerCountPlayers$AttackersDeclared
A:SP$ Draw | NumCards$ 3 | SpellDescription$ Draw three cards.
Oracle:This spell costs {1} less to cast for each creature that attacked this turn.\nDraw three cards."#,
    r#""Tally":{"event":"Attacked","filter":[],"who":"EachPlayer"}"#
)]
#[case::damage_dealt_to_you_this_turn(
    "Instant",
    r#"A:SP$ GainLife | Defined$ You | LifeAmount$ X | SubAbility$ Dmg | SpellDescription$ You gain life equal to the damage dealt to you this turn. CARDNAME deals damage to target creature you control equal to the damage dealt to you this turn.
SVar:Dmg:DB$ DealDamage | ValidTgts$ Creature.YouCtrl | TgtPrompt$ Select target creature you control | NumDmg$ X
SVar:X:PlayerCountPropertyYou$DamageThisTurn
Oracle:You gain life equal to the damage dealt to you this turn. Simulacrum deals damage to target creature you control equal to the damage dealt to you this turn."#,
    r#""Tallied":{"what":"DamageTaken","who":"You"}"#
)]
#[case::opponents_two_spells_this_turn(
    "Instant",
    r#"S:Mode$ ReduceCost | ValidCard$ Card.Self | CheckSVar$ OppCastThisTurn | Type$ Spell | Amount$ 1 | Color$ U | EffectZone$ All | Description$ This spell costs {U} less to cast if an opponent cast two or more spells this turn.
A:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | SpellDescription$ Counter target spell.
SVar:OppCastThisTurn:PlayerCountOpponents$ConditionGE2 SpellsCastThisTurn
Oracle:This spell costs {U} less to cast if an opponent cast two or more spells this turn.\nCounter target spell."#,
    r#""SpellsCast""#
)]
#[case::highest_cards_drawn_by_an_opponent(
    "Instant Trap",
    r#"S:Mode$ AlternativeCost | ValidSA$ Spell.Self | EffectZone$ All | Cost$ R | CheckSVar$ Y | SVarCompare$ GE3 | Description$ If an opponent drew three or more cards this turn, you may pay {R} rather than pay this spell's mana cost.
A:SP$ DealDamage | ValidTgts$ Player | NumDmg$ X | SpellDescription$ CARDNAME deals damage to target player equal to the number of cards in that player's hand.
SVar:X:TargetedPlayer$CardsInHand
SVar:Y:PlayerCountOpponents$HighestCardsDrawn
Oracle:If an opponent drew three or more cards this turn, you may pay {R} rather than pay this spell's mana cost.\nRuneflare Trap deals damage to target player equal to the number of cards in that player's hand."#,
    r#""Tallied":{"what":"Drawn","who":"Opponents"}"#
)]
#[case::cards_in_the_opponents_hands(
    "Enchantment Creature Demon",
    r#"K:Flying
S:Mode$ Continuous | AffectedDefined$ Self | AddPower$ -X | AddToughness$ -X | Description$ CARDNAME gets -1/-1 for each card in your opponents' hands.
SVar:X:PlayerCountOpponents$CardsInHand
SVar:NeedsToPlayVar:X LE3
T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigDiscard | TriggerDescription$ At the beginning of your upkeep, each player discards a card.
SVar:TrigDiscard:DB$ Discard | Defined$ Player | NumCards$ 1 | Mode$ TgtChoose
Oracle:Flying\nEnemy of Enlightenment gets -1/-1 for each card in your opponents' hands.\nAt the beginning of your upkeep, each player discards a card."#,
    r#""Count":[{"In":"Hand"},{"OwnedBy":"Opponents"}]"#
)]
#[case::defending_player_of_the_spawning_attack(
    "Legendary Creature Human Rebel",
    r#"T:Mode$ Attacks | ValidCard$ Card.Self | TriggerZones$ Battlefield | Execute$ TrigImmediateTrig | TriggerDescription$ Nitro-9 — Whenever CARDNAME attacks, you may sacrifice an artifact. When you do, put a +1/+1 counter on CARDNAME, then it fights up to one target creature defending player controls.
SVar:TrigImmediateTrig:AB$ ImmediateTrigger | Cost$ Sac<1/Artifact> | Execute$ TrigPutCounter | TriggerDescription$ When you do, put a +1/+1 counter on CARDNAME, then it fights up to one target creature defending player controls.
SVar:TrigPutCounter:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1 | SubAbility$ DBFight
SVar:DBFight:DB$ Fight | Defined$ Self | TargetMin$ 0 | TargetMax$ 1 | ValidTgts$ Creature.ControlledBy Spawner>TriggeredDefendingPlayer | TgtPrompt$ Select up to one target creature defending player controls
K:Doctor's companion
SVar:HasAttackEffect:TRUE
Oracle:Nitro-9 — Whenever Ace, Fearless Rebel attacks, you may sacrifice an artifact. When you do, put a +1/+1 counter on Ace, Fearless Rebel, then it fights up to one target creature defending player controls.\nDoctor's companion (You can have two commanders if the other is the Doctor.)"#,
    r#"{"ControlledBy":"Defending"}"#
)]
#[case::base_power_for_as_long_as_it_has_a_counter(
    "Creature Bird Wizard",
    r#"K:Flying
T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ TrigPutCounter | TriggerZones$ Battlefield | OptionalDecider$ You | TriggerDescription$ At the beginning of your upkeep, you may put a feather counter on target creature. If you do, that creature has base power and toughness 3/1 and has flying for as long as it has a feather counter on it.
SVar:TrigPutCounter:DB$ PutCounter | ValidTgts$ Creature | CounterType$ FEATHER | CounterNum$ 1 | RememberCards$ True | SubAbility$ DBEffect
SVar:DBEffect:DB$ Effect | RememberObjects$ Remembered | StaticAbilities$ MimeomancerStatic | ForgetOnMoved$ Battlefield | ForgetCounter$ FEATHER | Duration$ Permanent | SubAbility$ DBCleanup
SVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True
SVar:MimeomancerStatic:Mode$ Continuous | Affected$ Card.IsRemembered | SetPower$ 3 | SetToughness$ 1 | AddKeyword$ Flying | Description$ CARDNAME is 3/1 and has flying for as long as it has a feather counter on it.
Oracle:Flying\nAt the beginning of your upkeep, you may put a feather counter on target creature. If you do, that creature has base power and toughness 3/1 and has flying for as long as it has a feather counter on it."#,
    r#""WhileHasCounter":{"Other":"feather"}"#
)]
#[case::a_land_that_is_an_elemental_creature(
    "Creature Elemental",
    r#"K:Trample
T:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | CombatDamage$ True | OptionalDecider$ You | Execute$ TrigPutCounter | TriggerZones$ Battlefield | TriggerDescription$ Whenever CARDNAME deals combat damage to a player, you may choose any number of target lands you control and put an awakening counter on each of them. Each of those lands is an 8/8 green Elemental creature for as long as it has an awakening counter on it. They're still lands.
SVar:TrigPutCounter:DB$ PutCounter | ValidTgts$ Land.YouCtrl | TargetMin$ 0 | TargetMax$ X | TgtPrompt$ Select lands you control | CounterType$ AWAKENING | CounterNum$ 1 | SubAbility$ DBEffect
SVar:X:Count$Valid Land.YouCtrl
SVar:DBEffect:DB$ Effect | RememberObjects$ Targeted | StaticAbilities$ TangleStatic | ForgetOnMoved$ Battlefield | ForgetCounter$ AWAKENING | Duration$ Permanent
SVar:TangleStatic:Mode$ Continuous | Affected$ Card.IsRemembered | SetPower$ 8 | SetToughness$ 8 | AddType$ Creature & Elemental | RemoveCreatureTypes$ True | SetColor$ Green | Description$ Each of those lands is an 8/8 green Elemental creature for as long as it has an awakening counter on it. They're still lands.
SVar:MustBeBlocked:True
Oracle:Trample\nWhenever Liege of the Tangle deals combat damage to a player, you may choose any number of target lands you control and put an awakening counter on each of them. Each of those lands is an 8/8 green Elemental creature for as long as it has an awakening counter on it. They're still lands."#,
    r#"{"SetCreatureTypes":["Elemental"]}"#
)]
#[case::clean_up_of_the_source_at_end_step(
    "Creature Wall",
    r#"K:Defender
A:AB$ Effect | Cost$ 0 | StaticAbilities$ CanAttack | AtEOT$ Exile | SpellDescription$ CARDNAME can attack this turn as though it didn't have defender. Exile it at the beginning of the next end step.
SVar:CanAttack:Mode$ CanAttackDefender | ValidCard$ Card.EffectSource | Description$ EFFECTSOURCE can attack this turn as though it didn't have defender.
Oracle:Defender (This creature can't attack.)\n{0}: Dark Maze can attack this turn as though it didn't have defender. Exile it at the beginning of the next end step."#,
    r#""DelayedTrigger""#
)]
#[case::enchanted_player_cast_limit(
    "Enchantment Aura Curse",
    r#"K:Enchant:Player
SVar:AttachAILogic:Curse
S:Mode$ CantBeCast | ValidCard$ Card | Caster$ Player.EnchantedBy | NumLimitEachTurn$ 1 | Description$ Enchanted player can't cast more than one spell each turn.
Oracle:Enchant player\nEnchanted player can't cast more than one spell each turn."#,
    r#""who":"AttachedPlayer""#
)]
#[case::enchanted_player_cant_gain_life(
    "Enchantment Aura",
    r#"K:Enchant:Player
SVar:AttachAILogic:Curse
S:Mode$ CantGainLife | ValidPlayer$ Player.EnchantedBy | Description$ Enchanted player can't gain life.
T:Mode$ DamageDoneOnce | Execute$ TrigLoseLife | ValidTarget$ Player.EnchantedBy | TriggerZones$ Battlefield | TriggerDescription$ Whenever enchanted player is dealt damage, they lose half their life, rounded up.
SVar:TrigLoseLife:DB$ LoseLife | Defined$ TriggeredTarget | LifeAmount$ X
SVar:X:TriggeredTarget$LifeTotal/HalfUp
Oracle:Enchant player\nEnchanted player can't gain life.\nWhenever enchanted player is dealt damage, they lose half their life, rounded up."#,
    r#""CantGainLife":"AttachedPlayer""#
)]
#[case::an_opponent_draws(
    "Legendary Creature Phyrexian Praetor",
    r#"K:Deathtouch
T:Mode$ Drawn | ValidCard$ Card.YouCtrl | TriggerZones$ Battlefield | Execute$ TrigGainLife | TriggerDescription$ Whenever you draw a card, you gain 2 life.
SVar:TrigGainLife:DB$ GainLife | Defined$ You | LifeAmount$ 2
T:Mode$ Drawn | ValidCard$ Card.OppCtrl | TriggerZones$ Battlefield | Execute$ TrigLoseLife | TriggerDescription$ Whenever an opponent draws a card, they lose 2 life.
SVar:TrigLoseLife:DB$ LoseLife | Defined$ TriggeredCardController | LifeAmount$ 2
Oracle:Deathtouch\nWhenever you draw a card, you gain 2 life.\nWhenever an opponent draws a card, they lose 2 life."#,
    r#""Drew":{"who":"Opponents"}"#
)]
#[case::enchanted_player_draws(
    "Enchantment Aura",
    r#"K:Enchant:Opponent
SVar:AttachAILogic:Curse
R:Event$ BeginPhase | ActiveZones$ Battlefield | ValidPlayer$ You | Phase$ Draw | Skip$ True | Description$ Skip your draw step.
T:Mode$ Drawn | ValidCard$ Card.OwnedBy Player.EnchantedBy | TriggerZones$ Battlefield | Execute$ TrigDraw | OptionalDecider$ You | TriggerDescription$ Whenever enchanted opponent draws a card, you may draw a card.
SVar:TrigDraw:DB$ Draw
Oracle:Enchant opponent\nSkip your draw step.\nWhenever enchanted opponent draws a card, you may draw a card."#,
    r#""Drew":{"who":"AttachedPlayer"}"#
)]
#[case::attacks_the_player_with_the_most_life(
    "Creature Human Rogue",
    r#"T:Mode$ Attacks | ValidCard$ Card.Self | Attacked$ Player.withMostLife | Execute$ DBUnblockable | TriggerDescription$ Whenever CARDNAME attacks the player with the most life or tied for most life, it can't be blocked this turn.
SVar:DBUnblockable:DB$ Effect | RememberObjects$ Self | ExileOnMoved$ Battlefield | StaticAbilities$ Unblockable
SVar:Unblockable:Mode$ CantBlockBy | ValidAttacker$ Card.IsRemembered | Description$ EFFECTSOURCE can't be blocked this turn.
Oracle:Whenever Undercover Butler attacks the player with the most life or tied for most life, it can't be blocked this turn."#,
    r#""at":"MostLife""#
)]
#[case::attacks_an_opponent_defending_player(
    "Creature Zombie Wizard",
    r#"K:Menace
T:Mode$ Attacks | ValidCard$ Creature | Attacked$ Player.Opponent | TriggerZones$ Battlefield | Execute$ TrigLoseLife | TriggerDescription$ Whenever a creature attacks one of your opponents, that player loses 1 life.
SVar:TrigLoseLife:DB$ LoseLife | Defined$ TriggeredDefender | LifeAmount$ 1
Oracle:Menace\nWhenever a creature attacks one of your opponents, that player loses 1 life."#,
    r#""who":"Defending""#
)]
#[case::blocked_by_a_creature(
    "Creature Cat",
    r#"T:Mode$ AttackerBlockedByCreature | ValidCard$ Card.Self | ValidBlocked$ Creature | TriggerZones$ Battlefield | Execute$ TrigBlockedPump | TriggerDescription$ Whenever CARDNAME becomes blocked by a creature, CARDNAME gets +1/+1 until end of turn.
SVar:TrigBlockedPump:DB$ Pump | Defined$ Self | NumAtt$ +1 | NumDef$ +1
Oracle:Whenever Cave Tiger becomes blocked by a creature, Cave Tiger gets +1/+1 until end of turn."#,
    r#""BecomesBlocked""#
)]
#[case::other_than_the_attacker(
    "Artifact Equipment",
    r#"K:Equip:1
T:Mode$ Attacks | ValidCard$ Card.EquippedBy | Alone$ True | Execute$ TrigPump | TriggerDescription$ Whenever equipped creature attacks alone, it gets +1/+1 until end of turn for each other creature you control.
SVar:TrigPump:DB$ Pump | Defined$ TriggeredAttackerLKICopy | NumAtt$ +X | NumDef$ +X
SVar:X:Count$Valid Creature.YouCtrl+!TriggeredAttacker
Oracle:Whenever equipped creature attacks alone, it gets +1/+1 until end of turn for each other creature you control.\nEquip {1} ({1}: Attach to target creature you control. Equip only as a sorcery.)"#,
    r#"{"Not":{"Is":"Triggering"}}"#
)]
#[case::the_attacker_and_this(
    "Creature Human Soldier",
    r#"T:Mode$ AttackerBlocked | ValidCard$ Creature | ValidBlocker$ Card.Self | Execute$ TrigDestroy | TriggerDescription$ When CARDNAME blocks a creature, destroy that creature and CARDNAME.
SVar:TrigDestroy:DB$ Destroy | Defined$ TriggeredAttackerLKICopy & Self
Oracle:When Alaborn Zealot blocks a creature, destroy that creature and Alaborn Zealot."#,
    r#""AnyOf":[{"Is":"Triggering"},{"Is":"This"}]"#
)]
#[case::committed_a_crime_this_turn(
    "Creature Human Warlock",
    r#"K:Deathtouch
T:Mode$ DamageDone | ValidSource$ Card.Self | Execute$ TrigSearch | CombatDamage$ True | ValidTarget$ Player | CheckDefinedPlayer$ You.committedCrimeThisTurn | TriggerZones$ Battlefield | TriggerDescription$ Whenever CARDNAME deals combat damage to a player, if you've committed a crime this turn, you may sacrifice CARDNAME. If you do, search your library for a card, put it into your hand, then shuffle. (Targeting opponents, anything they control, and/or cards in their graveyards is a crime.)
SVar:TrigSearch:AB$ ChangeZone | Cost$ Sac<1/CARDNAME> | Origin$ Library | Destination$ Hand | ChangeType$ Card | Mandatory$ True
Oracle:Deathtouch\nWhenever Servant of the Stinger deals combat damage to a player, if you've committed a crime this turn, you may sacrifice Servant of the Stinger. If you do, search your library for a card, put it into your hand, then shuffle. (Targeting opponents, anything they control, and/or cards in their graveyards is a crime.)"#,
    r#""what":"Crimes""#
)]
#[case::the_city_s_blessing(
    "Creature Griffin",
    r#"K:Flying
K:Ascend
T:Mode$ Attacks | ValidCard$ Creature.Self | Execute$ TrigPutCounter | Blessing$ True | TriggerDescription$ Whenever CARDNAME attacks, if you have the city's blessing, put a +1/+1 counter on it.
SVar:TrigPutCounter:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1
Oracle:Flying\nAscend (If you control ten or more permanents, you get the city's blessing for the rest of the game.)\nWhenever Resplendent Griffin attacks, if you have the city's blessing, put a +1/+1 counter on it."#,
    r#""CitysBlessing""#
)]
#[case::attacks_the_monarch(
    "Legendary Artifact Equipment",
    r#"T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | TriggerZones$ Battlefield | CheckSVar$ NoMonarch | SVarCompare$ EQ0 | Execute$ TrigMonarch | TriggerDescription$ When CARDNAME enters, if there is no monarch, you become the monarch.
SVar:TrigMonarch:DB$ BecomeMonarch
SVar:NoMonarch:PlayerCountPlayers$HasPropertyisMonarch
S:Mode$ Continuous | AffectedDefined$ Equipped | Affected$ Creature | AddPower$ 2 | AddToughness$ 2 | AddKeyword$ Vigilance | Description$ Equipped creature gets +2/+2 and has vigilance.
T:Mode$ Attacks | ValidCard$ Card.EquippedBy | Attacked$ Player.isMonarch | TriggerZones$ Battlefield | Execute$ TrigDestroy | TriggerDescription$ Whenever equipped creature attacks the monarch, destroy target tapped nonland permanent that player controls.
SVar:TrigDestroy:DB$ Destroy | ValidTgts$ Permanent.nonLand+tapped+ControlledBy TriggeredDefendingPlayer | TgtPrompt$ Select target tapped nonland permanent damaged player controls
K:Equip:2
Oracle:When The Spear of Bashenga enters, if there is no monarch, you become the monarch.\nEquipped creature gets +2/+2 and has vigilance.\nWhenever equipped creature attacks the monarch, destroy target tapped nonland permanent that player controls.\nEquip {2}"#,
    r#""at":"Monarch""#
)]
#[case::players_who_discarded_this_turn(
    "Legendary Creature Human Noble",
    r#"K:Firebending:1
T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigDiscard | TriggerDescription$ Whenever NICKNAME attacks, you may discard a card. Then you get an experience counter for each player who discarded a card this turn.
SVar:TrigDiscard:DB$ Discard | Defined$ You | Mode$ TgtChoose | Optional$ True | NumCards$ 1 | SubAbility$ DBExperience
SVar:DBExperience:DB$ PutCounter | Defined$ You | CounterType$ Experience | CounterNum$ X
A:AB$ Pump | Cost$ 2 B | Defined$ Self | NumAtt$ +Y | NumDef$ +Y | KW$ Menace | SpellDescription$ Until end of turn, NICKNAME gets +1/+1 for each experience counter you have and gains menace.
SVar:X:PlayerCountPlayers$ConditionGE1 CardsDiscardedThisTurn
SVar:Y:Count$YourCountersExperience
Oracle:Firebending 1 (Whenever this creature attacks, add {R}. This mana lasts until end of combat.)\nWhenever Azula attacks, you may discard a card. Then you get an experience counter for each player who discarded a card this turn.\n{2}{B}: Until end of turn, Azula gets +1/+1 for each experience counter you have and gains menace."#,
    r#""Tallied":{"what":"Discarded""#
)]
#[case::cycling_another_card(
    "Creature Fox",
    r#"T:Mode$ Cycled | ValidCard$ Card.Other | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigPutCounter | TriggerDescription$ Whenever you cycle another card, put a +1/+1 counter on CARDNAME.
SVar:TrigPutCounter:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1
K:Cycling:1
Oracle:Whenever you cycle another card, put a +1/+1 counter on Flourishing Fox.\nCycling {1} ({1}, Discard this card: Draw a card.)"#,
    r#""Cycled":{"by":"You""#
)]
fn a_card_reads_without_a_gap_and_holds_its_fragment(
    #[case] types: &str,
    #[case] lines: &str,
    #[case] fragment: &str,
) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    let found = gaps(&text);
    assert!(found.is_empty(), "{found:?}");
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(json.contains(fragment), "{fragment} missing in {json}");
}

/// A shield counter has no meaning in the engine, so a rule that waits on one stays a gap.
#[test]
fn a_rule_that_waits_on_a_shield_counter_stays_a_gap() {
    let text = "Name:Subject\nTypes:Creature Human\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigPutCounter\nSVar:TrigPutCounter:DB$ PutCounter | ValidTgts$ Creature.YouDontCtrl | CounterType$ SHIELD | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | RememberObjects$ Targeted | StaticAbilities$ GainControl | ForgetOnMoved$ Battlefield | ForgetCounter$ SHIELD | Duration$ Permanent\nSVar:GainControl:Mode$ Continuous | Affected$ Card.IsRemembered | GainControl$ You\n";
    assert!(!gaps(text).is_empty());
}

/// "If an opponent is the monarch" and "if you have the most life" gate a trigger as the Forge state words do.
#[rstest]
#[case::opponent_monarch("Opponent.isMonarch", r#""Monarch":"Opponents""#)]
#[case::most_life("You.withMostLife", r#""HighestLife":"EachPlayer""#)]
fn a_state_word_of_a_player_gates_the_trigger(#[case] word: &str, #[case] fragment: &str) {
    let text = format!(
        "Name:Subject\nTypes:Creature Human\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | CheckDefinedPlayer$ {word} | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw | NumCards$ 1\n"
    );
    let found = gaps(&text);
    assert!(found.is_empty(), "{found:?}");
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(json.contains(fragment), "{fragment} missing in {json}");
}
