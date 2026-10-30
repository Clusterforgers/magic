/** The game log as sentences: one line for every `EventView`, in the words a player would use. */
import { describeLoss } from '@/features/game/outcome'
import type {
  AttackTarget,
  Choice,
  ContinuousEffect,
  CounterKind,
  DamageTarget,
  DeathCause,
  Event,
  EventView,
  HiddenEvent,
  ManualAction,
  ManualZone,
  Modification,
  ObjectId,
  ObjectRef,
  PlayerId,
  RuleEffect,
  Target,
  Zone,
  ZoneCause,
} from '@/protocol/generated'
import { colorsInSet, manaLetter } from './cardModel'
import type { EventContext } from './eventContext'
import { distinctTargets } from './prompt/targetText'
import { STEP_LABEL } from './turnText'

/** How the log names things; both fall back to a plain word when the thing is unknown. */
export interface EventNames {
  seat(seat: PlayerId): string
  card(id: ObjectId): string
  /** The clauses of a card's rules that are left to the table, when the viewer knows them. */
  clauses?(id: ObjectId): readonly string[]
}

/** One log line. Minor lines (priority passes, mana, untaps) are drawn quietly. */
export interface LogLine {
  text: string
  minor: boolean
}

const line = (text: string): LogLine => ({ text, minor: false })
const quiet = (text: string): LogLine => ({ text, minor: true })

const cards = (count: number): string => `${count} ${count === 1 ? 'card' : 'cards'}`

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
      SwitchPowerToughness: `${group ? 'have' : 'has'} power and toughness switched`,
      CantAttack: "can't attack",
      CantBlock: "can't block",
      DoesntUntap: `${group ? "don't" : "doesn't"} untap during their controller's untap step`,
      CantBeBlocked: "can't be blocked",
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
  if ('AddSubtype' in modification) return `${becomes} ${modification.AddSubtype} too`
  if ('SetCreatureTypes' in modification) {
    return `${becomes} ${joinWords(modification.SetCreatureTypes)}`
  }
  if ('SetColors' in modification) {
    const colors = colorsInSet(modification.SetColors).map((color) => color.toLowerCase())
    return `${becomes} ${colors.length === 0 ? 'colorless' : joinWords(colors)}`
  }
  if ('AddKeyword' in modification) return `${gains} ${keywordWords(modification.AddKeyword)}`
  if ('RemoveKeyword' in modification) return `${loses} ${keywordWords(modification.RemoveKeyword)}`
  if ('GrantAbility' in modification) return `${gains} an ability`
  if ('SetController' in modification) return `${group ? 'change' : 'changes'} controller`
  if ('SetPowerToughness' in modification) {
    const { power, toughness } = modification.SetPowerToughness
    return `${becomes} ${power}/${toughness}`
  }
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
  return `${who} ${what}${durationWords(duration, source, n)}`
}

/** How long a change lasts, as the end of a sentence (nothing for a change that never ends). */
function durationWords(
  duration: ContinuousEffect['duration'],
  source: ObjectRef | null,
  n: EventNames,
): string {
  const UNTIL: Record<ContinuousEffect['duration'], string> = {
    UntilEndOfTurn: ' until end of turn',
    UntilEndOfCombat: ' until end of combat',
    UntilYourNextTurn: " until their controller's next turn",
    UntilEndOfYourNextTurn: " until the end of their controller's next turn",
    WhileSourceOnBattlefield:
      source === null ? ' while its source stays' : ` while ${n.card(source.id)} stays`,
    WhileYouControlSource:
      source === null
        ? ' while its controller controls its source'
        : ` while its controller controls ${n.card(source.id)}`,
    NextUntapStep: " through its controller's next untap step",
    Forever: '',
  }
  return UNTIL[duration]
}

