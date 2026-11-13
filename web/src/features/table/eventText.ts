/** The game log as sentences: one line for every `EventView`, in the words a player would use. */
import { describeLoss } from '@/features/game/outcome'
import type {
  AttackTarget,
  Choice,
  ContinuousEffect,
  Chosen,
  CounterKind,
  DamageTarget,
  DeathCause,
  Event,
  EventView,
  HiddenEvent,
  ManaCost,
  ManualAction,
  ManualZone,
  Modification,
  ObjectId,
  ObjectRef,
  PlayerId,
  RuleEffect,
  Target,
  TextWord,
  Zone,
  ZoneCause,
} from '@/protocol/generated'
import { colorsInSet, costText, counterName, manaLetter } from './cardModel'
import type { CoinAnswer, EventContext } from './eventContext'
import { distinctTargets } from './prompt/targetText'
import { sameHost } from '@/play/hosts'
import type { HostRef } from '@/play/hosts'
import { STEP_LABEL } from './turnText'
import { REASON_WORDS } from './handReason'
import { rollSentence } from './manual/panel/roll'
import { teamLabel } from '@/lib/teams'
import { keywordWords as grantKeywordWords } from './manual/offers'
import { takeBackAction } from './takeBackText'

/** How the log names things; both fall back to a plain word when the thing is unknown. */
export interface EventNames {
  seat(seat: PlayerId): string
  card(id: ObjectId): string
  /** The clauses of a card's rules that are left to the table, when the viewer knows them. */
  clauses?(id: ObjectId): readonly string[]
  /** Whether the card is a permanent card, whose clauses are its abilities and not the effects of a spell. */
  permanent?(id: ObjectId): boolean
  /** The card an ability on the stack comes from, when the log knows the object is an ability. */
  ability?(id: ObjectId): ObjectId | undefined
  /** The name of a door (0 or 1) of a Room, when the viewer knows it. */
  door?(id: ObjectId, door: number): string | undefined
}

/** One log line. Minor lines (priority passes, mana, untaps) are drawn quietly. */
export interface LogLine {
  text: string
  minor: boolean
}

const line = (text: string): LogLine => ({ text, minor: false })
const quiet = (text: string): LogLine => ({ text, minor: true })

/** What a permanent chose as it entered, in words: ", choosing red". */
function chosenWords(chosen: Chosen, n: EventNames): string {
  if ('Chapter' in chosen) return `, reading ahead to chapter ${chosen.Chapter}`
  if ('Color' in chosen) return `, choosing ${chosen.Color.toLowerCase()}`
  if ('Colors' in chosen) return `, choosing ${joinWords(chosen.Colors.map((color) => color.toLowerCase()))}`
  if ('Player' in chosen) return `, choosing ${n.seat(chosen.Player)}`
  if ('LandType' in chosen) return `, choosing ${chosen.LandType}`
  if ('LandTypes' in chosen) return `, choosing ${chosen.LandTypes.join(', then ')}`
  if ('CardType' in chosen) return `, choosing ${chosen.CardType.toLowerCase()}`
  if ('Name' in chosen) return `, choosing ${chosen.Name}`
  if ('Number' in chosen) return `, choosing ${chosen.Number}`
  if ('Word' in chosen) return `, choosing ${chosen.Word}`
  if ('ColorAndType' in chosen) {
    const [color, type] = chosen.ColorAndType
    return `, choosing ${color.toLowerCase()} and ${type}`
  }
  if ('Players' in chosen) return `, choosing ${joinWords(chosen.Players.map((seat) => n.seat(seat)))}`
  return `, choosing ${chosen.CreatureType}`
}

/**
 * A spell or ability on the stack by name: a spell as its card is named, an ability as "an ability of" its source
 * (capitalised at the start of a sentence), so a trigger is never mistaken for the card it comes from.
 */
function stackName(n: EventNames, id: ObjectId, start: boolean): string {
  const source = n.ability?.(id)
  if (source === undefined) return n.card(id)
  return `${start ? 'An' : 'an'} ability of ${n.card(source)}`
}

const cards = (count: number): string => `${count} ${count === 1 ? 'card' : 'cards'}`

/** The longest clause the log quotes whole. */
const QUOTE_CHARS = 160

/** A clause of a card's rules in quotes, cut at `QUOTE_CHARS`. */
export const quoted = (text: string): string =>
  `“${text.length > QUOTE_CHARS ? `${text.slice(0, QUOTE_CHARS - 1)}…` : text}”`

/** "A", "A and B", "A, B and C". */
export function joinWords(items: readonly string[]): string {
  const last = items.at(-1)
  if (last === undefined) return ''
  return items.length === 1 ? last : `${items.slice(0, -1).join(', ')} and ${last}`
}

function keywordWords(keyword: unknown): string {
  const tag = typeof keyword === 'string' ? keyword : (Object.keys(Object(keyword))[0] ?? '')
  return tag.replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase()
}

/** The words of a kind of keyword a change takes away, whatever its parameter. */
const KEYWORD_KIND_WORDS = {
  Landwalk: 'all landwalk abilities',
  Protection: 'protection',
  Ward: 'ward',
  BandsWithOther: 'all "bands with other" abilities',
} as const

/** What a change does to what it changes, as a verb phrase ("becomes an artifact too", "gets +1/+1"). */
/** The card types in declaration order; the wire set holds bit n for the nth. */
const CARD_TYPE_ORDER = [
  'artifact',
  'battle',
  'creature',
  'enchantment',
  'instant',
  'kindred',
  'land',
  'planeswalker',
  'sorcery',
]

function cardTypeNames(types: number): string[] {
  return CARD_TYPE_ORDER.filter((_, bit) => (types >> bit) & 1)
}

function modificationWords(modification: Modification, group: boolean, n: EventNames): string {
  const becomes = group ? 'become' : 'becomes'
  const gains = group ? 'gain' : 'gains'
  const loses = group ? 'lose' : 'loses'
  if (typeof modification === 'string') {
    return {
      LoseAbilities: `${loses} all abilities`,
      LoseAbilitiesExceptMana: `${loses} all abilities except mana abilities`,
      SwitchPowerToughness: `${group ? 'have' : 'has'} power and toughness switched`,
      CantAttack: "can't attack",
      CantBlock: "can't block",
      DoesntUntap: `${group ? "don't" : "doesn't"} untap during their controller's untap step`,
      CantBeBlocked: "can't be blocked",
      CantBeBlockedUnlessAttackingOwner:
        "can't be blocked unless it's attacking its owner or a permanent its owner controls",
      CantBeTurnedFaceUp: "can't be turned face up",
      BlockedByAll: "can't be blocked unless all creatures defending player controls block it",
      AddAllCreatureTypes: `${becomes} every creature type`,
    }[modification]
  }
  if ('CantBeBlockedBy' in modification) return "can't be blocked by some creatures"
  if ('CanBlockOnly' in modification) return 'can block only some creatures'
  if ('MinBlockers' in modification) {
    return `can't be blocked except by ${modification.MinBlockers} or more creatures`
  }
  if ('MaxBlockers' in modification) {
    return `can't be blocked by more than ${modification.MaxBlockers} ${modification.MaxBlockers === 1 ? 'creature' : 'creatures'}`
  }
  if ('BlocksAdditional' in modification) {
    const more = modification.BlocksAdditional
    return `can block an additional ${more === 1 ? 'creature' : `${more} creatures`} each combat`
  }
  if ('AddType' in modification) return `${becomes} ${modification.AddType.toLowerCase()} too`
  if ('RemoveType' in modification) return `${loses} ${modification.RemoveType.toLowerCase()} as a type`
  if ('SetTypes' in modification) {
    return `${becomes} only ${joinWords(cardTypeNames(modification.SetTypes))}`
  }
  if ('AddSupertype' in modification) {
    return `${becomes} ${modification.AddSupertype.toLowerCase()} too`
  }
  if ('RemoveSupertype' in modification) {
    return `${loses} ${modification.RemoveSupertype.toLowerCase()}`
  }
  if ('AddSubtype' in modification) return `${becomes} ${modification.AddSubtype} too`
  if ('SetCreatureTypes' in modification) {
    return `${becomes} ${joinWords(modification.SetCreatureTypes)}`
  }
  if ('SetLandTypes' in modification) return `${becomes} ${joinWords(modification.SetLandTypes)}`
  if ('SetColors' in modification) {
    const colors = colorsInSet(modification.SetColors).map((color) => color.toLowerCase())
    return `${becomes} ${colors.length === 0 ? 'colorless' : joinWords(colors)}`
  }
  if ('AddColors' in modification) {
    const colors = colorsInSet(modification.AddColors).map((color) => color.toLowerCase())
    return `${becomes} ${joinWords(colors)} in addition to its other colors`
  }
  if ('AddKeyword' in modification) return `${gains} ${keywordWords(modification.AddKeyword)}`
  if ('AddKeywords' in modification) {
    const keywords = Array.isArray(modification.AddKeywords) ? (modification.AddKeywords as unknown[]) : []
    return `${gains} ${joinWords(keywords.map(keywordWords))}`
  }
  if ('RemoveKeyword' in modification) return `${loses} ${keywordWords(modification.RemoveKeyword)}`
  if ('RemoveKeywordKind' in modification) {
    return `${loses} ${KEYWORD_KIND_WORDS[modification.RemoveKeywordKind]}`
  }
  if ('LockKeyword' in modification) {
    const word = keywordWords(modification.LockKeyword)
    return `${loses} ${word} and can't have or gain it`
  }
  if ('GrantAbility' in modification || 'GainAbilities' in modification) return `${gains} an ability`
  if ('GrantText' in modification) return `${gains} “${modification.GrantText}”`
  if ('AddNameWords' in modification)
    return `${group ? 'have' : 'has'} “${modification.AddNameWords}” added to the name`
  if ('SetController' in modification) return `${group ? 'change' : 'changes'} controller`
  if ('SetPower' in modification) return `${becomes} ${modification.SetPower} power`
  if ('SetToughness' in modification) return `${becomes} ${modification.SetToughness} toughness`
  if ('SetPowerToughness' in modification) {
    const { power, toughness } = modification.SetPowerToughness
    return `${becomes} ${power}/${toughness}`
  }
  if ('LoseAbility' in modification) return `${loses} one of its abilities`
  if ('Goaded' in modification) return `${group ? 'are' : 'is'} goaded`
  if ('CantAttackYou' in modification) {
    const { you, attacked } = modification.CantAttackYou
    const seat = n.seat(you)
    const whom = {
      Player: seat,
      Planeswalkers: `planeswalkers ${seat} controls`,
      Both: `${seat} or planeswalkers ${seat} controls`,
    }[attacked]
    return `can't attack ${whom}`
  }
  if ('CantAttackOwner' in modification) {
    const whom = {
      Player: 'its owner',
      Planeswalkers: 'planeswalkers its owner controls',
      Both: 'its owner or planeswalkers its owner controls',
    }[modification.CantAttackOwner.attacked]
    return `can't attack ${whom}`
  }
  if ('ChangeText' in modification) {
    const word = (text: TextWord) => ('Color' in text ? text.Color.toLowerCase() : text.Subtype)
    const { from, to } = modification.ChangeText
    return `${group ? 'have' : 'has'} "${word(from)}" changed to "${word(to)}" in ${group ? 'their' : 'its'} text`
  }
  const { power, toughness } = modification.ModifyPowerToughness
  return `${group ? 'get' : 'gets'} ${power >= 0 ? '+' : ''}${power}/${toughness >= 0 ? '+' : ''}${toughness}`
}

