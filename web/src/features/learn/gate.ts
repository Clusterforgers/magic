import type {
  Answer,
  Decision,
  DecisionKind,
  ObjectId,
  ObjectView,
  PriorityOptions,
  Target,
} from '@/protocol/generated'
import { decisionName } from '@/features/table/turnText'
import type { Allow } from './step'
import { ALLOW_DECISION } from './step'

/** The objects of the table by id, to read a card's name. */
export type Objects = Readonly<Record<number, ObjectView | undefined>>

const nameOf = (objects: Objects, id: ObjectId): string => objects[id]?.name ?? ''

const targetName = (objects: Objects, target: Target): string | null =>
  'Object' in target ? nameOf(objects, target.Object.id) : null

/** Whether `answer` is the input `allow` lets through. */
export function allowed(allow: Allow | undefined, answer: Answer, objects: Objects): boolean {
  if (allow === undefined || answer === 'Undo') return false
  switch (allow.kind) {
    case 'keep':
      return 'Mulligan' in answer && answer.Mulligan === 'Keep'
    case 'pass':
      return 'Priority' in answer && answer.Priority === 'Pass'
    case 'land': {
      if (!('Priority' in answer) || typeof answer.Priority !== 'object') return false
      return 'PlayLand' in answer.Priority && nameOf(objects, answer.Priority.PlayLand) === allow.name
    }
    case 'cast': {
      if (!('Priority' in answer) || typeof answer.Priority !== 'object') return false
      return 'CastSpell' in answer.Priority && nameOf(objects, answer.Priority.CastSpell) === allow.name
    }
    case 'pay': {
      if (!('PayMana' in answer)) return false
      const confirmed = typeof answer.PayMana === 'object' && 'Manual' in answer.PayMana
      // Pay for me proposes a payment that Confirm sends as it stands, or pays at once.
      if (allow.how === 'auto') return answer.PayMana === 'Auto' || confirmed
      return confirmed
    }
    case 'attack': {
      if (!('DeclareAttackers' in answer)) return false
      const names = answer.DeclareAttackers.map(([id]) => nameOf(objects, id)).toSorted()
      return names.join('|') === [...allow.names].toSorted().join('|')
    }
    case 'block': {
      if (!('DeclareBlockers' in answer)) return false
      const [only, ...rest] = answer.DeclareBlockers
      return (
        only !== undefined &&
        rest.length === 0 &&
        nameOf(objects, only[0]) === allow.blocker &&
        nameOf(objects, only[1]) === allow.attacker
      )
    }
    case 'target': {
      if (!('ChooseTargets' in answer) || typeof answer.ChooseTargets !== 'object') return false
      if (!('Targets' in answer.ChooseTargets)) return false
      const [only, ...rest] = answer.ChooseTargets.Targets
      return only !== undefined && rest.length === 0 && targetName(objects, only) === allow.name
    }
    case 'choose':
      return 'YesNo' in answer && answer.YesNo === allow.yes
    case 'endTurn':
    case 'stop':
    case 'look':
    case 'manual':
      return false
  }
}

/** The priority offer of a step: only what `allow` asks for stays legal; the rest is held with the reason `Lesson`. */
function narrowPriority(options: PriorityOptions, allow: Allow | undefined, objects: Objects): HeldOptions {
  const keep = (ids: readonly ObjectId[], kind: 'land' | 'cast'): ObjectId[] =>
    ids.filter((id) => allow?.kind === kind && nameOf(objects, id) === allow.name)
  const castable = keep(options.castable, 'cast')
  const playable_lands = keep(options.playable_lands, 'land')
  const held = [...options.castable, ...options.playable_lands].filter(
    (id) => !castable.includes(id) && !playable_lands.includes(id),
  )
  return {
    ...options,
    castable,
    playable_lands,
    mana_abilities: [],
    abilities: [],
    suspendable: [],
    foretellable: [],
    plottable: [],
    turnable: [],
    unlockable: [],
    endable: [],
    ignorable: [],
    faces: [],
    companion: false,
    held,
  }
}

/** A priority offer with the cards a lesson holds back (`handReason.ts`, reason `Lesson`). */
export type HeldOptions = PriorityOptions & { held?: readonly ObjectId[] }

/**
 * `decision` as the table should draw it during a step that lets `allow` through: a card the step does not ask for is
 * not legal, an attacker, blocker or target the step does not name is not offered. Other kinds are left as they are;
 * the gate still checks every answer.
 */
export function narrow(decision: Decision, allow: Allow | undefined, objects: Objects): Decision {
  const kind: DecisionKind = decision.kind
  const named = (id: ObjectId, names: readonly string[]): boolean => names.includes(nameOf(objects, id))
  if ('Priority' in kind)
    return { ...decision, kind: { Priority: narrowPriority(kind.Priority, allow, objects) } }
  if ('DeclareAttackers' in kind && allow?.kind === 'attack') {
    const options = kind.DeclareAttackers
    return {
      ...decision,
      kind: {
        DeclareAttackers: {
          ...options,
          attackers: options.attackers.filter((option) => named(option.object, allow.names)),
        },
      },
    }
  }
  if ('DeclareBlockers' in kind && allow?.kind === 'block') {
    const options = kind.DeclareBlockers
    return {
      ...decision,
      kind: {
        DeclareBlockers: {
          ...options,
          blockers: options.blockers
            .filter((option) => named(option.object, [allow.blocker]))
            .map((option) => ({
              ...option,
              can_block: option.can_block.filter((id) => named(id, [allow.attacker])),
            })),
        },
      },
    }
  }
  if ('ChooseTargets' in kind && allow?.kind === 'target') {
    const options = kind.ChooseTargets
    return {
      ...decision,
      kind: {
        ChooseTargets: {
          ...options,
          legal: options.legal.filter((target) => targetName(objects, target) === allow.name),
        },
      },
    }
  }
  return decision
}

/** Whether a step that lets `allow` through waits on a decision of kind `decision`. */
export function stepFits(allow: Allow | undefined, decision: Decision | null): boolean {
  if (allow === undefined) return false
  const wanted = ALLOW_DECISION[allow.kind]
  if (wanted === null) return true
  return decision !== null && decisionName(decision.kind) === wanted
}

/** What the coach says by default when an input is held back. */
export function defaultHint(allow: Allow | undefined): string {
  switch (allow?.kind) {
    case 'keep':
      return 'Not yet: keep this hand first.'
    case 'land':
      return `Not yet: play the ${allow.name} first.`
    case 'cast':
      return `Not yet: cast ${allow.name} first.`
    case 'pass':
      return 'Not yet: pass first.'
    case 'pay':
      return allow.how === 'auto'
        ? 'Not yet: let the game pay first.'
        : 'Not yet: tap your lands to pay first.'
    case 'endTurn':
      return 'Not yet: end the turn first.'
    case 'attack':
      return `Not yet: attack with ${allow.names.join(' and ')} first.`
    case 'block':
      return `Not yet: block with ${allow.blocker} first.`
    case 'target':
      return `Not yet: target ${allow.name} first.`
    case 'choose':
      return 'Not yet: answer the question first.'
    case 'stop':
      return 'Not yet: set the stop first.'
    case 'manual':
      return 'Not yet: use the Commands panel first.'
    case 'look':
    case undefined:
      return 'Not in this step of the lesson.'
  }
}