/** What a change to a rule of the game says, for the players or cards it names. */
function describeRule(effect: RuleEffect, n: EventNames): string {
  const { rule, duration, source } = effect
  const named = (objects: readonly ObjectRef[]): string =>
    joinWords(objects.map((object) => n.card(object.id)))
  const seats = (players: readonly PlayerId[]): string => joinWords(players.map((player) => n.seat(player)))
  let what: string
  if (rule === 'GraveyardCardsUntargetable') what = "Cards in graveyards can't be targeted"
  else if ('MustAttack' in rule) what = `${named(rule.MustAttack)} must attack if able`
  else if ('MustBlock' in rule) what = `${named(rule.MustBlock)} must block if able`
  else if ('CantBeCountered' in rule) what = `${named(rule.CantBeCountered)} can't be countered`
  else if ('CantGainLife' in rule) what = `${seats(rule.CantGainLife)} can't gain life`
  else if ('NoLifeLossFromDamage' in rule)
    what = `Damage doesn't cause ${seats(rule.NoLifeLossFromDamage)} to lose life`
  else if ('RadiationGainsLife' in rule)
    what = `Radiation makes ${seats(rule.RadiationGainsLife)} gain life rather than lose life`
  else if ('CantDraw' in rule) what = `${seats(rule.CantDraw)} can't draw cards`
  else if ('CantSearch' in rule) what = `${seats(rule.CantSearch)} can't search libraries`
  else if ('VoteAgain' in rule) what = `${seats(rule.VoteAgain)} may vote an additional time`
  else if ('CantLoseGame' in rule) what = `${seats(rule.CantLoseGame.who)} can't lose the game`
  else if ('CantCastSpells' in rule) what = `${seats(rule.CantCastSpells.who)} can't cast spells`
  else if ('ToughnessAssignsDamage' in rule)
    what = `Creatures of ${seats(rule.ToughnessAssignsDamage)} deal combat damage by toughness`
  else if ('RevealTop' in rule)
    what = `${seats(rule.RevealTop)} play with the top card of their library revealed`
  else if ('MayCast' in rule) {
    const { who, only, play } = rule.MayCast
    what = `${seats(who)} may ${play ? 'play' : 'cast'} ${only === null ? 'cards they could not' : named(only)}`
  } else if ('DamageCantBePrevented' in rule) what = "Damage can't be prevented"
  else if ('PreventDamage' in rule) {
    const { left } = rule.PreventDamage
    what = left === null ? 'Damage is prevented' : `The next ${left} damage is prevented`
  } else if ('Regenerate' in rule) what = `${n.card(rule.Regenerate.id)} gets a regeneration shield`
  else if ('BlockLimitEach' in rule) {
    const { most, who } = rule.BlockLimitEach
    what = `${seats(who)} can't block with more than ${most} ${most === 1 ? 'creature' : 'creatures'}`
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

/**
 * What a seat did by hand, as one sentence; the events of the change follow it in the log. `from` is the zone a moved
 * card left, which the action does not name and the change does (`foldLog` passes it).
 */
export function describeManual(player: PlayerId, action: ManualAction, n: EventNames, from?: Zone): string {
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
    const name = 'Common' in token ? token.Common : token.Custom.name
    return `${who} creates ${count} ${name} ${count === 1 ? 'token' : 'tokens'} (by hand)`
  }
  if ('Tap' in action) return `${who} taps ${n.card(action.Tap.object)} (by hand)`
  if ('Untap' in action) return `${who} untaps ${n.card(action.Untap.object)} (by hand)`
  if ('Draw' in action) return `${who} draws ${cards(action.Draw.count)} (by hand)`
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
  const { object, power, toughness } = action.Pump
  const signed = (value: number): string => (value < 0 ? `${value}` : `+${value}`)
  return `${who} gives ${n.card(object)} ${signed(power)}/${signed(toughness)} until end of turn (by hand)`
}

/** A card moved by hand that the viewer cannot name, in the words of `describeManual`; `from` is the zone it left. */
export function describeHiddenManual(player: PlayerId, to: ManualZone, n: EventNames, from?: Zone): string {
  const left = from === undefined ? '' : ` from ${leftWord(from)}`
  return `${n.seat(player)} moves a card${left} to ${MANUAL_ZONE[to]} (by hand)`
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
  return 'chooses which effect applies first'
}

/** The words for a cause that is a plain name; a card hidden with hideaway or exiled until a permanent leaves carries that permanent. */
const BECAUSE: Record<
  Exclude<
    ZoneCause,
    | { Hideaway: unknown }
    | { ExiledUntil: unknown }
    | { ExiledUntilMonarch: unknown }
    | { Encoded: unknown }
    | { Manifested: unknown }
    | { Cloaked: unknown }
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
  Adventure: 'is exiled on an adventure',
}