function describeEffect(effect: ContinuousEffect, n: EventNames): string {
  const { selector, modification, duration, source } = effect
  let who = 'All creatures'
  if (typeof selector !== 'string') {
    who =
      'Objects' in selector
        ? joinWords(selector.Objects.map((object) => n.card(object.id)))
        : `Creatures controlled by ${n.seat(selector.CreaturesControlledBy)}`
  }
  // One card "becomes" and "gets"; a group of creatures "become" and "get".
  const group =
    typeof selector === 'string' || 'CreaturesControlledBy' in selector || selector.Objects.length !== 1
  const what = modificationWords(modification, group, n)
  return `${who} ${what}${durationWords(duration, source, n, group)}`
}

/** How long a change lasts, as the end of a sentence (nothing for a change that never ends). */
function durationWords(
  duration: ContinuousEffect['duration'],
  source: ObjectRef | null,
  n: EventNames,
  several = false,
): string {
  if (typeof duration === 'object') {
    if ('UntilPaid' in duration) return ` until its controller pays ${duration.UntilPaid} to end it`
    if ('WhileMonarch' in duration) return ' while that player is the monarch'
    const counter = `${counterName(duration.WhileHasCounter)} counter`
    return several ? ` while each has a ${counter}` : ` while it has a ${counter}`
  }
  const UNTIL: Record<Exclude<ContinuousEffect['duration'], object>, string> = {
    UntilEndOfTurn: ' until end of turn',
    UntilEndOfCombat: ' until end of combat',
    UntilYourNextTurn: " until their controller's next turn",
    UntilYourNextUpkeep: " until their controller's next upkeep",
    UntilEndOfYourNextUpkeep: " until the end of their controller's next upkeep",
    UntilYourNextEndStep: " until their controller's next end step",
    UntilEndOfYourNextTurn: " until the end of their controller's next turn",
    DuringItsControllersNextTurn: " during its controller's next turn",
    UntilItsControllersNextCombatEnds: " during its controller's next combat phase",
    UntilEndOfCombatOnYourNextTurn: " until end of combat on their controller's next turn",
    WhileSourceOnBattlefield:
      source === null ? ' while its source stays' : ` while ${n.card(source.id)} stays`,
    WhileSourceAttached:
      source === null ? ' while its source stays attached' : ` while ${n.card(source.id)} stays attached`,
    WhileYouControlSource:
      source === null
        ? ' while its controller controls its source'
        : ` while its controller controls ${n.card(source.id)}`,
    WhileSourceTapped:
      source === null ? ' while its source stays tapped' : ` while ${n.card(source.id)} stays tapped`,
    WhileYouControlSourceTapped:
      source === null
        ? ' while its controller controls its source and it stays tapped'
        : ` while its controller controls ${n.card(source.id)} and it stays tapped`,
    UntilOpponentBecomesMonarch: ' until an opponent becomes the monarch',
    NextUntapStep: " through its controller's next untap step",
    UntilTurnSkipped: ' until the turn it skips',
    UntilSkipped: ' until the step or combat it skips',
    UntilSkippedThisTurn: ' until the step or combat it skips this turn',
    TheirNextTurn: " during that player's next turn",
    UntilTheirNextTurn: " until that player's next turn",
    UntilCastFromExile:
      source === null
        ? ' until its source is cast from exile'
        : ` until ${n.card(source.id)} is cast from exile`,
    WhileEnchanted: several ? ' while each is enchanted' : ' while it is enchanted',
    UntilItsControllersNextUntapStep: " until its controller's next untap step",
    Forever: '',
  }
  return UNTIL[duration]
}

/** The price of one taxed creature: its mana, its life or both. */
function taxPrice(mana: ManaCost, life: number): string {
  const parts = [mana.length > 0 ? costText(mana) : '', life > 0 ? `${life} life` : ''].filter(
    (part) => part !== '',
  )
  return parts.join(' and ')
}

/** What a change to a rule of the game says, for the players or cards it names. */
function describeRule(effect: RuleEffect, n: EventNames): string {
  const { rule, duration, source } = effect
  const named = (objects: readonly ObjectRef[]): string =>
    joinWords(objects.map((object) => n.card(object.id)))
  const seats = (players: readonly PlayerId[]): string => joinWords(players.map((player) => n.seat(player)))
  let what: string
  if (rule === 'GraveyardCardsUntargetable') what = "Cards in graveyards can't be targeted"
  else if (rule === 'DamageAsWither') what = 'Damage is dealt as though its source had wither'
  else if (rule === 'ManaBurn') what = 'Unspent mana makes its player lose that much life'
  else if (rule === 'ReverseTurnOrder') what = 'Turn order is reversed'
  else if (rule === 'Planar') what = 'A rule about the planar die or planeswalking is in force'
  else if ('AttackAsThoughHasteAt' in rule)
    what = `Creatures can attack ${seats(rule.AttackAsThoughHasteAt)} as though they had haste`
  else if ('MustAttack' in rule) what = `${named(rule.MustAttack)} must attack if able`
  else if ('MustAttackPlayers' in rule)
    what = rule.MustAttackPlayers.map(
      ([object, player]) => `${n.card(object.id)} must attack ${n.seat(player)} if able`,
    ).join(', ')
  else if ('MustAttackAt' in rule) {
    const { creatures, players, at } = rule.MustAttackAt
    const who = [
      creatures.length > 0 ? named(creatures) : '',
      players.length > 0
        ? `${creatures.length > 0 ? 'creatures' : 'Creatures'} ${seats(players)} controls`
        : '',
    ].filter((part) => part !== '')
    what = `${joinWords(who)} must attack ${n.card(at.id)} if able`
  } else if ('MustBlock' in rule) what = `${named(rule.MustBlock)} must block if able`
  else if ('CantBeCountered' in rule) what = `${named(rule.CantBeCountered)} can't be countered`
  else if ('EntersWith' in rule) {
    const { spell, counters, keyword, this_turn } = rule.EntersWith
    const gets = [
      counters > 0
        ? `enters with ${counters} additional +1/+1 ${counters === 1 ? 'counter' : 'counters'}`
        : '',
      typeof keyword === 'string'
        ? `gains ${keyword.toLowerCase()}${this_turn ? ' until end of turn' : ''}`
        : '',
    ].filter((part) => part !== '')
    what = `${n.card(spell.id)} ${gets.join(' and ')}`
  } else if ('CantGainLife' in rule) what = `${seats(rule.CantGainLife)} can't gain life`
  else if ('NoLifeLossFromDamage' in rule)
    what = `Damage doesn't cause ${seats(rule.NoLifeLossFromDamage)} to lose life`
  else if ('LifeFloor' in rule)
    what = `Damage can't reduce ${seats(rule.LifeFloor.players)} below ${rule.LifeFloor.floor} life`
  else if ('RadiationGainsLife' in rule)
    what = `Radiation makes ${seats(rule.RadiationGainsLife)} gain life rather than lose life`
  else if ('MayCastRather' in rule)
    what = `${seats(rule.MayCastRather.casts.map(([owner]) => owner))} may cast ${named(rule.MayCastRather.casts.map(([, card]) => card))} for a different cost`
  else if ('ManaProduced' in rule)
    what = `Taps for mana by ${seats(rule.ManaProduced.who)} produce changed mana`
  else if ('CantDraw' in rule) what = `${seats(rule.CantDraw)} can't draw cards`
  else if ('CantSearch' in rule) what = `${seats(rule.CantSearch)} can't search libraries`
  else if ('VoteAgain' in rule) what = `${seats(rule.VoteAgain)} may vote an additional time`
  else if ('CantLoseGame' in rule)
    what = rule.CantLoseGame.life_only
      ? `${seats(rule.CantLoseGame.who)} can't lose the game for having 0 or less life`
      : `${seats(rule.CantLoseGame.who)} can't lose the game`
  else if ('CantCastSpells' in rule) what = `${seats(rule.CantCastSpells.who)} can't cast spells`
  else if ('ToughnessAssignsDamage' in rule)
    what = `Creatures of ${seats(rule.ToughnessAssignsDamage)} deal combat damage by toughness`
  else if ('RevealTop' in rule)
    what = `${seats(rule.RevealTop)} play with the top card of their library revealed`
  else if ('RevealHand' in rule) what = `${seats(rule.RevealHand)} play with their hands revealed`
  else if ('MayCast' in rule) {
    const { who, only, play } = rule.MayCast
    what = `${seats(who)} may ${play ? 'play' : 'cast'} ${only === null ? 'cards they could not' : named(only)}`
  } else if ('DamageCantBePrevented' in rule) what = "Damage can't be prevented"
  else if ('PreventDamage' in rule) {
    const { left, redirect } = rule.PreventDamage
    if (redirect !== undefined && left !== null)
      what = `The next ${left} damage is dealt to ${targetName(redirect, n)} instead`
    else what = left === null ? 'Damage is prevented' : `The next ${left} damage is prevented`
  } else if ('Regenerate' in rule) what = `${n.card(rule.Regenerate.id)} gets a regeneration shield`
  else if ('BlockLimitEach' in rule) {
    const { most, who } = rule.BlockLimitEach
    what = `${seats(who)} can't block with more than ${most} ${most === 1 ? 'creature' : 'creatures'}`
  } else if ('AttackTax' in rule) {
    const { players, mana, life } = rule.AttackTax
    what = `Creatures can't attack ${seats(players)} unless their controller pays ${taxPrice(mana, life)} for each`
  } else if ('BlockTax' in rule) {
    const { mana, life } = rule.BlockTax
    what = `Some creatures can't block unless their controller pays ${taxPrice(mana, life)} for each`
  } else if ('SpellCost' in rule) {
    const { change, mana } = rule.SpellCost
    what = `Some spells cost ${String(mana)} ${change === 'Reduce' ? 'less' : 'more'} to cast`
  } else what = 'Triggered abilities trigger an additional time'
  return `${what}${durationWords(duration, source, n)}`
}

