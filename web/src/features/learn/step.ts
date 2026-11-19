import type { Step, StopSide } from '@/protocol/generated'
import type { DecisionName } from '@/features/table/turnText'
import { ANCHORS, cardRef, isAnchor } from './anchors'
import type { AnchorId, Target } from './anchors'
import type { Facts } from './facts'

/** What a step is: it explains (left with Next), waits for the learner's presses, or watches the bot act. */
export type StepKind = 'read' | 'do' | 'watch'

/** The one input a `do` step lets through. */
export type Allow =
  | { kind: 'keep' }
  | { kind: 'land'; name: string }
  | { kind: 'cast'; name: string }
  | { kind: 'pass' }
  | { kind: 'pay'; how: 'tap' | 'auto' }
  | { kind: 'endTurn' }
  | { kind: 'attack'; names: readonly string[] }
  | { kind: 'block'; blocker: string; attacker: string }
  | { kind: 'target'; name: string }
  | { kind: 'choose'; yes: boolean }
  | { kind: 'stop'; side: StopSide; step: Step }
  | { kind: 'look' }
  | { kind: 'manual' }

/** The decision each kind of input answers; `stop`, `endTurn` and `look` answer none. */
export const ALLOW_DECISION: Readonly<Record<Allow['kind'], DecisionName | null>> = {
  keep: 'Mulligan',
  land: 'Priority',
  cast: 'Priority',
  pass: 'Priority',
  pay: 'PayMana',
  endTurn: null,
  attack: 'DeclareAttackers',
  block: 'DeclareBlockers',
  target: 'ChooseTargets',
  choose: 'YesNo',
  stop: null,
  look: null,
  manual: null,
}

/** One step of a lesson. */
export interface LessonStep {
  id: string
  kind: StepKind
  /** At most 28 characters. */
  title: string
  /** At most 160 characters; `{@anchor}` is that element's words, `{look}` the verb of a look, `[[term]]` a lexicon link. */
  text: string
  /** A read or watch step's anchors, at most two. */
  lit?: readonly Target[]
  /** A do step's presses in order; the coach lights one at a time. */
  presses?: readonly Target[]
  /** The input a do step lets through. */
  allow?: Allow
  /** Whether a do or watch step is done. */
  done?: (facts: Facts) => boolean
  /** What the coach says when another input is tried. */
  hint?: string
}

/** A lesson's id, shared with the server's `tutorial::LESSONS`. */
export type LessonId = 'seat' | 'mana' | 'combat' | 'stack' | 'turn' | 'commander' | 'help'

/** A lesson: its id, name, what it teaches and its steps. */
export interface Lesson {
  id: LessonId | 'manual'
  title: string
  /** One line for the Learn page. */
  text: string
  minutes: number
  steps: readonly LessonStep[]
}

/** The limits of a step's words. */
export const TITLE_MAX = 28
export const TEXT_MAX = 160

/**
 * The step the lesson is at, moving forward from `index`: past every do or watch step whose `done` holds. A read step
 * is left only with Next, so it stops the walk; past the last step is the lesson's length.
 */
export function advance(steps: readonly LessonStep[], index: number, facts: Facts): number {
  let at = index
  while (at < steps.length) {
    const step = steps[at]
    if (step === undefined || step.kind === 'read' || step.done === undefined || !step.done(facts)) break
    at += 1
  }
  return at
}

/** What the coach shows: the step, and whether the game is moving on without the learner meanwhile. */
export interface Shown {
  index: number
  /** The game is not waiting on the learner, so the step at `index` is not started yet. */
  moving: boolean
}

/**
 * What the coach shows at cursor `index`: a read or do step only while a decision of the learner waits or the game is
 * over; until then the watch step before it, or the step before with "The game moves on" under it.
 */
export function shown(steps: readonly LessonStep[], index: number, facts: Facts): Shown {
  const step = steps[index]
  const waiting = facts.decision !== null || facts.over
  if (step === undefined) return { index: steps.length - 1, moving: false }
  if (step.kind === 'watch' || waiting) return { index, moving: false }
  const before = steps[index - 1]
  if (before?.kind === 'watch') return { index: index - 1, moving: false }
  return { index: Math.max(0, index - 1), moving: true }
}

/** The anchors a step lights now: a read or watch step's, or a do step's press `press`. */
export function litOf(step: LessonStep, press: number): readonly Target[] {
  if (step.kind !== 'do') return step.lit ?? []
  const target = step.presses?.[press]
  return target === undefined ? [] : [target]
}

/** A piece of a step's text: plain words, the words of an element (bold), or a lexicon term. */
export type TextPart =
  | { kind: 'text'; text: string }
  | { kind: 'name'; text: string; anchor: AnchorId }
  | { kind: 'term'; text: string }

const TOKENS = /\{@([^}]+)\}|\{look\}|\[\[([^\]]+)\]\]/g

/**
 * `text` in parts: `{@anchor}` becomes `words(anchor)`, `{look}` "Hover over" (or "Press and hold" on touch, `coarse`),
 * `[[term]]` a lexicon term. A sentence that starts with `{look}` keeps its capital.
 */
export function textParts(text: string, words: (anchor: AnchorId) => string, coarse: boolean): TextPart[] {
  const parts: TextPart[] = []
  let last = 0
  for (const match of text.matchAll(TOKENS)) {
    const at = match.index
    if (at > last) parts.push({ kind: 'text', text: text.slice(last, at) })
    const [whole, anchor, term] = match
    if (anchor !== undefined && isAnchor(anchor)) parts.push({ kind: 'name', text: words(anchor), anchor })
    else if (term !== undefined) parts.push({ kind: 'term', text: term })
    else if (whole === '{look}') {
      const verb = coarse ? 'press and hold' : 'hover over'
      const start = text.slice(0, at).trim() === '' || /[.:!?]\s*$/.test(text.slice(0, at))
      parts.push({ kind: 'text', text: start ? verb.charAt(0).toUpperCase() + verb.slice(1) : verb })
    } else parts.push({ kind: 'text', text: whole })
    last = at + whole.length
  }
  if (last < text.length) parts.push({ kind: 'text', text: text.slice(last) })
  return parts
}

/** The plain words of `parts`. */
export const plainText = (parts: readonly TextPart[]): string => parts.map((part) => part.text).join('')

/** Every anchor and card ref a step names, and every `{@…}` of its text. */
export function namedTargets(step: LessonStep): string[] {
  const tokens = [...step.text.matchAll(/\{@([^}]+)\}/g)].map((match) => match[1] ?? '')
  return [...(step.lit ?? []), ...(step.presses ?? []), ...tokens]
}

/** Whether `target` is an anchor of the registry or a card ref. */
export const knownTarget = (target: string): boolean => isAnchor(target) || cardRef(target) !== null

/** The registry's words for an anchor, used where its element has none. */
export const anchorLabel = (anchor: AnchorId): string => ANCHORS[anchor].label
