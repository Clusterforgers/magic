/** The game log as sentences: one line for every `EventView`, in the words a player would use. */
import { describeLoss } from '@/features/game/outcome'
import type {
  AttackTarget,
  Choice,
  ContinuousEffect,
  DamageTarget,
  Event,
  EventView,
  HiddenEvent,
  ObjectId,
  PlayerId,
  Target,
  Zone,
  ZoneCause,
} from '@/protocol/generated'
import { colorsInSet, manaLetter } from './cardModel'
import { STEP_LABEL } from './turnText'

/** How the log names things; both fall back to a plain word when the thing is unknown. */
export interface EventNames {
  seat(seat: PlayerId): string
  card(id: ObjectId): string
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

function describeEffect(effect: ContinuousEffect, n: EventNames): string {
  const { selector, modification, duration } = effect
  let who = 'All creatures'
  if (selector !== 'AllCreatures') {
    who =
      'Object' in selector
        ? n.card(selector.Object)
        : `Creatures controlled by ${n.seat(selector.CreaturesControlledBy)}`
  }
  // One card "becomes" and "gets"; a group of creatures "become" and "get".
  const group = typeof selector === 'string' || 'CreaturesControlledBy' in selector
  const becomes = group ? 'become' : 'becomes'
  let what = 'changes'
  if ('AddType' in modification) what = `${becomes} ${modification.AddType.toLowerCase()} too`
  else if ('SetColors' in modification) {
    const colors = colorsInSet(modification.SetColors).map((color) => color.toLowerCase())
    what = `${becomes} ${colors.length === 0 ? 'colorless' : joinWords(colors)}`
  } else if ('SetPowerToughness' in modification) {
    const { power, toughness } = modification.SetPowerToughness
    what = `${becomes} ${power}/${toughness}`
  } else {
    const { power, toughness } = modification.ModifyPowerToughness
    what = `${group ? 'get' : 'gets'} ${power >= 0 ? '+' : ''}${power}/${toughness >= 0 ? '+' : ''}${toughness}`
  }
  const until =
    duration === 'UntilEndOfTurn'
      ? ' until end of turn'
      : ` while ${n.card(duration.WhileSourceOnBattlefield)} stays`
  return `${who} ${what}${until}`
}

function attackTarget(target: AttackTarget, n: EventNames): string {
  return 'Player' in target ? n.seat(target.Player) : n.card(target.Planeswalker)
}

function damageTarget(target: DamageTarget, n: EventNames): string {
  return 'Player' in target ? n.seat(target.Player) : n.card(target.Object)
}

function targetName(target: Target, n: EventNames): string {
  return 'Player' in target ? n.seat(target.Player) : n.card(target.Object.id)
}

function zoneWord(zone: Zone, n: EventNames): string {
  if (zone === 'Battlefield') return 'the battlefield'
  if (zone === 'Stack') return 'the stack'
  if (zone === 'Exile') return 'exile'
  if ('Library' in zone) return `${n.seat(zone.Library)}'s library`
  if ('Hand' in zone) return `${n.seat(zone.Hand)}'s hand`
  if ('Graveyard' in zone) return `${n.seat(zone.Graveyard)}'s graveyard`
  return `${n.seat(zone.Command)}'s command zone`
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
  return 'chooses which effect applies first'
}

const BECAUSE: Record<ZoneCause, string> = {
  Moved: 'moves',
  Destroyed: 'is destroyed',
  Sacrificed: 'is sacrificed',
  Exiled: 'is exiled',
  Countered: 'is countered',
  Milled: 'is milled',
}

/** The events of spells and abilities: abilities on the stack, the steps of a resolution, life, zones and what players learn. */
const PIPELINE = [
  'AbilityPutOnStack',
  'AbilityActivated',
  'AbilityTriggered',
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

function describePipeline(event: PipelineEvent, n: EventNames): LogLine {
  if ('AbilityPutOnStack' in event) {
    return quiet(`An ability of ${n.card(event.AbilityPutOnStack.source.id)} goes on the stack`)
  }
  if ('AbilityActivated' in event) {
    const { player, source } = event.AbilityActivated
    return line(`${n.seat(player)} activates an ability of ${n.card(source.id)}`)
  }
  if ('AbilityTriggered' in event) {
    return line(`An ability of ${n.card(event.AbilityTriggered.source.id)} triggers`)
  }
  if ('ResolutionBegan' in event) return quiet(`${n.card(event.ResolutionBegan.object)} begins to resolve`)
  if ('ChoiceMade' in event) {
    const { player, choice } = event.ChoiceMade
    return quiet(`${n.seat(player)} ${describeChoice(choice, n)}`)
  }
  if ('StepDone' in event) return quiet('A step of the resolution is done')
  if ('ResolutionEnded' in event) {
    const { object, outcome } = event.ResolutionEnded
    if (outcome === 'Fizzled') return line(`${n.card(object)} does nothing: its targets are gone`)
    if (outcome === 'Countered') return line(`${n.card(object)} is countered`)
    return quiet(`${n.card(object)} finishes resolving`)
  }
  if ('ZoneChanged' in event) {
    const { object, to, cause } = event.ZoneChanged
    return line(
      cause === 'Moved'
        ? `${n.card(object)} moves to ${zoneWord(to, n)}`
        : `${n.card(object)} ${BECAUSE[cause]}`,
    )
  }
  if ('LifeGained' in event)
    return line(`${n.seat(event.LifeGained.player)} gains ${event.LifeGained.amount} life`)
  if ('LifeLost' in event) return line(`${n.seat(event.LifeLost.player)} loses ${event.LifeLost.amount} life`)
  if ('CountersChanged' in event) {
    const { on, delta } = event.CountersChanged
    return quiet(`${delta > 0 ? 'Counters are put on' : 'Counters are removed from'} ${targetName(on, n)}`)
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

function describePublic(event: Event, n: EventNames): LogLine {
  if (isPipeline(event)) return describePipeline(event, n)
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
  if ('TurnBegan' in event) {
    return line(`Turn ${event.TurnBegan.number} begins: ${n.seat(event.TurnBegan.active)}'s turn`)
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
  if ('SpellCast' in event)
    return line(`${n.seat(event.SpellCast.player)} casts ${n.card(event.SpellCast.object)}`)
  if ('SpellResolved' in event) return line(`${n.card(event.SpellResolved.object)} resolves`)
  if ('PermanentTapped' in event) return quiet(`${n.card(event.PermanentTapped.object)} becomes tapped`)
  if ('PermanentUntapped' in event) return quiet(`${n.card(event.PermanentUntapped.object)} becomes untapped`)
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
  if ('LifePaid' in event) return line(`${n.seat(event.LifePaid.player)} pays ${event.LifePaid.amount} life`)
  if ('CardDrawn' in event)
    return line(`${n.seat(event.CardDrawn.player)} draws ${n.card(event.CardDrawn.object)}`)
  if ('DrewFromEmptyLibrary' in event) {
    return line(`${n.seat(event.DrewFromEmptyLibrary.player)} tries to draw from an empty library`)
  }
  if ('CardDiscarded' in event) {
    return line(`${n.seat(event.CardDiscarded.player)} discards ${n.card(event.CardDiscarded.object)}`)
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
  if ('CreatureDied' in event) {
    const how = event.CreatureDied.cause === 'LethalDamage' ? 'from lethal damage' : 'with zero toughness'
    return line(`${n.card(event.CreatureDied.object)} dies ${how}`)
  }
  if ('PlayerLost' in event) {
    return line(`${n.seat(event.PlayerLost.player)} ${describeLoss(event.PlayerLost.reason)}`)
  }
  if ('ObjectLeftGame' in event) return quiet(`${n.card(event.ObjectLeftGame.object)} leaves the game`)
  if ('GameEnded' in event) {
    const { result } = event.GameEnded
    return line(result === 'Draw' ? 'The game ends in a draw' : `${n.seat(result.Winner)} wins the game`)
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
  if ('ZoneChanged' in event) return quiet(`A card moves to ${zoneWord(event.ZoneChanged.to, n)}`)
  if ('ChoiceMade' in event) return quiet(`${n.seat(event.ChoiceMade.player)} chooses`)
  void (event satisfies { StepDone: unknown })
  return quiet('A step of the resolution is done')
}

/** The log line for one event as this viewer saw it. */
export function describeEvent(event: EventView, names: EventNames): LogLine {
  return 'Public' in event ? describePublic(event.Public, names) : describeHidden(event.Hidden, names)
}