function attackTarget(target: AttackTarget, n: EventNames): string {
  return 'Player' in target ? n.seat(target.Player) : n.card(target.Planeswalker)
}

function damageTarget(target: DamageTarget, n: EventNames): string {
  return 'Player' in target ? n.seat(target.Player) : n.card(target.Object)
}

/** The name of a player or an object a spell, ability or effect names. */
export function targetName(target: Target, n: EventNames): string {
  return 'Player' in target ? n.seat(target.Player) : n.card(target.Object.id)
}

/** " targeting A and B" for the targets of a spell or ability (each once), and nothing when it has none. */
export function targetingWords(targets: readonly Target[] | undefined, n: EventNames): string {
  const named = distinctTargets(targets ?? [])
  return named.length === 0 ? '' : ` targeting ${joinWords(named.map((target) => targetName(target, n)))}`
}

/** "a +1/+1 counter", "3 poison counters". */
function counterPhrase(count: number, kind: CounterKind): string {
  const name = typeof kind === 'string' ? kind : kind.Other
  return count === 1 ? `a ${name} counter` : `${count} ${name} counters`
}

function zoneWord(zone: Zone, n: EventNames): string {
  if (zone === 'Battlefield') return 'the battlefield'
  if (zone === 'Stack') return 'the stack'
  if (zone === 'Exile') return 'exile'
  if (zone === 'PhasedOut') return 'the battlefield, phased out'
  if ('Library' in zone) return `${n.seat(zone.Library)}'s library`
  if ('Hand' in zone) return `${n.seat(zone.Hand)}'s hand`
  if ('Graveyard' in zone) return `${n.seat(zone.Graveyard)}'s graveyard`
  return `${n.seat(zone.Command)}'s command zone`
}

const MANUAL_ZONE: Record<ManualZone, string> = {
  Battlefield: 'the battlefield',
  Hand: 'the hand',
  Graveyard: 'the graveyard',
  Exile: 'exile',
  LibraryTop: 'the top of the library',
  LibraryBottom: 'the bottom of the library',
  Command: 'the command zone',
}

/** The zone a card left, without whose it was. */
function leftWord(zone: Zone): string {
  if (zone === 'Battlefield' || zone === 'PhasedOut') return 'the battlefield'
  if (zone === 'Stack') return 'the stack'
  if (zone === 'Exile') return 'exile'
  if ('Library' in zone) return 'the library'
  if ('Hand' in zone) return 'the hand'
  if ('Graveyard' in zone) return 'the graveyard'
  return 'the command zone'
}

/** The cards a seat put back, by place; a sentence names them for the seat that knows them. */
export interface Arranged {
  top: readonly ObjectId[]
  bottom: readonly ObjectId[]
  graveyard: readonly ObjectId[]
}

/**
 * How a seat put back the top cards it looked at by hand: "Ana puts back the top 3 cards of their library: A on top, B
 * and C on the bottom (by hand)" for the seat that knows them (`named`), "1 on top, 2 on the bottom" by count for any
 * other.
 */
export function describeArrange(
  player: PlayerId,
  top: number,
  bottom: number,
  graveyard: number,
  n: EventNames,
  named?: Arranged,
): string {
  const place = (count: number, ids: readonly ObjectId[] | undefined, where: string): string | null => {
    if (count === 0) return null
    return `${ids === undefined ? count : joinWords(ids.map(n.card))} ${where}`
  }
  const said = [
    place(top, named?.top, 'on top'),
    place(bottom, named?.bottom, 'on the bottom'),
    place(graveyard, named?.graveyard, 'into their graveyard'),
  ].filter((part) => part !== null)
  const all = cards(top + bottom + graveyard)
  return `${n.seat(player)} puts back the top ${all} of their library: ${said.join(', ')} (by hand)`
}

/**
 * What a seat did by hand, as one sentence; the events of the change follow it in the log. `from` is the zone a moved
 * card left, which the action does not name and the change does (`foldLog` passes it).
 */
export function describeManual(
  player: PlayerId,
  action: ManualAction,
  n: EventNames,
  from?: Zone,
  shown?: readonly ObjectId[],
  rolled?: number,
): string {
  const who = n.seat(player)
  if ('Move' in action) {
    const left = from === undefined ? '' : ` from ${leftWord(from)}`
    return `${who} moves ${n.card(action.Move.object)}${left} to ${MANUAL_ZONE[action.Move.to]} (by hand)`
  }
  if ('MoveTop' in action) {
    const { count, to } = action.MoveTop
    return `${who} moves the top ${cards(count)} of their library to ${MANUAL_ZONE[to]} (by hand)`
  }
  if ('Counters' in action) {
    const { object, counter, delta } = action.Counters
    const phrase = counterPhrase(Math.abs(delta), counter)
    return `${who} ${delta > 0 ? 'puts' : 'removes'} ${phrase} ${delta > 0 ? 'on' : 'from'} ${n.card(object)} (by hand)`
  }
  if ('PlayerCounters' in action) {
    const { counter, delta } = action.PlayerCounters
    return `${who} ${delta > 0 ? 'gets' : 'loses'} ${counterPhrase(Math.abs(delta), counter)} (by hand)`
  }
  if ('Life' in action) {
    const { delta } = action.Life
    return `${who} ${delta > 0 ? 'gains' : 'loses'} ${Math.abs(delta)} life (by hand)`
  }
  if ('CreateToken' in action) {
    const { token, count } = action.CreateToken
    const noun = count === 1 ? 'token' : 'tokens'
    // The token of a card is named by the TokenCreated events after this line; the card says where it is from.
    if ('Card' in token) return `${who} creates ${count} ${noun} ${token.Card.card} makes (by hand)`
    const name = 'Common' in token ? token.Common : token.Custom.name
    return `${who} creates ${count} ${name} ${noun} (by hand)`
  }
  if ('CopyToken' in action) {
    const { object, count } = action.CopyToken
    return `${who} creates ${count} ${count === 1 ? 'token that is a copy' : 'tokens that are copies'} of ${n.card(object)} (by hand)`
  }
  if ('Tap' in action) return `${who} taps ${n.card(action.Tap.object)} (by hand)`
  if ('Untap' in action) return `${who} untaps ${n.card(action.Untap.object)} (by hand)`
  if ('Draw' in action) {
    // The drawer is told the cards, as an ordinary draw does; the others see how many.
    const { count } = action.Draw
    const said = shown !== undefined && shown.length === count ? joinWords(shown.map(n.card)) : cards(count)
    return `${who} draws ${said} (by hand)`
  }
  if ('Discard' in action) return `${who} discards ${n.card(action.Discard.object)} (by hand)`
  if ('Attach' in action) {
    return `${who} attaches ${n.card(action.Attach.object)} to ${n.card(action.Attach.to)} (by hand)`
  }
  if ('Unattach' in action) {
    return `${who} takes ${n.card(action.Unattach.object)} off what it is attached to (by hand)`
  }
  if ('Damage' in action) {
    const { source, target, amount } = action.Damage
    return `${who}: ${n.card(source)} deals ${amount} damage to ${damageTarget(target, n)} (by hand)`
  }
  if ('AddMana' in action) {
    const { mana, count } = action.AddMana
    return `${who} adds ${`{${manaLetter(mana)}}`.repeat(count)} (by hand)`
  }
  if ('LookTop' in action) {
    // The seat that looked is told the cards; the others see how many.
    const told = shown === undefined || shown.length === 0 ? '' : `: ${joinWords(shown.map(n.card))}`
    return `${who} looks at the top ${cards(action.LookTop.count)} of their library${told} (by hand)`
  }
  if ('ArrangeTop' in action) {
    const { top, bottom, graveyard } = action.ArrangeTop
    return describeArrange(player, top.length, bottom.length, graveyard.length, n, action.ArrangeTop)
  }
  if ('Roll' in action) return rollSentence(who, action.Roll.kind, rolled)
  if ('Shuffle' in action) return `${who} shuffles their library (by hand)`
  if ('Reveal' in action) {
    const { what } = action.Reveal
    if ('Hand' in what) return `${who} reveals ${n.card(what.Hand.object)} from their hand (by hand)`
    const top = shown?.at(0)
    return `${who} reveals the top card of their library${top === undefined ? '' : `: ${n.card(top)}`} (by hand)`
  }
  if ('Note' in action) {
    const { object, text } = action.Note
    return text.trim() === ''
      ? `${who} clears the note on ${n.card(object)} (by hand)`
      : `${who} puts a note on ${n.card(object)}: “${text.trim()}” (by hand)`
  }
  if ('Pump' in action) {
    const { object, power, toughness } = action.Pump
    const signed = (value: number): string => (value < 0 ? `${value}` : `+${value}`)
    return `${who} gives ${n.card(object)} ${signed(power)}/${signed(toughness)} until end of turn (by hand)`
  }
  if ('GrantAbility' in action) {
    const { object, grant, until_end_of_turn } = action.GrantAbility
    const what = 'Keyword' in grant ? grantKeywordWords(grant.Keyword) : `“${grant.Text.trim()}”`
    return `${who} gives ${n.card(object)} ${what} ${until_end_of_turn ? 'until end of turn' : 'for good'} (by hand)`
  }
  if ('Transform' in action) return `${who} turns ${n.card(action.Transform.object)} over (by hand)`
  if ('AttackDespiteDefender' in action) {
    return `${who} lets ${n.card(action.AttackDespiteDefender.object)} attack despite defender this turn (by hand)`
  }
  if ('BlockAdditional' in action) {
    const { object, count } = action.BlockAdditional
    return `${who} lets ${n.card(object)} block ${count} more ${count === 1 ? 'creature' : 'creatures'} this turn (by hand)`
  }
  if ('MoveBack' in action) {
    return `${who} puts ${n.card(action.MoveBack.object)} onto the battlefield back face up (by hand)`
  }
  return describeControl(who, action, n, from)
}

