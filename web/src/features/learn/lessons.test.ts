// Goal: every lesson of the course fits the game the server really plays (`traces/<id>.json`, written by the server's tutorial test): each decision the learner is sent meets a do step whose input is the scripted one and that holds back every other card, pass or answer of that decision, each read step is entered while the game waits on the learner, every step is reached and the end leaves the last one, and each step keeps its word limits and names only registered anchors.
import { describe, expect, it } from 'vitest'
import type { Answer, ObjectView } from '@/protocol/generated'
import { COURSE } from './course'
import type { Facts, LessonFacts, Named } from './facts'
import { allowed } from './gate'
import { ALLOW_DECISION, TEXT_MAX, TITLE_MAX, advance, knownTarget, namedTargets } from './step'
import type { Allow, Lesson, LessonStep } from './step'
import type { AnchorId } from './anchors'

/** One line of a trace: the scripted act at a decision, and the facts then. */
interface TraceEntry {
  act: string | null
  cards: string[]
  yes?: boolean
  stop?: { side: string; step: string }
  facts: LessonFacts
}

const TRACES = import.meta.glob<TraceEntry[]>('./traces/*.json', { eager: true, import: 'default' })

const traceOf = (lesson: Lesson): TraceEntry[] => TRACES[`./traces/${lesson.id}.json`] ?? []

/** Whether the scripted `entry` is the input `allow` lets through. */
function accepts(allow: Allow, entry: TraceEntry): boolean {
  const [first, second] = entry.cards
  switch (entry.act) {
    case 'Keep':
      return allow.kind === 'keep'
    case 'Land':
      return allow.kind === 'land' && allow.name === first
    case 'Cast':
      return allow.kind === 'cast' && allow.name === first
    case 'Pass':
      return allow.kind === 'pass'
    case 'PayAuto':
      return allow.kind === 'pay' && allow.how === 'auto'
    case 'PayTap':
      return allow.kind === 'pay' && allow.how === 'tap'
    case 'Attack':
      return (
        allow.kind === 'attack' && [...allow.names].toSorted().join('|') === entry.cards.toSorted().join('|')
      )
    case 'Block':
      return allow.kind === 'block' && allow.blocker === first && allow.attacker === second
    case 'Target':
      return allow.kind === 'target' && allow.name === first
    case 'Choose':
      return allow.kind === 'choose' && allow.yes === entry.yes
    case 'SetStop':
      return allow.kind === 'stop' && allow.side === entry.stop?.side && allow.step === entry.stop.step
    case 'EndTurn':
      return allow.kind === 'endTurn'
    default:
      return false
  }
}

/** The objects of `facts` by id, as the gate reads names. */
function objectsOf(facts: LessonFacts): Record<number, ObjectView> {
  const all: Named[] = [
    ...facts.hand,
    ...facts.lands,
    ...facts.creatures,
    ...facts.theirCreatures,
    ...facts.command,
    ...facts.stack,
  ]
  return Object.fromEntries(all.map((card) => [card.id, { id: card.id, name: card.name } as ObjectView]))
}

/** The other answers of a decision the step must hold back: every other card to play, and a pass. */
function otherAnswers(facts: LessonFacts, allow: Allow): Answer[] {
  if (facts.decision !== 'Priority') return []
  const cards = [...facts.hand, ...facts.command]
  const plays: Answer[] = cards.flatMap((card) => [
    { Priority: { PlayLand: card.id } },
    { Priority: { CastSpell: card.id } },
  ])
  const pass: Answer[] = allow.kind === 'pass' ? [] : [{ Priority: 'Pass' }]
  return [...plays, ...pass].filter((answer) => {
    if (typeof answer !== 'object' || !('Priority' in answer) || typeof answer.Priority !== 'object')
      return true
    const id =
      'PlayLand' in answer.Priority
        ? answer.Priority.PlayLand
        : 'CastSpell' in answer.Priority
          ? answer.Priority.CastSpell
          : -1
    const name = cards.find((card) => card.id === id)?.name
    const kind = 'PlayLand' in answer.Priority ? 'land' : 'cast'
    return !(allow.kind === kind && allow.name === name)
  })
}

