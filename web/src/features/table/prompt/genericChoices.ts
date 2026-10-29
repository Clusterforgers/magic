/**
 * The answers the interface can give to any decision, whatever its kind: what the generic prompt offers for a kind
 * that has no prompt of its own, so a player is never left with a decision and no button. It follows the answer the
 * server gives when a response clock runs out (a pass, no blocks, the first option, the order given, the cast taken
 * back) and, for a plain choice, adds one answer for each thing on offer.
 */
import type { Answer, DecisionKind, ObjectId, PlayerId, Target } from '@/protocol/generated'
import { optionLabel, modeLabel } from './choiceText'
import { divisionAnswer, evenShares } from './divideText'
import { distinctTargets, targetKey } from './targetText'

/** The most answers listed for one list of things on offer; a longer list is cut and says how many it left out. */
export const MOST_ANSWERS = 12

/** One answer the generic prompt offers, with the words on its button. */
export interface GenericChoice {
  key: string
  label: string
  answer: Answer
  /** The answer backs out of the cast or activation; the prompt draws it apart from the others. */
  cancel?: boolean
}

/** What the answers need to name things: cards by their name, seats by their player's name. */
export interface GenericWords {
  object(id: ObjectId): string
  seat(seat: PlayerId): string
  viewer: PlayerId | null
  /** "Cancel the spell" or "Cancel the ability", by what asks. */
  cancel: string
}

/** The answers on offer for a decision, and how many more things were on offer than the buttons show. */
export interface GenericOffer {
  choices: GenericChoice[]
  /** Things on offer that no button names, because the list is longer than `MOST_ANSWERS`. */
  unlisted: number
}

const pick = (key: string, label: string, answer: Answer): GenericChoice => ({ key, label, answer })
const back = (words: GenericWords, answer: Answer): GenericChoice => ({
  key: 'cancel',
  label: words.cancel,
  answer,
  cancel: true,
})
const cards = (count: number): string => (count === 1 ? '1 card' : `${count} cards`)

/** One answer for each of `things`, at most `MOST_ANSWERS` of them. */
function each<T>(
  things: readonly T[],
  make: (thing: T, at: number) => GenericChoice,
): { choices: GenericChoice[]; unlisted: number } {
  return {
    choices: things.slice(0, MOST_ANSWERS).map(make),
    unlisted: Math.max(0, things.length - MOST_ANSWERS),
  }
}

/** The target as a button names it: a player by name, a card by its name. */
function targetWords(target: Target, words: GenericWords): string {
  if ('Player' in target) {
    const name = words.seat(target.Player)
    return target.Player === words.viewer ? `${name} (you)` : name
  }
  return words.object(target.Object.id)
}

/** The numbers from `min` up that are allowed, at most `MOST_ANSWERS` of them. */
function allowedNumbers(min: number, max: number | null, except: readonly number[]): number[] {
  const numbers: number[] = []
  for (let n = min; numbers.length < MOST_ANSWERS && (max === null || n <= max); n += 1) {
    if (!except.includes(n)) numbers.push(n)
  }
  return numbers
}

const none = (): GenericOffer => ({ choices: [], unlisted: 0 })
const some = (...choices: GenericChoice[]): GenericOffer => ({ choices, unlisted: 0 })

/**
 * The answers on offer for `kind`. A kind that needs a set made by hand (an attack with creatures, a payment by hand)
 * offers what needs none: no attackers, no blockers, the engine's own payment, or the cast taken back; a decision
 * with no known kind offers nothing.
 */