/** The search of a library, a tutor, a sacrifice, a summoning sickness removed and a control given, by hand. */
function describeControl(who: string, action: ManualAction, n: EventNames, from?: Zone): string {
  if ('Search' in action) return `${who} searches their library (by hand)`
  if ('Tutor' in action) {
    return `${who} puts ${n.card(action.Tutor.object)} from their library into their hand and shuffles (by hand)`
  }
  if ('RemoveSickness' in action) {
    return `${who} removes summoning sickness from ${n.card(action.RemoveSickness.object)} (by hand)`
  }
  if ('Sacrifice' in action) return `${who} sacrifices ${n.card(action.Sacrifice.object)} (by hand)`
  if ('GiveControl' in action) {
    const { object, to } = action.GiveControl
    if (from === undefined || from === 'Battlefield') {
      return `${who} gives control of ${n.card(object)} to ${n.seat(to)} (by hand)`
    }
    return `${who} puts ${n.card(object)} from ${leftWord(from)} onto the battlefield under ${n.seat(to)}’s control (by hand)`
  }
  return `${who} acts by hand`
}

/** A card put into a hand from its owner's library by hand, which the viewer cannot name. */
export function describeHiddenTutor(player: PlayerId, n: EventNames): string {
  return `${n.seat(player)} puts a card from their library into their hand and shuffles (by hand)`
}

/** A card moved by hand that the viewer cannot name, in the words of `describeManual`; `from` is the zone it left. */
export function describeHiddenManual(player: PlayerId, to: ManualZone, n: EventNames, from?: Zone): string {
  const left = from === undefined ? '' : ` from ${leftWord(from)}`
  return `${n.seat(player)} moves a card${left} to ${MANUAL_ZONE[to]} (by hand)`
}

/** "two coins", "four coins": the coins one flip became. */
function coinCount(count: number): string {
  return ['two', 'three', 'four'][count - 2] ?? String(count)
}

/** What an answer to a question of a coin flip says, after the name: the call (heads first) or the coin kept (a won one first). */
function coinChoiceWords(coin: CoinAnswer, option: number): string {
  if (coin === 'call') return `calls ${option === 0 ? 'heads' : 'tails'}`
  return option === 0 ? 'keeps a won coin' : 'keeps a lost coin'
}

/** What a player's answer to a question of a spell or ability says, after their name. */
function describeChoice(choice: Choice, n: EventNames): string {
  if ('Number' in choice) return `chooses ${choice.Number}`
  if ('Cards' in choice) {
    return choice.Cards.length === 0 ? 'chooses nothing' : `chooses ${joinWords(choice.Cards.map(n.card))}`
  }
  if ('Order' in choice) return `puts ${cards(choice.Order.length)} in order`
  if ('YesNo' in choice) return choice.YesNo ? 'answers yes' : 'answers no'
  if ('Option' in choice) return `chooses option ${choice.Option + 1}`
  if ('Vote' in choice) return `votes for option ${choice.Vote + 1}`
  if ('SecretNumber' in choice) return `secretly chooses ${choice.SecretNumber}`
  return 'chooses which effect applies first'
}

/** The words for a cause that is a plain name; a card hidden with hideaway or exiled until a permanent leaves carries that permanent. */
const BECAUSE: Record<
  Exclude<
    ZoneCause,
    | { Hideaway: unknown }
    | { Held: unknown }
    | { Sealed: unknown }
    | { ExiledUntil: unknown }
    | { ExiledUntilMonarch: unknown }
    | { Encoded: unknown }
    | { Haunting: unknown }
    | { Manifested: unknown }
    | { Cloaked: unknown }
    | { PutFaceDown: unknown }
  >,
  string
> = {
  Moved: 'moves',
  Destroyed: 'is destroyed',
  Buried: "is destroyed and can't be regenerated",
  Sacrificed: 'is sacrificed',
  Exiled: 'is exiled',
  Countered: 'is countered',
  Milled: 'is milled',
  StateBased: 'is put into the graveyard',
  Discarded: 'is discarded',
  Foretold: 'is foretold',
  Plotted: 'is plotted',
  Adventure: 'is exiled on an adventure',
  LandPlayed: 'is played and put into the graveyard',
}

/** The words for a cause that carries a player or a permanent. */
function objectCause(cause: Extract<ZoneCause, object>): string {
  if ('Hideaway' in cause || 'Held' in cause || 'Sealed' in cause) return 'is exiled face down'
  if ('Encoded' in cause) return 'is exiled encoded on a creature'
  if ('Haunting' in cause) return 'is exiled haunting a creature'
  if ('Manifested' in cause) return 'is manifested'
  if ('Cloaked' in cause) return 'is cloaked'
  if ('PutFaceDown' in cause) return 'is put onto the battlefield face down'
  return 'is exiled for a while'
}

/** The events of spells and abilities: abilities on the stack, the steps of a resolution, life, zones and what players learn. */
const PIPELINE = [
  'AbilityPutOnStack',
  'AbilityActivated',
  'AbilityTriggered',
  'TriggersOrdered',
  'TriggerRemoved',
  'DelayedTriggerCreated',
  'DelayedTriggerExpired',
  'ResolutionBegan',
  'ChoiceMade',
  'StepDone',
  'Remembered',
  'ResolutionEnded',
  'AbilityLeftToTable',
  'ByHandDue',
  'ZoneChanged',
  'LifeGained',
  'LifeLost',
  'CountersChanged',
  'TokenCreated',
  'CardConjured',
  'Sought',
  'LibraryLooked',
  'HandLooked',
  'CardsFound',
  'Scried',
  'Surveilled',
  'Revealed',
] as const
type PipelineName = (typeof PIPELINE)[number]
type PipelineEvent = PipelineName extends infer K
  ? K extends PipelineName
    ? Extract<Event, Record<K, unknown>>
    : never
  : never

function isPipeline(event: Event): event is PipelineEvent {
  return typeof event !== 'string' && PIPELINE.some((name) => name in event)
}

/**
 * A move that the resolving spell or ability made, named by it: "Hill Giant is exiled by Swords to Plowshares", "Unsummon
 * returns Hill Giant to Bo's hand". Null for every other move, which keeps its plain line.
 */
function byResolving(
  resolving: ObjectId,
  name: string,
  from: Zone,
  to: Zone,
  cause: ZoneCause,
  n: EventNames,
): string | null {
  const by = stackName(n, resolving, false)
  if (cause === 'Exiled' || cause === 'Destroyed') return `${name} ${BECAUSE[cause]} by ${by}`
  if (cause === 'Buried') return `${name} is destroyed by ${by} and can't be regenerated`
  const bounced = cause === 'Moved' && from === 'Battlefield' && typeof to !== 'string' && 'Hand' in to
  return bounced ? `${stackName(n, resolving, true)} returns ${name} to ${zoneWord(to, n)}` : null
}

