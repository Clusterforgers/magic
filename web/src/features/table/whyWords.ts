/**
 * The plain sentence for why a card or ability cannot be played now (docs/ui-design.md, Playing is legible). Pure: it
 * reads the engine's reason and its detail (`Why`), the card's name and its rules text, and nothing else.
 */
import type {
  Color,
  HostBy,
  ManaCost,
  ManaType,
  ObjectView,
  Refusal,
  Unplayable,
  Why,
} from '@/protocol/generated'
import { costText } from './cardModel'

/** Whether the sentence is about casting a card or activating one of its abilities. */
export type Doing = 'cast' | 'activate'

/** What the sentence speaks of: the card's name, its rules text when known, and whether it is cast or activated. */
export interface Subject {
  name: string
  rulesText?: string | null
  doing: Doing
}

const MANA_SYMBOL: Readonly<Record<ManaType, string>> = {
  White: '{W}',
  Blue: '{U}',
  Black: '{B}',
  Red: '{R}',
  Green: '{G}',
  Colorless: '{C}',
}

/** The line each coarse reason says when the engine gives no detail. */
const PLAIN: Readonly<Record<Unplayable, string>> = {
  Timing: 'Sorcery speed: only in your main phase with an empty stack.',
  LandPlayed: 'Land already played this turn.',
  Mana: 'Not enough mana.',
  Targets: 'No legal target.',
  Cost: 'A cost cannot be paid.',
  Restricted: 'An effect forbids it now.',
}

/** The first sentence of `text` that begins with one of `starts`, as printed. */
function sentence(text: string | null | undefined, starts: readonly string[]): string | null {
  if (text === null || text === undefined) return null
  for (const line of text.split('\n')) {
    for (const part of line.split(/(?<=\.)\s+/)) {
      const trimmed = part.trim().replace(/^\(|\)$/g, '')
      if (starts.some((start) => trimmed.startsWith(start))) return trimmed
    }
  }
  return null
}

/** "a creature spell you control on the stack" → "a creature": what to cast first so it can be targeted. */
function spellToCast(needs: string): string {
  const bare = needs
    .replace(/ on the stack$/, '')
    .replace(/ you control$/, '')
    .replace(/ spell\b/, '')
    .trim()
  return bare === 'a' || bare === 'an' || bare === '' ? 'a spell' : bare
}

/** The verb the card does to its target, read off its rules text: copy, counter, else target. */
function verbOf(rulesText: string | null | undefined): string {
  const text = (rulesText ?? '').toLowerCase()
  if (/\bcopy target\b/.test(text)) return 'copy'
  if (/\bcounter target\b/.test(text)) return 'counter'
  return 'target'
}

/**
 * The hint for a common misreading, or null: a spell that targets a spell on the stack can be cast only while one is
 * there, which means keeping priority after casting your own (Full Control) or answering an opponent's.
 */
export function whyHint(subject: Subject, why: Why | undefined): string | null {
  if (why === undefined || typeof why === 'string' || !('Target' in why)) return null
  const { needs, zone } = why.Target
  if (zone !== 'Stack') return null
  const verb = verbOf(subject.rulesText)
  if (/\byou control\b/.test(needs)) {
    return `Cast ${spellToCast(needs)} first and keep priority (Full Control) to ${verb} it.`
  }
  return `Wait until one is cast, then ${verb} it while it is on the stack.`
}

/** A rule that lets each player cast `limit` spells a turn, which the viewer has: "One Voice: you have cast your spell this turn." */
function limitLine(source: string | undefined, limit: number): string {
  const spent =
    limit === 1 ? 'you have cast your spell this turn' : `you have cast your ${limit} spells this turn`
  return source === undefined
    ? `${spent.charAt(0).toUpperCase()}${spent.slice(1)}: no more are allowed.`
    : `${source}: ${spent}.`
}

/** An Aura, bestowed or mutating spell with nothing to go on: "Pacifism needs a creature on the battlefield to enchant." */
function hostLine(name: string, needs: string, by: HostBy): string {
  switch (by) {
    case 'Enchant':
      return `${name} needs ${needs} to enchant.`
    case 'Bestow':
      return `To cast ${name} for bestow, it needs ${needs} to enchant.`
    case 'Mutate':
      return `To mutate, ${name} needs ${needs}.`
  }
}

/** "X can't be 0" short at its least X: "X must be at least 1: that costs {1}{G}, and you have 1 mana available." */
function leastXLine(least: number, cost: ManaCost | undefined, atHand: number | undefined): string {
  if (cost === undefined)
    return `X must be at least ${least}, and the rest of its cost can't be paid for that.`
  return `X must be at least ${least}: that costs ${costText(cost)}, and you have ${atHand ?? 0} mana available.`
}