/** The page change that finishes a `look` step: a panel opened or closed. */
function looked(step: LessonStep, facts: Facts): Facts | null {
  const changes: ((shown: Set<AnchorId>) => void)[] = [
    (shown) => shown.add('preview'),
    (shown) => shown.add('lexicon'),
    (shown) => shown.delete('lexicon'),
  ]
  for (const change of changes) {
    const shown = new Set(facts.page.shown)
    change(shown)
    const next = { ...facts, page: { shown } }
    if (step.done?.(next) === true) return next
  }
  return null
}

/** Walks `lesson` over its trace; returns the steps reached in order. */
function walk(lesson: Lesson): string[] {
  const steps = lesson.steps
  const reached: string[] = []
  let index = 0
  let page = { shown: new Set<AnchorId>() as ReadonlySet<AnchorId> }
  const trace = traceOf(lesson)
  for (const [line, entry] of trace.entries()) {
    let facts: Facts = { ...entry.facts, page }
    for (;;) {
      const from = index
      index = advance(steps, index, facts)
      // A watch step passed between two decisions was on show while the bot played.
      for (const each of steps.slice(from, index + 1)) if (reached.at(-1) !== each.id) reached.push(each.id)
      const step = steps[index]
      if (step === undefined) break
      const where = `${lesson.id} step ${step.id} at trace line ${line} (${entry.act ?? 'end'})`
      if (step.kind === 'read') {
        expect(facts.decision !== null || facts.over, `${where}: a read step while the game moves`).toBe(true)
        index += 1
        continue
      }
      if (step.kind === 'watch') {
        expect(facts.decision, `${where}: a decision arrives while the coach watches`).toBeNull()
        break
      }
      const allow = step.allow
      expect(allow, `${where}: a do step without an input`).toBeDefined()
      if (allow === undefined) break
      if (allow.kind === 'look') {
        const next = looked(step, facts)
        expect(next, `${where}: no page change finishes the look`).not.toBeNull()
        if (next === null) break
        facts = next
        page = next.page
        continue
      }
      if (entry.act === null) break
      expect(facts.decision, `${where}: the step waits on another decision`).toBe(
        ALLOW_DECISION[allow.kind] ?? facts.decision,
      )
      expect(accepts(allow, entry), `${where}: the step does not let the scripted act through`).toBe(true)
      const objects = objectsOf(facts)
      for (const answer of otherAnswers(facts, allow))
        expect(allowed(allow, answer, objects), `${where}: ${JSON.stringify(answer)} passes`).toBe(false)
      break
    }
  }
  expect(index, `${lesson.id} ends before its last step`).toBe(steps.length)
  return reached
}

describe.each(COURSE.map((lesson) => [lesson.id, lesson] as const))('lesson %s', (_, lesson) => {
  it('has a trace', () => {
    expect(traceOf(lesson).length).toBeGreaterThan(0)
  })

  it('meets every decision of its trace with the step that asks for it and reaches every step', () => {
    expect(walk(lesson)).toEqual(lesson.steps.map((step) => step.id))
  })

  it('keeps at most twelve steps, the word limits, and names only registered anchors', () => {
    expect(lesson.steps.length).toBeLessThanOrEqual(12)
    for (const step of lesson.steps) {
      expect(step.title.length, step.id).toBeLessThanOrEqual(TITLE_MAX)
      expect(step.text.length, step.id).toBeLessThanOrEqual(TEXT_MAX)
      expect(
        namedTargets(step).filter((target) => !knownTarget(target)),
        step.id,
      ).toEqual([])
      if (step.kind === 'do') expect(step.presses?.length ?? 0, step.id).toBeGreaterThan(0)
      else expect(step.lit?.length ?? 0, step.id).toBeLessThanOrEqual(2)
    }
  })
})