function describePipeline(event: PipelineEvent, n: EventNames, context?: EventContext): LogLine {
  if ('AbilityPutOnStack' in event) {
    const { source, cause, choices } = event.AbilityPutOnStack
    const targeting = cause === undefined ? '' : targetingWords(choices?.targets?.flat(), n)
    const text = `An ability of ${n.card(source.id)} goes on the stack${targeting}`
    // A triggered ability names its targets here; an activated one named them in its own line.
    return targeting === '' ? quiet(text) : line(text)
  }
  if ('AbilityActivated' in event) {
    const { player, source } = event.AbilityActivated
    return line(
      `${n.seat(player)} activates an ability of ${n.card(source.id)}${targetingWords(context?.targets, n)}`,
    )
  }
  if ('AbilityTriggered' in event) {
    return line(`An ability of ${n.card(event.AbilityTriggered.source.id)} triggers`)
  }
  if ('TriggersOrdered' in event) {
    return quiet(`${n.seat(event.TriggersOrdered.player)} puts their triggered abilities in order`)
  }
  if ('TriggerRemoved' in event) {
    return line(`A triggered ability of ${n.card(event.TriggerRemoved.source.id)} is removed`)
  }
  if ('DelayedTriggerCreated' in event) {
    return line(`A delayed ability of ${n.card(event.DelayedTriggerCreated.source.id)} is set up`)
  }
  if ('DelayedTriggerExpired' in event) return quiet('A delayed ability ends without triggering')
  if ('ResolutionBegan' in event)
    return quiet(`${stackName(n, event.ResolutionBegan.object, true)} begins to resolve`)
  if ('ChoiceMade' in event) {
    const { player, choice } = event.ChoiceMade
    const coin = context?.coin
    if (coin !== undefined && 'Option' in choice)
      return quiet(`${n.seat(player)} ${coinChoiceWords(coin, choice.Option)}`)
    return quiet(`${n.seat(player)} ${describeChoice(choice, n)}`)
  }
  if ('Remembered' in event)
    return quiet(
      event.Remembered.objects.length === 0 && event.Remembered.values.length > 0
        ? 'A source remembers a value'
        : 'A source remembers objects',
    )
  if ('StepDone' in event) return quiet('A step of the resolution is done')
  if ('ResolutionEnded' in event) {
    const { object, outcome } = event.ResolutionEnded
    if (outcome === 'Fizzled') return line(`${stackName(n, object, true)} does nothing: its targets are gone`)
    if (outcome === 'Countered') {
      const by = context?.resolving
      return line(
        by === undefined
          ? `${stackName(n, object, true)} is countered`
          : `${stackName(n, by, true)} counters ${stackName(n, object, false)}`,
      )
    }
    if (outcome === 'Lapsed')
      return line(`${stackName(n, object, true)} does nothing: its condition is no longer true`)
    return quiet(`${stackName(n, object, true)} finishes resolving`)
  }
  if ('AbilityLeftToTable' in event) {
    return line(
      `${n.card(event.AbilityLeftToTable.source.id)}'s ability could not be run by the game: apply its text by hand`,
    )
  }
  if ('ByHandDue' in event) {
    const { source, text } = event.ByHandDue
    return line(`${n.card(source.id)} has a line to apply by hand now: ${quoted(text)}`)
  }
  if ('ZoneChanged' in event) {
    const { object, from, to, cause } = event.ZoneChanged
    const by = context?.resolving
    if (by !== undefined && by !== object) {
      const done = byResolving(by, n.card(object), from, to, cause, n)
      if (done !== null) return line(done)
    }
    if (typeof to !== 'string' && 'Command' in to) {
      // A commander that goes home: the answer to the return question, or a move the question replaced.
      const how =
        cause === 'Moved' || cause === 'StateBased' || typeof cause === 'object'
          ? ''
          : `${BECAUSE[cause]} and `
      return line(`${n.card(object)} ${how}returns to the command zone`)
    }
    return line(
      cause === 'Moved'
        ? `${n.card(object)} moves to ${zoneWord(to, n)}`
        : `${n.card(object)} ${typeof cause === 'object' ? objectCause(cause) : BECAUSE[cause]}`,
    )
  }
  if ('LifeGained' in event)
    return line(`${n.seat(event.LifeGained.player)} gains ${event.LifeGained.amount} life`)
  if ('LifeLost' in event) return line(`${n.seat(event.LifeLost.player)} loses ${event.LifeLost.amount} life`)
  if ('CountersChanged' in event) {
    const { on, counter, delta } = event.CountersChanged
    const phrase = counterPhrase(Math.abs(delta), counter)
    return line(`${targetName(on, n)} ${delta > 0 ? 'gets' : 'loses'} ${phrase}`)
  }
  if ('TokenCreated' in event) {
    const { controller, name, blocking } = event.TokenCreated
    const blocks = blocking === undefined ? '' : ` blocking ${n.card(blocking)}`
    return line(`${n.seat(controller)} creates a ${name} token${blocks}`)
  }
  if ('CardConjured' in event) {
    const { owner, name, zone } = event.CardConjured
    return line(`${n.seat(owner)} conjures ${name} into ${zoneWord(zone, n)}`)
  }
  if ('Sought' in event) {
    return line(`${n.seat(event.Sought.player)} seeks ${cards(event.Sought.count)}`)
  }
  if ('LibraryLooked' in event) {
    const { player, cards: looked } = event.LibraryLooked
    return line(`${n.seat(player)} looks at ${cards(looked.length)} of their library`)
  }
  if ('HandLooked' in event) {
    const { player, of, cards: looked } = event.HandLooked
    const whose = of === player ? 'their' : `${n.seat(of)}'s`
    return line(`${n.seat(player)} looks at ${whose} hand: ${joinWords(looked.map(n.card))}`)
  }
  if ('CardsFound' in event) {
    return line(`${n.seat(event.CardsFound.player)} finds ${cards(event.CardsFound.cards.length)}`)
  }
  if ('Scried' in event) {
    const { player, top, bottom } = event.Scried
    return line(`${n.seat(player)} scries ${top.length + bottom.length}`)
  }
  if ('Surveilled' in event) {
    const { player, top, graveyard } = event.Surveilled
    return line(`${n.seat(player)} surveils ${top.length + graveyard.length}`)
  }
  const { player, cards: shown } = event.Revealed
  return line(`${n.seat(player)} reveals ${joinWords(shown.map(n.card))}`)
}

// One clause per target, in the order the targets first appear: "Bo with A and B; Cy with C".
function attackClauses(attackers: readonly [ObjectId, AttackTarget][], n: EventNames): string {
  const byTarget = new Map<string, string[]>()
  for (const [id, target] of attackers) {
    const key = attackTarget(target, n)
    byTarget.set(key, [...(byTarget.get(key) ?? []), n.card(id)])
  }
  return [...byTarget].map(([target, names]) => `${target} with ${joinWords(names)}`).join('; ')
}