/** "Spend only mana of the chosen color": the color, and its mana against the cost. */
function chosenColorLine(
  name: string,
  color: Color | undefined,
  cost: ManaCost | undefined,
  atHand: number,
): string {
  if (color === undefined) {
    return `${name} has no chosen color, and only mana of the chosen color can be spent to activate it.`
  }
  const word = color.toLowerCase()
  if (cost === undefined)
    return `Only ${word} mana can be spent to activate it, and its cost asks for another color.`
  return `Only ${word} mana can be spent to activate it: needs ${costText(cost)}, and you have ${atHand} ${word} mana available.`
}

/** The short label on a card's foot when its spell has nothing to go on. */
export const HOST_LABEL: Readonly<Record<HostBy, string>> = {
  Enchant: 'Nothing to enchant',
  Bestow: 'Nothing to enchant',
  Mutate: 'Nothing to mutate onto',
}

/** The sentence for one detail, without the hint. */
function detailLine(subject: Subject, why: Why): string {
  const { name, doing } = subject
  const it = doing === 'cast' ? 'casting it' : 'activating it'
  if (typeof why === 'string') {
    switch (why) {
      case 'SorcerySpeed':
        return doing === 'cast'
          ? PLAIN.Timing
          : 'Activate only as a sorcery: in your main phase with an empty stack.'
      case 'OwnCondition':
        return (
          sentence(
            subject.rulesText,
            doing === 'cast' ? ['Cast this spell only', 'Cast ~ only'] : ['Activate only'],
          ) ??
          `${name} can't be ${doing === 'cast' ? 'cast' : 'activated'} now: its own condition does not hold.`
        )
      case 'SummoningSick':
        return `${name} came under your control this turn, so its {T} abilities wait until your next turn.`
      case 'Tapped':
        return `${name} is tapped.`
      case 'LimitSpent':
        return 'Already used as often as it allows this turn.'
      case 'OtherCost': {
        const extra = sentence(subject.rulesText, ['As an additional cost'])
        return extra === null ? 'A cost besides mana cannot be paid.' : `You can't pay its cost. ${extra}`
      }
      case 'NoWay':
        return `${name} can't be cast from here.`
    }
  }
  if ('Target' in why) return `${name} needs a target: ${why.Target.needs}.`
  if ('Host' in why) return hostLine(name, why.Host.needs, why.Host.by)
  if ('XTarget' in why) {
    const { needs, least, most } = why.XTarget
    const range = least === most ? `X = ${least}` : `X from ${least} to ${most}`
    return `${name} has no target for any X you can pay (${range}): it needs ${needs}.`
  }
  if ('LeastX' in why) return leastXLine(why.LeastX.least, why.LeastX.cost, why.LeastX.at_hand)
  if ('ChosenColor' in why) {
    const { color, cost, at_hand } = why.ChosenColor
    return chosenColorLine(name, color, cost, at_hand)
  }
  if ('Mana' in why) {
    const { cost, at_hand, missing, tax } = why.Mana
    const taxed = tax === undefined || tax === 0 ? '' : ` with {${tax}} commander tax`
    const lacking =
      missing === undefined || missing.length === 0
        ? ''
        : ` Nothing you have makes ${missing.map((kind) => MANA_SYMBOL[kind]).join(' or ')}.`
    return `Needs ${costText(cost)}${taxed}: you have ${at_hand} mana available.${lacking}`
  }
  if ('ForbiddenBy' in why) {
    const { name: source, limit } = why.ForbiddenBy
    if (limit !== undefined) return limitLine(source, limit)
    return source === undefined ? `An effect stops you ${it} now.` : `${source} stops you ${it} now.`
  }
  return `${why.SplitSecond.name} has split second: nothing can be cast or activated until it resolves.`
}

/** The whole line for a reason: the detail when the engine gives one, else the reason's plain line, then any hint. */
export function whyLine(subject: Subject, reason: Unplayable, why: Why | undefined): string {
  const main = why === undefined ? PLAIN[reason] : detailLine(subject, why)
  const hint = whyHint(subject, why)
  return hint === null ? main : `${main} ${hint}`
}

/** The notice for a play the server refused (`Error.refusal`): the card's name and why, as the card's note says it. */
export function refusalNotice(refusal: Refusal, objectsById: Readonly<Record<number, ObjectView>>): string {
  const object = objectsById[refusal.object]
  const name = object?.name || 'That card'
  const doing: Doing = refusal.ability === undefined ? 'cast' : 'activate'
  const line = whyLine({ name, rulesText: object?.rules_text, doing }, refusal.reason, refusal.why)
  if (refusal.why !== undefined) return line
  return `${name} can't be ${doing === 'cast' ? 'played' : 'activated'} now. ${line}`
}