export function genericChoices(kind: DecisionKind, words: GenericWords): GenericOffer {
  if ('Mulligan' in kind) {
    const { can_mulligan: can } = kind.Mulligan
    return some(
      pick('keep', 'Keep this hand', { Mulligan: 'Keep' }),
      ...(can ? [pick('mulligan', 'Take a mulligan', { Mulligan: 'Mulligan' })] : []),
    )
  }
  if ('BottomCards' in kind) {
    const { hand, count } = kind.BottomCards
    const first = hand.slice(0, count)
    return first.length === 0
      ? none()
      : some(pick('first', `Put the first ${cards(first.length)} on the bottom`, { BottomCards: first }))
  }
  if ('Discard' in kind) {
    const first = kind.Discard.hand.slice(0, kind.Discard.count)
    return first.length === 0
      ? none()
      : some(pick('first', `Discard the first ${cards(first.length)}`, { Discard: first }))
  }
  if ('Priority' in kind) {
    return kind.Priority.can_pass ? some(pick('pass', 'Pass priority', { Priority: 'Pass' })) : none()
  }
  if ('PayMana' in kind) {
    const { auto_available: auto, can_cancel: cancel } = kind.PayMana
    return some(
      ...(auto ? [pick('auto', 'Pay automatically', { PayMana: 'Auto' })] : []),
      ...(cancel ? [back(words, { PayMana: 'Cancel' })] : []),
    )
  }
  if ('DeclareAttackers' in kind) {
    return (kind.DeclareAttackers.required ?? []).length > 0
      ? none()
      : some(pick('none', 'Attack with nothing', { DeclareAttackers: [] }))
  }
  if ('DeclareBlockers' in kind) {
    return (kind.DeclareBlockers.required ?? []).length > 0
      ? none()
      : some(pick('none', 'Block nothing', { DeclareBlockers: [] }))
  }
  if ('AssignDamage' in kind) {
    const { blockers, power, lethal = [], excess } = kind.AssignDamage
    if (blockers.length === 0) return none()
    let left = power
    const shares = blockers.map((blocker, at): [ObjectId, number] => {
      const share = Math.min(left, lethal[at] ?? left)
      left -= share
      return [blocker, share]
    })
    const last = shares.at(-1)
    if (excess === undefined && last !== undefined) last[1] += left
    return some(pick('lethal', 'Assign lethal damage to each blocker in turn', { AssignDamage: shares }))
  }
  if ('ChooseTargets' in kind) {
    const { legal, min, max, can_cancel: cancel, triggered } = kind.ChooseTargets
    const single = min <= 1 && max >= 1
    const offer = each(single ? distinctTargets(legal) : [], (target) =>
      pick(targetKey(target), targetWords(target, words), { ChooseTargets: { Targets: [target] } }),
    )
    return {
      choices: [
        ...offer.choices,
        ...(min === 0 ? [pick('no-target', 'Choose no target', { ChooseTargets: { Targets: [] } })] : []),
        ...(cancel
          ? [back(words, { ChooseTargets: 'Cancel' })]
          : triggered === true
            ? [pick('remove', 'Remove the ability', { ChooseTargets: 'Remove' })]
            : []),
      ],
      unlisted: offer.unlisted,
    }
  }
  if ('ChooseMode' in kind) {
    const { modes, min, max, can_cancel: cancel } = kind.ChooseMode
    const single = min <= 1 && max >= 1
    const offer = each(single ? modes : [], (mode) =>
      pick(`mode-${mode.index}`, modeLabel(mode), { ChooseMode: { Modes: [mode.index] } }),
    )
    const first = modes.slice(0, min).map((mode) => mode.index)
    return {
      choices: [
        ...offer.choices,
        ...(!single && first.length > 0
          ? [pick('first-modes', `Choose the first ${first.length} modes`, { ChooseMode: { Modes: first } })]
          : []),
        ...(cancel ? [back(words, { ChooseMode: 'Cancel' })] : []),
      ],
      unlisted: offer.unlisted,
    }
  }
  if ('ChooseNumber' in kind) {
    const { min, max, except = [], can_cancel: cancel } = kind.ChooseNumber
    const offer = each(allowedNumbers(min, max, except), (n) =>
      pick(`n-${n}`, String(n), { ChooseNumber: { Number: n } }),
    )
    return {
      choices: [...offer.choices, ...(cancel === true ? [back(words, { ChooseNumber: 'Cancel' })] : [])],
      unlisted: 0,
    }
  }
  if ('DivideAmount' in kind) {
    const { total, among, can_cancel: cancel } = kind.DivideAmount
    const even = total >= among.length && among.length > 0
    return some(
      ...(even
        ? [pick('even', 'Divide evenly', divisionAnswer(among, evenShares(total, among.length)))]
        : []),
      ...(cancel === true ? [back(words, { DivideAmount: 'Cancel' })] : []),
    )
  }
  if ('ChooseCards' in kind) {
    const { from, min, max, can_cancel: cancel } = kind.ChooseCards
    const single = min <= 1 && max >= 1
    const offer = each(single ? from : [], (id) =>
      pick(`card-${id}`, words.object(id), { ChooseCards: [id] }),
    )
    const first = from.slice(0, min)
    return {
      choices: [
        ...offer.choices,
        ...(min === 0 ? [pick('no-cards', 'Choose no card', { ChooseCards: [] })] : []),
        ...(!single && first.length > 0
          ? [pick('first-cards', `Choose the first ${cards(first.length)}`, { ChooseCards: first })]
          : []),
        ...(cancel === true && max > min ? [back(words, { ChooseCards: [] })] : []),
      ],
      unlisted: offer.unlisted,
    }
  }
  if ('OrderCards' in kind) {
    return some(pick('keep', 'Keep the order', { OrderCards: kind.OrderCards.cards }))
  }
  if ('YesNo' in kind) {
    return some(
      ...(kind.YesNo.only_no === true ? [] : [pick('yes', 'Yes', { YesNo: true })]),
      pick('no', 'No', { YesNo: false }),
    )
  }
  if ('ChooseOption' in kind) {
    return each(kind.ChooseOption.options, (option, at) =>
      pick(`option-${at}`, optionLabel(option, words.seat, words.viewer), { ChooseOption: at }),
    )
  }
  if ('ChooseCosts' in kind) {
    const { optional, can_cancel: cancel } = kind.ChooseCosts
    const offer = each(optional, (cost) =>
      pick(`cost-${cost.index}`, cost.label, { ChooseCosts: { Costs: [cost.index] } }),
    )
    return {
      choices: [
        pick('no-cost', 'Pay no optional cost', { ChooseCosts: { Costs: [] } }),
        ...offer.choices,
        ...(cancel === true ? [back(words, { ChooseCosts: 'Cancel' })] : []),
      ],
      unlisted: offer.unlisted,
    }
  }
  if ('OrderTriggers' in kind) {
    const order = kind.OrderTriggers.triggers.map((_, at) => at)
    return some(pick('order', 'In the order they triggered', { OrderTriggers: order }))
  }
  if ('ChooseReplacement' in kind) {
    return each(kind.ChooseReplacement.candidates, (candidate, at) =>
      pick(`effect-${at}`, `${words.object(candidate.source)} first`, { ChooseReplacement: at }),
    )
  }
  return none()
}

/** The objects `kind` names that the buttons call by name, so the prompt can look them up in the view. */
export function namedObjects(kind: DecisionKind): ObjectId[] {
  if ('ChooseTargets' in kind) {
    return kind.ChooseTargets.legal.flatMap((target) => ('Object' in target ? [target.Object.id] : []))
  }
  if ('ChooseCards' in kind) return kind.ChooseCards.from
  if ('ChooseReplacement' in kind)
    return kind.ChooseReplacement.candidates.map((candidate) => candidate.source)
  return []
}

/** The object that asks, when the kind names one: what decides whether a cancel is of a spell or of an ability. */
export function askerOf(kind: DecisionKind): ObjectId | undefined {
  for (const options of Object.values(kind)) {
    if (typeof options === 'object' && options !== null && 'source' in options) {
      const { source } = options as { source: unknown }
      return typeof source === 'number' ? source : undefined
    }
  }
  return undefined
}