function describePublic(event: Event, n: EventNames, context?: EventContext): LogLine {
  if (isPipeline(event)) return describePipeline(event, n, context)
  if (typeof event === 'string') {
    switch (event) {
      case 'MulliganPhaseBegan':
        return line('The mulligan phase begins')
      case 'MulliganPhaseEnded':
        return line('The mulligan phase ends')
      case 'CombatEnded':
        return quiet('Combat ends')
      case 'DamageCleared':
        return quiet('Damage wears off')
    }
  }
  if ('FirstPlayerChosen' in event) return line(`${n.seat(event.FirstPlayerChosen.player)} goes first`)
  if ('CommanderColorChosen' in event) {
    const { player, object, color } = event.CommanderColorChosen
    return line(`${n.seat(player)} chooses ${color.toLowerCase()} for ${n.card(object)}`)
  }
  if ('LibraryShuffled' in event)
    return quiet(`${n.seat(event.LibraryShuffled.player)} shuffles their library`)
  if ('AttractionDeckShuffled' in event)
    return quiet(`${n.seat(event.AttractionDeckShuffled.player)} shuffles their Attraction deck`)
  if ('HandDealt' in event) {
    return line(`${n.seat(event.HandDealt.player)} is dealt ${cards(event.HandDealt.cards.length)}`)
  }
  if ('MulliganDeclared' in event) return line(`${n.seat(event.MulliganDeclared.player)} decides to mulligan`)
  if ('MulliganTaken' in event) return line(`${n.seat(event.MulliganTaken.player)} takes a mulligan`)
  if ('OpeningCardsPlayed' in event) {
    const { player, cards: begun } = event.OpeningCardsPlayed
    const names = begun.map((id) => n.card(id)).join(', ')
    return line(
      `${n.seat(player)} begins the game with ${names === '' ? 'no cards' : names} on the battlefield`,
    )
  }
  if ('MulliganKept' in event) return line(`${n.seat(event.MulliganKept.player)} keeps their hand`)
  if ('CardsBottomed' in event) {
    const { player, cards: bottomed } = event.CardsBottomed
    return line(`${n.seat(player)} puts ${cards(bottomed.length)} on the bottom of their library`)
  }
  if ('LibraryBottomed' in event) {
    const { player, cards: bottomed } = event.LibraryBottomed
    return line(`${n.seat(player)} puts ${cards(bottomed.length)} on the bottom of their library`)
  }
  if ('LibraryInserted' in event) {
    const { player, cards: inserted, below } = event.LibraryInserted
    return line(
      `${n.seat(player)} puts ${cards(inserted.length)} beneath the top ${cards(below)} of their library`,
    )
  }
  if ('ChosenAtRandom' in event) {
    return line(`${event.ChosenAtRandom.cards.map((id) => n.card(id)).join(', ')} is picked at random`)
  }
  if ('PlayersChosenAtRandom' in event) {
    const players = event.PlayersChosenAtRandom.players.map((seat) => n.seat(seat)).join(', ')
    return line(
      `${players} ${event.PlayersChosenAtRandom.players.length === 1 ? 'is' : 'are'} picked at random`,
    )
  }
  if ('Entered' in event) {
    const { object, tapped, controller, chosen } = event.Entered
    const chose = chosen === undefined ? '' : chosenWords(chosen, n)
    return quiet(
      `${n.card(object)} enters${tapped ? ' tapped' : ''} under ${n.seat(controller)}'s control${chose}`,
    )
  }
  if ('ChosenAsAttached' in event) {
    const { object, chosen } = event.ChosenAsAttached
    return line(`${n.card(object)} becomes attached${chosenWords(chosen, n)}`)
  }
  if ('Attached' in event) {
    return line(`${n.card(event.Attached.object)} becomes attached to ${n.card(event.Attached.to)}`)
  }
  if ('Transformed' in event) return line(`${n.card(event.Transformed.object)} transforms`)
  if ('Flipped' in event) return line(`${n.card(event.Flipped.object)} flips`)
  if ('Prepared' in event) return line(`${n.card(event.Prepared.object)} becomes prepared`)
  if ('Unprepared' in event) return line(`${n.card(event.Unprepared.object)} is no longer prepared`)
  if ('Suspected' in event) return line(`${n.card(event.Suspected.object)} becomes suspected`)
  if ('Unsuspected' in event) return line(`${n.card(event.Unsuspected.object)} is no longer suspected`)
  if ('Solved' in event) return line(`${n.card(event.Solved.object)} is solved`)
  if ('Harnessed' in event) return line(`${n.card(event.Harnessed.object)} is harnessed`)
  if ('BecameCopy' in event) {
    const { object, of } = event.BecameCopy
    return line(`${n.card(object)} becomes a copy of ${n.card(of.id)}`)
  }
  if ('SpellCopied' in event) {
    const { controller, of } = event.SpellCopied
    return line(`${n.seat(controller)} copies ${n.card(of.id)}`)
  }
  if ('AbilityCopied' in event) {
    const { controller, of } = event.AbilityCopied
    return line(`${n.seat(controller)} copies the ability of ${n.card(n.ability?.(of.id) ?? of.id)}`)
  }
  if ('TargetsChanged' in event) {
    const { object } = event.TargetsChanged
    return line(`The targets of ${n.card(n.ability?.(object.id) ?? object.id)} change`)
  }
  if ('CopyEnded' in event) return line(`${n.card(event.CopyEnded.object)} stops being a copy`)
  if ('CardCopied' in event) {
    const { owner, of } = event.CardCopied
    return line(`${n.seat(owner)} copies ${n.card(of.id)}`)
  }
  if ('CopyCeased' in event) return line(`${n.card(event.CopyCeased.object)} ceases to exist`)
  if ('CommanderStayed' in event)
    return line(`${n.card(event.CommanderStayed.object)} stays out of the command zone`)
  if ('CompanionRevealed' in event)
    return line(`${n.seat(event.CompanionRevealed.player)} reveals a companion`)
  if ('CompanionBrought' in event) {
    return line(`${n.seat(event.CompanionBrought.player)} puts a companion into their hand`)
  }
  if ('DayNightChanged' in event)
    return line(`It becomes ${event.DayNightChanged.to === 'Day' ? 'day' : 'night'}`)
  if ('MonarchChanged' in event) return line(`${n.seat(event.MonarchChanged.player)} becomes the monarch`)
  if ('EmblemCreated' in event) return line(`${n.seat(event.EmblemCreated.owner)} gets an emblem`)
  if ('Proliferated' in event) return line(`${n.seat(event.Proliferated.player)} proliferates`)
  if ('Learned' in event) return line(`${n.seat(event.Learned.player)} learns`)
  if ('EvidenceCollected' in event) return line(`${n.seat(event.EvidenceCollected.player)} collects evidence`)
  if ('Investigated' in event) return line(`${n.seat(event.Investigated.player)} investigates`)
  if ('Discovered' in event) {
    const { player, value, card } = event.Discovered
    const found = card === null ? '' : `, finding ${n.card(card)}`
    return line(`${n.seat(player)} discovers ${value}${found}`)
  }
  if ('DreadManifested' in event) {
    const { player, card } = event.DreadManifested
    const buried = card === null ? '' : ` and puts ${n.card(card)} into their graveyard`
    return line(`${n.seat(player)} manifests dread${buried}`)
  }
  if ('GiftGiven' in event) {
    return line(`${n.seat(event.GiftGiven.player)} gives ${n.seat(event.GiftGiven.to)} a gift`)
  }
  if ('UpkeepSettled' in event) {
    const { player, object, echo, paid } = event.UpkeepSettled
    const cost = echo ? 'echo cost' : 'cumulative upkeep'
    return line(`${n.seat(player)} ${paid ? 'pays' : "doesn't pay"} the ${cost} of ${n.card(object)}`)
  }
  if ('Foraged' in event) return line(`${n.seat(event.Foraged.player)} forages`)
  if ('DieRolled' in event) {
    const { player, sides, result } = event.DieRolled
    return line(`${n.seat(player)} rolls a d${sides}: ${result}`)
  }
  if ('RolledToVisit' in event) {
    const { player, result, visited } = event.RolledToVisit
    const count = visited.length
    return line(
      `${n.seat(player)} rolls ${result} to visit their Attractions: ${count === 0 ? 'none is' : count === 1 ? 'one is' : `${count} are`} visited`,
    )
  }
  if ('CoinFlipped' in event) {
    const { player, won, call, side, coins } = event.CoinFlipped
    const who = n.seat(player)
    if (coins !== undefined && coins.length > 1) {
      const flipped = joinWords(coins.map((coin) => coin.toLowerCase()))
      return line(
        `${who} calls ${call.toLowerCase()} and flips ${coinCount(coins.length)} coins: ${flipped}; ${who} keeps ${side.toLowerCase()} and ${won ? 'wins' : 'loses'} the flip`,
      )
    }
    return line(
      `${who} calls ${call.toLowerCase()}, the coin comes up ${side.toLowerCase()}: ${who} ${won ? 'wins' : 'loses'} the flip`,
    )
  }
  if ('VotesFinished' in event) {
    return line('The players finish voting')
  }
  if ('Clashed' in event) {
    const { player, opponent, won } = event.Clashed
    return line(`${n.seat(player)} ${won ? 'wins' : 'loses'} a clash with ${n.seat(opponent)}`)
  }
  if ('Ventured' in event) {
    const v = event.Ventured
    return line(`${n.seat(v.player)} ventures into ${v.room_name} (${v.dungeon})`)
  }
  if ('InitiativeTaken' in event) {
    return line(`${n.seat(event.InitiativeTaken.player)} takes the initiative`)
  }
  if ('DungeonCompleted' in event) {
    return line(`${n.seat(event.DungeonCompleted.player)} completes a dungeon`)
  }
  if ('SpeedChanged' in event) {
    const { player, speed } = event.SpeedChanged
    return line(`${n.seat(player)}'s speed becomes ${speed}${speed >= 4 ? ' (max speed)' : ''}`)
  }
  if ('CitysBlessingGained' in event) {
    return line(`${n.seat(event.CitysBlessingGained.player)} gets the city's blessing`)
  }
  if ('EnduringStoryGained' in event) {
    return line(`${n.seat(event.EnduringStoryGained.player)} gets an enduring story`)
  }
  if ('RingTempted' in event) {
    const { player, level, bearer } = event.RingTempted
    const chosen = bearer === undefined ? '' : ` and chooses ${n.card(bearer)} as their Ring-bearer`
    return line(`The Ring tempts ${n.seat(player)} (level ${level})${chosen}`)
  }
  if ('RingBearerLost' in event) {
    return line(`${n.seat(event.RingBearerLost.player)} loses their Ring-bearer`)
  }
  if ('Explored' in event) return line(`${n.card(event.Explored.object)} explores`)
  if ('Connived' in event) return line(`${n.card(event.Connived.object)} connives`)
  if ('Exerted' in event) return line(`${n.card(event.Exerted.object)} is exerted`)
  if ('ExertEnded' in event) return line(`${n.card(event.ExertEnded.object)} is no longer exerted`)
  if ('Amassed' in event) return line(`${n.seat(event.Amassed.player)} amasses`)
  if ('PrizeClaimed' in event) {
    const { player, attraction } = event.PrizeClaimed
    return line(`${n.seat(player)} claims the prize of ${n.card(attraction)}`)
  }
  if ('StickerSheetsChosen' in event) {
    const count = event.StickerSheetsChosen.sheets.length
    return quiet(
      `${n.seat(event.StickerSheetsChosen.player)} plays with ${count} sticker ${count === 1 ? 'sheet' : 'sheets'}`,
    )
  }
  if ('StickerPut' in event)
    return line(`${n.seat(event.StickerPut.player)} puts a sticker on ${n.card(event.StickerPut.object)}`)
  if ('Monstrous' in event) return line(`${n.card(event.Monstrous.object)} becomes monstrous`)
  if ('Devoured' in event) return line(`${n.card(event.Devoured.object)} devours ${event.Devoured.count}`)
  if ('Renowned' in event) return line(`${n.card(event.Renowned.object)} becomes renowned`)
  if ('Enlisted' in event)
    return line(`${n.card(event.Enlisted.object)} enlists ${n.card(event.Enlisted.enlisted)}`)
  if ('Fought' in event)
    return line(`${n.card(event.Fought.object)} fights ${n.card(event.Fought.other)}`)
  if ('Exploited' in event)
    return line(`${n.card(event.Exploited.object)} exploits ${n.card(event.Exploited.sacrificed)}`)
  if ('Crewed' in event) {
    const { object, creatures, saddle } = event.Crewed
    const by = creatures.length > 0 ? ` by ${joinWords(creatures.map((id) => n.card(id)))}` : ''
    return line(`${n.card(object)} is ${saddle ? 'saddled' : 'crewed'}${by}`)
  }
  if ('TurnedFaceUp' in event)
    return line(`${n.seat(event.TurnedFaceUp.player)} turns ${n.card(event.TurnedFaceUp.object)} face up`)
  if ('TurnedFaceDown' in event)
    return line(
      `${n.seat(event.TurnedFaceDown.player)} turns ${n.card(event.TurnedFaceDown.object)} face down`,
    )
  if ('FirstDrawRevealed' in event) {
    const { player, card } = event.FirstDrawRevealed
    return line(`${n.seat(player)} reveals ${n.card(card)} as the first card they drew this turn`)
  }
  if ('FaceDownLooked' in event)
    return line(`${n.seat(event.FaceDownLooked.player)} looks at ${n.card(event.FaceDownLooked.object)}`)
  if ('DoorLocked' in event) {
    const { player, object, door } = event.DoorLocked
    return line(`${n.seat(player)} locks ${n.door?.(object, door) ?? `a door of ${n.card(object)}`}`)
  }
  if ('DoorUnlocked' in event) {
    const { player, object, door } = event.DoorUnlocked
    return line(`${n.seat(player)} unlocks ${n.door?.(object, door) ?? `a door of ${n.card(object)}`}`)
  }
  if ('CardParted' in event) {
    const { permanent, card, to } = event.CardParted
    return line(`${n.card(card)} goes to ${zoneWord(to, n)} apart from ${n.card(permanent)}`)
  }
  if ('Mutated' in event)
    return line(
      `${n.seat(event.Mutated.player)} mutates a creature ${event.Mutated.over ? 'over' : 'under'} ${n.card(event.Mutated.object)}`,
    )
  if ('Adapted' in event) return line(`${n.card(event.Adapted.object)} adapts`)
  if ('ClassLevelGained' in event)
    return line(`${n.card(event.ClassLevelGained.object)} becomes level ${event.ClassLevelGained.level}`)
  if ('Bolstered' in event) return line(`${n.seat(event.Bolstered.player)} bolsters`)
  if ('AttachedToPlayer' in event) {
    return line(`${n.card(event.AttachedToPlayer.object)} enchants ${n.seat(event.AttachedToPlayer.player)}`)
  }
  if ('Unattached' in event) {
    return line(`${n.card(event.Unattached.object)} becomes unattached`)
  }
  if ('Paired' in event) {
    return line(`${n.card(event.Paired.object)} is paired with ${n.card(event.Paired.with)}`)
  }
  if ('Unpaired' in event) {
    return line(`${n.card(event.Unpaired.object)} is no longer paired with ${n.card(event.Unpaired.with)}`)
  }
  if ('JoinedAttack' in event) {
    const { object, target } = event.JoinedAttack
    return line(`${n.card(object)} enters attacking ${attackTarget(target, n)}`)
  }
  if ('AttackRedirected' in event) {
    const { object, target } = event.AttackRedirected
    return line(`${n.card(object)} now attacks ${attackTarget(target, n)}`)
  }
  if ('RemovedFromCombat' in event) {
    return line(`${n.card(event.RemovedFromCombat.object)} is removed from combat`)
  }
  if ('AttackersBlocked' in event) {
    const { attackers } = event.AttackersBlocked
    const verb = attackers.length === 1 ? 'becomes' : 'become'
    return line(`${joinWords(attackers.map(n.card))} ${verb} blocked`)
  }
  if ('TurnBegan' in event) {
    const { number, active, extra } = event.TurnBegan
    return line(`Turn ${number} begins: ${n.seat(active)}'s ${extra ? 'extra turn' : 'turn'}`)
  }
  if ('ExtraTurnAdded' in event) {
    return line(`${n.seat(event.ExtraTurnAdded.player)} takes an extra turn after this one`)
  }
  if ('ExtraTurnSkipped' in event) {
    return quiet(`${n.seat(event.ExtraTurnSkipped.player)} skips an extra turn`)
  }
  if ('PlayerControlGained' in event) {
    const { player, by, extra_turn } = event.PlayerControlGained
    const after = extra_turn === true ? ', then takes an extra turn' : ''
    return line(`${n.seat(by)} will control ${n.seat(player)} during their next turn${after}`)
  }
  if ('GameRestarted' in event) {
    const { by, kept } = event.GameRestarted
    const cards = kept.length === 0 ? '' : `, keeping ${kept.map((id) => n.card(id)).join(', ')} in exile`
    return line(`${n.seat(by)} restarts the game${cards}`)
  }
  if ('SpellControlGained' in event) {
    return line(
      `${n.seat(event.SpellControlGained.player)} gains control of ${n.card(event.SpellControlGained.object)}`,
    )
  }
  if ('PhasesAdded' in event) {
    const { steps, lone } = event.PhasesAdded
    const first = steps[0]
    if (lone && first !== undefined) {
      const step = STEP_LABEL[first].toLowerCase()
      return line(`${steps.length} additional ${step} step(s) follow this phase`)
    }
    return line(`${steps.length} additional phase(s) follow this phase`)
  }
  if ('StepAdded' in event)
    return line(`An additional ${STEP_LABEL[event.StepAdded.step].toLowerCase()} follows this step`)
  if ('StepBegan' in event) return quiet(`${STEP_LABEL[event.StepBegan.step]} begins`)
  if ('AnnouncementRewound' in event) {
    const { player, object, reason } = event.AnnouncementRewound
    return line(`${n.seat(player)} cannot play ${n.card(object)}: ${REASON_WORDS[reason].line.toLowerCase()}`)
  }
  if ('NothingToDo' in event) {
    const what = event.NothingToDo.kind === 'Attack' ? 'attackers' : 'blockers'
    return quiet(`${n.seat(event.NothingToDo.player)} has no possible ${what}`)
  }
  if ('PriorityGranted' in event) return quiet(`${n.seat(event.PriorityGranted.player)} receives priority`)
  if ('PriorityPassed' in event) return quiet(`${n.seat(event.PriorityPassed.player)} passes priority`)
  if ('LandPlayed' in event) {
    return line(`${n.seat(event.LandPlayed.player)} plays ${n.card(event.LandPlayed.object)}`)
  }
  if ('SpellCast' in event) {
    const { player, object, choices } = event.SpellCast
    return line(`${n.seat(player)} casts ${n.card(object)}${targetingWords(choices?.targets?.flat(), n)}`)
  }
  if ('SpellResolved' in event) return line(`${n.card(event.SpellResolved.object)} resolves`)
  if ('PermanentTapped' in event) return quiet(`${n.card(event.PermanentTapped.object)} becomes tapped`)
  if ('PermanentUntapped' in event) return quiet(`${n.card(event.PermanentUntapped.object)} becomes untapped`)
  if ('PermanentPhasedOut' in event) {
    const { object, until } = event.PermanentPhasedOut
    const held = until ? ` until ${n.card(until.id)} leaves the battlefield` : ''
    return line(`${n.card(object)} phases out${held}`)
  }
  if ('PermanentPhasedIn' in event) return line(`${n.card(event.PermanentPhasedIn.object)} phases in`)
  if ('ManaAbilityActivated' in event) {
    const { player, source } = event.ManaAbilityActivated
    return quiet(`${n.seat(player)} activates a mana ability of ${n.card(source.id)}`)
  }
  if ('ManaAdded' in event) {
    const { player, mana, source } = event.ManaAdded
    const from = source === null ? '' : ` from ${n.card(source)}`
    return quiet(`${n.seat(player)} adds {${manaLetter(mana)}}${from}`)
  }
  if ('ManaSpent' in event) {
    const symbols = event.ManaSpent.mana.map((mana) => `{${manaLetter(mana)}}`).join('')
    return quiet(`${n.seat(event.ManaSpent.player)} spends ${symbols}`)
  }
  if ('ManaPoolEmptied' in event) {
    const { player, kept, becomes } = event.ManaPoolEmptied
    if (becomes) return quiet(`${n.seat(player)}'s unspent mana becomes {${manaLetter(becomes)}}`)
    const except = kept?.length ? ` except ${kept.map((mana) => `{${manaLetter(mana)}}`).join('')}` : ''
    return quiet(`${n.seat(player)}'s mana pool empties${except}`)
  }
  if ('ManaBurned' in event) return quiet(`${n.seat(event.ManaBurned.player)}'s unspent mana burns`)
  if ('LibraryRefilled' in event) {
    return quiet(`${n.seat(event.LibraryRefilled.player)} shuffles their graveyard into their library`)
  }
  if ('LifePaid' in event) return line(`${n.seat(event.LifePaid.player)} pays ${event.LifePaid.amount} life`)
  if ('CardDrawn' in event)
    return line(`${n.seat(event.CardDrawn.player)} draws ${n.card(event.CardDrawn.object)}`)
  if ('DrewFromEmptyLibrary' in event) {
    return line(`${n.seat(event.DrewFromEmptyLibrary.player)} tries to draw from an empty library`)
  }
  if ('CardDiscarded' in event) {
    return line(`${n.seat(event.CardDiscarded.player)} discards ${n.card(event.CardDiscarded.object)}`)
  }
  if ('Cycled' in event) {
    return quiet(`${n.seat(event.Cycled.player)} cycles ${n.card(event.Cycled.object)}`)
  }
  if ('AttackersDeclared' in event) {
    const { player, attackers } = event.AttackersDeclared
    if (attackers.length === 0) return quiet(`${n.seat(player)} declares no attackers`)
    return line(`${n.seat(player)} attacks ${attackClauses(attackers, n)}`)
  }
  if ('BandFormed' in event) {
    const { player, attackers } = event.BandFormed
    return line(`${n.seat(player)} bands ${joinWords(attackers.map((id) => n.card(id)))} together`)
  }
  if ('BlockersDeclared' in event) {
    const { player, blocks } = event.BlockersDeclared
    if (blocks.length === 0) return quiet(`${n.seat(player)} declares no blockers`)
    const pairs = blocks.map(([blocker, attacker]) => `${n.card(blocker)} blocks ${n.card(attacker)}`)
    return line(`${n.seat(player)} declares blockers: ${joinWords(pairs)}`)
  }
  if ('DamageAssigned' in event) {
    const { attacker, assignment } = event.DamageAssigned
    const parts = assignment.map(([id, amount]) => `${amount} to ${n.card(id)}`)
    return quiet(`${n.card(attacker)} assigns ${joinWords(parts)}`)
  }
  if ('DamageDealt' in event) {
    const { source, target, amount } = event.DamageDealt
    return line(`${n.card(source)} deals ${amount} damage to ${damageTarget(target, n)}`)
  }
  if ('EffectCreated' in event) return quiet(describeEffect(event.EffectCreated.effect, n))
  if ('EffectExpired' in event) return quiet('An effect ends')
  if ('RuleCreated' in event) return quiet(describeRule(event.RuleCreated.effect, n))
  if ('RuleExpired' in event) return quiet('A rule change ends')
  if ('Regenerated' in event) return line(`${n.card(event.Regenerated.object)} regenerates`)
  if ('DamageRemoved' in event) {
    return line(`${n.card(event.DamageRemoved.object)} has all its damage removed`)
  }
  if ('ShieldUsed' in event) return quiet(`A prevention shield has ${event.ShieldUsed.left} left`)
  if ('DamagePrevented' in event) {
    const { source, target, amount } = event.DamagePrevented
    return line(`${amount} damage ${n.card(source)} would deal to ${damageTarget(target, n)} is prevented`)
  }
  if ('CreatureDied' in event) {
    const HOW: Record<DeathCause, string> = {
      LethalDamage: 'from lethal damage',
      Deathtouch: 'from deathtouch',
      ZeroToughness: 'with zero toughness',
    }
    return line(`${n.card(event.CreatureDied.object)} dies ${HOW[event.CreatureDied.cause]}`)
  }
  if ('BecameSpirit' in event) {
    return line(`${n.seat(event.BecameSpirit.player)} fell and rises as a Spirit`)
  }
  if ('PhaseBegan' in event) {
    const { player, emblem } = event.PhaseBegan
    return line(`${n.seat(player)} enters ${n.card(emblem)}`)
  }
  if ('SchemeSetInMotion' in event) {
    const { player, emblem } = event.SchemeSetInMotion
    return line(`${n.seat(player)} sets ${n.card(emblem)} in motion`)
  }
  if ('PlayerLost' in event) {
    return line(`${n.seat(event.PlayerLost.player)} ${describeLoss(event.PlayerLost.reason)}`)
  }
  if ('ObjectLeftGame' in event) return quiet(`${n.card(event.ObjectLeftGame.object)} leaves the game`)
  if ('GameEnded' in event) {
    const { result } = event.GameEnded
    if (result === 'Draw') return line('The game ends in a draw')
    return line(
      'Team' in result ? `${teamLabel(result.Team)} wins the game` : `${n.seat(result.Winner)} wins the game`,
    )
  }
  if ('ManualAction' in event) {
    return line(describeManual(event.ManualAction.player, event.ManualAction.action, n))
  }
  if ('ManualRolled' in event) {
    const { player, kind, result } = event.ManualRolled
    return line(rollSentence(n.seat(player), kind, result))
  }
  if ('TookBack' in event) {
    const { player, kind, name } = event.TookBack
    return line(`${n.seat(player)} took back ${takeBackAction(kind, name)}`)
  }
  const unhandled: never = event
  return line(String(unhandled))
}