/** The words for a cause that carries a player or a permanent. */
function objectCause(cause: Extract<ZoneCause, object>): string {
  if ('Hideaway' in cause) return 'is exiled face down'
  if ('Encoded' in cause) return 'is exiled encoded on a creature'
  if ('Manifested' in cause) return 'is manifested'
  if ('Cloaked' in cause) return 'is cloaked'
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
  'ResolutionEnded',
  'ZoneChanged',
  'LifeGained',
  'LifeLost',
  'CountersChanged',
  'TokenCreated',
  'LibraryLooked',
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
  by: string,
  name: string,
  from: Zone,
  to: Zone,
  cause: ZoneCause,
  n: EventNames,
): string | null {
  if (cause === 'Exiled' || cause === 'Destroyed') return `${name} ${BECAUSE[cause]} by ${by}`
  if (cause === 'Buried') return `${name} is destroyed by ${by} and can't be regenerated`
  const bounced = cause === 'Moved' && from === 'Battlefield' && typeof to !== 'string' && 'Hand' in to
  return bounced ? `${by} returns ${name} to ${zoneWord(to, n)}` : null
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
  if ('ResolutionBegan' in event) return quiet(`${n.card(event.ResolutionBegan.object)} begins to resolve`)
  if ('ChoiceMade' in event) {
    const { player, choice } = event.ChoiceMade
    return quiet(`${n.seat(player)} ${describeChoice(choice, n)}`)
  }
  if ('StepDone' in event) return quiet('A step of the resolution is done')
  if ('ResolutionEnded' in event) {
    const { object, outcome } = event.ResolutionEnded
    if (outcome === 'Fizzled') return line(`${n.card(object)} does nothing: its targets are gone`)
    if (outcome === 'Countered') {
      const by = context?.resolving
      return line(
        by === undefined ? `${n.card(object)} is countered` : `${n.card(by)} counters ${n.card(object)}`,
      )
    }
    if (outcome === 'Lapsed') return line(`${n.card(object)} does nothing: its condition is no longer true`)
    return quiet(`${n.card(object)} finishes resolving`)
  }
  if ('ZoneChanged' in event) {
    const { object, from, to, cause } = event.ZoneChanged
    const by = context?.resolving
    if (by !== undefined && by !== object) {
      const done = byResolving(n.card(by), n.card(object), from, to, cause, n)
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
    return line(`${n.seat(event.TokenCreated.controller)} creates a ${event.TokenCreated.name} token`)
  }
  if ('LibraryLooked' in event) {
    const { player, cards: looked } = event.LibraryLooked
    return line(`${n.seat(player)} looks at ${cards(looked.length)} of their library`)
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
  if ('LibraryShuffled' in event)
    return quiet(`${n.seat(event.LibraryShuffled.player)} shuffles their library`)
  if ('HandDealt' in event) {
    return line(`${n.seat(event.HandDealt.player)} is dealt ${cards(event.HandDealt.cards.length)}`)
  }
  if ('MulliganDeclared' in event) return line(`${n.seat(event.MulliganDeclared.player)} decides to mulligan`)
  if ('MulliganTaken' in event) return line(`${n.seat(event.MulliganTaken.player)} takes a mulligan`)
  if ('MulliganKept' in event) return line(`${n.seat(event.MulliganKept.player)} keeps their hand`)
  if ('CardsBottomed' in event) {
    const { player, cards: bottomed } = event.CardsBottomed
    return line(`${n.seat(player)} puts ${cards(bottomed.length)} on the bottom of their library`)
  }
  if ('LibraryBottomed' in event) {
    const { player, cards: bottomed } = event.LibraryBottomed
    return line(`${n.seat(player)} puts ${cards(bottomed.length)} on the bottom of their library`)
  }
  if ('ChosenAtRandom' in event) {
    return line(`${event.ChosenAtRandom.cards.map((id) => n.card(id)).join(', ')} is picked at random`)
  }
  if ('Entered' in event) {
    const { object, tapped, controller, chosen } = event.Entered
    const chose =
      chosen === undefined
        ? ''
        : 'Chapter' in chosen
          ? `, reading ahead to chapter ${chosen.Chapter}`
          : `, choosing ${'Color' in chosen ? chosen.Color.toLowerCase() : 'Player' in chosen ? n.seat(chosen.Player) : chosen.CreatureType}`
    return quiet(
      `${n.card(object)} enters${tapped ? ' tapped' : ''} under ${n.seat(controller)}'s control${chose}`,
    )
  }
  if ('Attached' in event) {
    return line(`${n.card(event.Attached.object)} becomes attached to ${n.card(event.Attached.to)}`)
  }
  if ('Transformed' in event) return line(`${n.card(event.Transformed.object)} transforms`)
  if ('Flipped' in event) return line(`${n.card(event.Flipped.object)} flips`)
  if ('Prepared' in event) return line(`${n.card(event.Prepared.object)} becomes prepared`)
  if ('Unprepared' in event) return line(`${n.card(event.Unprepared.object)} is no longer prepared`)
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
    return line(`${n.seat(controller)} copies the ability of ${n.card(of.id)}`)
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
  if ('DieRolled' in event) {
    const { player, sides, result } = event.DieRolled
    return line(`${n.seat(player)} rolls a d${sides}: ${result}`)
  }
  if ('CoinFlipped' in event) {
    const { player, won } = event.CoinFlipped
    return line(`${n.seat(player)} ${won ? 'wins' : 'loses'} a coin flip`)
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
  if ('CitysBlessingGained' in event) {
    return line(`${n.seat(event.CitysBlessingGained.player)} gets the city's blessing`)
  }
  if ('Explored' in event) return line(`${n.card(event.Explored.object)} explores`)
  if ('Connived' in event) return line(`${n.card(event.Connived.object)} connives`)
  if ('Exerted' in event) return line(`${n.card(event.Exerted.object)} is exerted`)
  if ('ExertEnded' in event) return line(`${n.card(event.ExertEnded.object)} is no longer exerted`)
  if ('Amassed' in event) return line(`${n.seat(event.Amassed.player)} amasses`)
  if ('Monstrous' in event) return line(`${n.card(event.Monstrous.object)} becomes monstrous`)
  if ('Devoured' in event) return line(`${n.card(event.Devoured.object)} devours ${event.Devoured.count}`)
  if ('Renowned' in event) return line(`${n.card(event.Renowned.object)} becomes renowned`)
  if ('Exploited' in event)
    return line(`${n.card(event.Exploited.object)} exploits ${n.card(event.Exploited.sacrificed)}`)
  if ('Crewed' in event) {
    const { object, creatures, saddle } = event.Crewed
    const by = creatures.length > 0 ? ` by ${joinWords(creatures.map((id) => n.card(id)))}` : ''
    return line(`${n.card(object)} is ${saddle ? 'saddled' : 'crewed'}${by}`)
  }
  if ('TurnedFaceUp' in event)
    return line(`${n.seat(event.TurnedFaceUp.player)} turns ${n.card(event.TurnedFaceUp.object)} face up`)
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
  if ('JoinedAttack' in event) {
    const { object, target } = event.JoinedAttack
    return line(`${n.card(object)} enters attacking ${attackTarget(target, n)}`)
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
  if ('PhasesAdded' in event) {
    return line(`${event.PhasesAdded.steps.length} additional phase(s) follow this phase`)
  }
  if ('StepBegan' in event) return quiet(`${STEP_LABEL[event.StepBegan.step]} begins`)
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
  if ('ManaAdded' in event) {
    const { player, mana, source } = event.ManaAdded
    const from = source === null ? '' : ` from ${n.card(source)}`
    return quiet(`${n.seat(player)} adds {${manaLetter(mana)}}${from}`)
  }
  if ('ManaSpent' in event) {
    const symbols = event.ManaSpent.mana.map((mana) => `{${manaLetter(mana)}}`).join('')
    return quiet(`${n.seat(event.ManaSpent.player)} spends ${symbols}`)
  }
  if ('ManaPoolEmptied' in event) return quiet(`${n.seat(event.ManaPoolEmptied.player)}'s mana pool empties`)
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
    if (attackers.length === 0) return line(`${n.seat(player)} declares no attackers`)
    return line(`${n.seat(player)} attacks ${attackClauses(attackers, n)}`)
  }
  if ('BlockersDeclared' in event) {
    const { player, blocks } = event.BlockersDeclared
    if (blocks.length === 0) return line(`${n.seat(player)} declares no blockers`)
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
  if ('CreatureDied' in event) {
    const HOW: Record<DeathCause, string> = {
      LethalDamage: 'from lethal damage',
      Deathtouch: 'from deathtouch',
      ZeroToughness: 'with zero toughness',
    }
    return line(`${n.card(event.CreatureDied.object)} dies ${HOW[event.CreatureDied.cause]}`)
  }
  if ('PlayerLost' in event) {
    return line(`${n.seat(event.PlayerLost.player)} ${describeLoss(event.PlayerLost.reason)}`)
  }
  if ('ObjectLeftGame' in event) return quiet(`${n.card(event.ObjectLeftGame.object)} leaves the game`)
  if ('GameEnded' in event) {
    const { result } = event.GameEnded
    return line(result === 'Draw' ? 'The game ends in a draw' : `${n.seat(result.Winner)} wins the game`)
  }
  if ('ManualAction' in event) {
    return line(describeManual(event.ManualAction.player, event.ManualAction.action, n))
  }
  const unhandled: never = event
  return line(String(unhandled))
}

function describeHidden(event: HiddenEvent, n: EventNames): LogLine {
  if (typeof event === 'string') return quiet('A card leaves the game')
  if ('CardDrawn' in event) return line(`${n.seat(event.CardDrawn.player)} draws a card`)
  if ('HandDealt' in event)
    return line(`${n.seat(event.HandDealt.player)} is dealt ${cards(event.HandDealt.count)}`)
  if ('LibraryShuffled' in event)
    return quiet(`${n.seat(event.LibraryShuffled.player)} shuffles their library`)
  if ('LibraryRefilled' in event) {
    return quiet(`${n.seat(event.LibraryRefilled.player)} shuffles their graveyard into their library`)
  }
  if ('MulliganTaken' in event) return line(`${n.seat(event.MulliganTaken.player)} takes a mulligan`)
  if ('CardsBottomed' in event) {
    const { player, count } = event.CardsBottomed
    return line(`${n.seat(player)} puts ${cards(count)} on the bottom of their library`)
  }
  if ('LibraryLooked' in event) {
    const { player, count } = event.LibraryLooked
    return line(`${n.seat(player)} looks at ${cards(count)} of their library`)
  }
  if ('CardsFound' in event)
    return line(`${n.seat(event.CardsFound.player)} finds ${cards(event.CardsFound.count)}`)
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
    return line(`An ability of ${n.card(event.AbilityTriggered.source.id)} triggers`)
  }
  if ('AbilityPutOnStack' in event) {
    return quiet(`An ability of ${n.card(event.AbilityPutOnStack.source.id)} goes on the stack`)
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
  void (event satisfies { StepDone: unknown })
  return quiet('A step of the resolution is done')
}

/** The log line for one event as this viewer saw it. */
export function describeEvent(event: EventView, names: EventNames, context?: EventContext): LogLine {
  return 'Public' in event
    ? describePublic(event.Public, names, context)
    : describeHidden(event.Hidden, names)
}