/** The name of the source of a hidden ability, or "a hidden card" when the viewer cannot identify it. */
function unseen(source: ObjectRef | null | undefined, n: EventNames): string {
  return source ? n.card(source.id) : 'a hidden card'
}

function describeHidden(event: HiddenEvent, n: EventNames): LogLine {
  if (typeof event === 'string') return quiet('A card leaves the game')
  if ('Remembered' in event) return quiet('A source remembers objects')
  if ('CardDrawn' in event) return line(`${n.seat(event.CardDrawn.player)} draws a card`)
  if ('CardConjured' in event) return line(`${n.seat(event.CardConjured.player)} conjures a card`)
  if ('HandDealt' in event)
    return line(`${n.seat(event.HandDealt.player)} is dealt ${cards(event.HandDealt.count)}`)
  if ('LibraryShuffled' in event)
    return quiet(`${n.seat(event.LibraryShuffled.player)} shuffles their library`)
  if ('AttractionDeckShuffled' in event)
    return quiet(`${n.seat(event.AttractionDeckShuffled.player)} shuffles their Attraction deck`)
  if ('LibraryRefilled' in event) {
    return quiet(`${n.seat(event.LibraryRefilled.player)} shuffles their graveyard into their library`)
  }
  if ('MulliganTaken' in event) return line(`${n.seat(event.MulliganTaken.player)} takes a mulligan`)
  if ('CardsBottomed' in event) {
    const { player, count } = event.CardsBottomed
    return line(`${n.seat(player)} puts ${cards(count)} on the bottom of their library`)
  }
  if ('CardsInserted' in event) {
    const { player, count, below } = event.CardsInserted
    return line(`${n.seat(player)} puts ${cards(count)} beneath the top ${cards(below)} of their library`)
  }
  if ('LibraryLooked' in event) {
    const { player, count } = event.LibraryLooked
    return line(`${n.seat(player)} looks at ${cards(count)} of their library`)
  }
  if ('HandLooked' in event) {
    const { player, of, count } = event.HandLooked
    const whose = of === player ? 'their' : `${n.seat(of)}'s`
    return line(`${n.seat(player)} looks at ${cards(count)} of ${whose} hand`)
  }
  if ('CardsFound' in event)
    return line(`${n.seat(event.CardsFound.player)} finds ${cards(event.CardsFound.count)}`)
  if ('PickedAtRandom' in event) {
    const { count } = event.PickedAtRandom
    return line(`${cards(count)} from a hidden zone ${count === 1 ? 'is' : 'are'} picked at random`)
  }
  if ('Scried' in event) {
    const { player, top, bottom } = event.Scried
    return line(`${n.seat(player)} scries ${top + bottom}`)
  }
  if ('Surveilled' in event) {
    const { player, top, graveyard } = event.Surveilled
    return line(`${n.seat(player)} surveils ${top + graveyard}`)
  }
  if ('ZoneChanged' in event) {
    const { from, to, cause } = event.ZoneChanged
    // A foretold card lies face down: the others learn who foretold and nothing about the card.
    if (typeof cause === 'object' && typeof from === 'object' && 'Library' in from) {
      return line(`${n.seat(from.Library)} hides a card face down`)
    }
    if (cause === 'Foretold' && typeof from === 'object' && 'Hand' in from) {
      return line(`${n.seat(from.Hand)} foretells a card`)
    }
    return quiet(`A card moves to ${zoneWord(to, n)}`)
  }
  if ('ChoiceMade' in event) return quiet(`${n.seat(event.ChoiceMade.player)} chooses`)
  if ('AbilityTriggered' in event) {
    return line(`An ability of ${unseen(event.AbilityTriggered.source, n)} triggers`)
  }
  if ('AbilityPutOnStack' in event) {
    return quiet(`An ability of ${unseen(event.AbilityPutOnStack.source, n)} goes on the stack`)
  }
  if ('AbilityLeftToTable' in event) {
    const { source, controller } = event.AbilityLeftToTable
    return line(
      `An ability of ${unseen(source, n)} could not be run by the game: ${n.seat(controller)} applies it by hand`,
    )
  }
  if ('TriggerRemoved' in event) {
    return line(`A triggered ability of ${n.card(event.TriggerRemoved.source.id)} is removed`)
  }
  if ('DelayedTriggerCreated' in event) {
    return line(`A delayed ability of ${n.card(event.DelayedTriggerCreated.source.id)} is set up`)
  }
  if ('ManualMove' in event) {
    return line(describeHiddenManual(event.ManualMove.player, event.ManualMove.to, n))
  }
  if ('ManualArrange' in event) {
    const { player, top, bottom, graveyard } = event.ManualArrange
    return line(describeArrange(player, top, bottom, graveyard, n))
  }
  if ('ManualTutor' in event) return line(describeHiddenTutor(event.ManualTutor.player, n))
  if ('AnnouncementRewound' in event) {
    return quiet(`${n.seat(event.AnnouncementRewound.player)} could not finish a play`)
  }
  void (event satisfies { StepDone: unknown })
  return quiet('A step of the resolution is done')
}

/**
 * The line of an attach event that knows where the attachment hung before: "Bonesplitter moves from Hill Giant to
 * Llanowar Elves", or "Bonesplitter is attached to Llanowar Elves" when it hung on nothing. Null for any other event,
 * and for one that attaches to the same host again, which keep their usual line.
 */
function attachLine(event: Event | string, was: HostRef | null, n: EventNames): LogLine | null {
  if (typeof event === 'string') return null
  const to: HostRef | null =
    'Attached' in event
      ? { card: event.Attached.to }
      : 'AttachedToPlayer' in event
        ? { player: event.AttachedToPlayer.player }
        : null
  const object =
    'Attached' in event
      ? event.Attached.object
      : 'AttachedToPlayer' in event
        ? event.AttachedToPlayer.object
        : null
  if (to === null || object === null || sameHost(was, to)) return null
  const name = (host: HostRef): string => ('card' in host ? n.card(host.card) : n.seat(host.player))
  if (was === null) return 'card' in to ? line(`${n.card(object)} is attached to ${name(to)}`) : null
  return line(`${n.card(object)} moves from ${name(was)} to ${name(to)}`)
}

/**
 * The log line for one event as this viewer saw it. `context` is what the batch around the event tells (the resolving
 * spell, the targets of an activation); `hostBefore` is where the attachment of an attach event hung before it (null:
 * on nothing), when the caller knows; without it the line only says what became attached.
 */
export function describeEvent(
  event: EventView,
  names: EventNames,
  context?: EventContext,
  hostBefore?: HostRef | null,
): LogLine {
  if (hostBefore !== undefined && 'Public' in event) {
    const moved = attachLine(event.Public, hostBefore, names)
    if (moved !== null) return moved
  }
  return 'Public' in event
    ? describePublic(event.Public, names, context)
    : describeHidden(event.Hidden, names)
}
