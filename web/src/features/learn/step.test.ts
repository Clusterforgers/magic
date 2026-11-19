// Goal: the lesson cursor moves only forward, past do and watch steps the game shows done and never past a read step; a read or do step shows only while the game waits on the learner, else the watch step before it or "the game moves on"; step texts read the words of the anchors they name, the look verb for mouse and touch, and lexicon terms; and resolving a target finds exactly one drawn element, its opener while it is hidden, or reports none or several.
import { afterEach, describe, expect, it } from 'vitest'
import { NO_PAGE } from './facts'
import type { Facts, LessonFacts } from './facts'
import { resolve } from './resolve'
import { advance, plainText, shown, textParts } from './step'
import type { LessonStep } from './step'

const FACTS: LessonFacts = {
  turn: 1,
  mine: true,
  step: 'Main1',
  decision: 'Priority',
  hand: [],
  lands: [],
  creatures: [],
  theirCreatures: [],
  command: [],
  graveyard: [],
  theirGraveyard: [],
  stack: [],
  attacking: 0,
  blocking: 0,
  attacked: false,
  life: 40,
  theirLife: 40,
  stops: [],
  over: false,
  won: false,
}
const facts = (change: Partial<LessonFacts>): Facts => ({ ...FACTS, ...change, page: NO_PAGE })

const STEPS: LessonStep[] = [
  { id: 'a', kind: 'read', title: 'A', text: '' },
  { id: 'b', kind: 'do', title: 'B', text: '', presses: ['pass.primary'], done: (f) => f.turn >= 2 },
  { id: 'c', kind: 'watch', title: 'C', text: '', done: (f) => f.turn >= 3 && f.decision !== null },
  { id: 'd', kind: 'read', title: 'D', text: '' },
  { id: 'e', kind: 'do', title: 'E', text: '', presses: ['pass.primary'], done: (f) => f.turn >= 5 },
]

describe('advance', () => {
  it.each([
    ['stays at a read step', 0, { turn: 9 }, 0],
    ['stays at a do step not done', 1, { turn: 1 }, 1],
    ['passes a done do step and stops at the watch', 1, { turn: 2, decision: null }, 2],
    ['passes several steps done at once, up to a read step', 1, { turn: 3 }, 3],
    ['runs past the last step', 4, { turn: 5 }, 5],
  ] as const)('%s', (_, from, change, want) => {
    expect(advance(STEPS, from, facts(change))).toBe(want)
  })
})

describe('shown', () => {
  it.each([
    ['a step while the learner decides', 1, { decision: 'Priority' }, { index: 1, moving: false }],
    ['the watch step while the bot plays', 2, { decision: null }, { index: 2, moving: false }],
    ['the watch step before a read step not started', 3, { decision: null }, { index: 2, moving: false }],
    ['the step before with the game moving on', 4, { decision: null }, { index: 3, moving: true }],
    ['a read step once the game is over', 3, { decision: null, over: true }, { index: 3, moving: false }],
  ] as const)('shows %s', (_, index, change, want) => {
    expect(shown(STEPS, index, facts(change))).toEqual(want)
  })
})

describe('textParts', () => {
  const words = (anchor: string): string => (anchor === 'pass.primary' ? 'Next: Combat' : anchor)

  it('names an anchor by its words, in bold', () => {
    const parts = textParts('Press {@pass.primary} now.', words, false)
    expect(parts).toEqual([
      { kind: 'text', text: 'Press ' },
      { kind: 'name', text: 'Next: Combat', anchor: 'pass.primary' },
      { kind: 'text', text: ' now.' },
    ])
  })

  it.each([
    [false, 'Hover over Serra Angel.'],
    [true, 'Press and hold Serra Angel.'],
  ])('says the look for a coarse pointer %s', (coarse, want) => {
    expect(plainText(textParts('{look} Serra Angel.', words, coarse))).toBe(want)
  })

  it('links a lexicon term', () => {
    expect(textParts('It waits on the [[stack]].', words, false)[1]).toEqual({ kind: 'term', text: 'stack' })
  })
})

describe('resolve', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  const place = (element: HTMLElement, width = 40, height = 20): void => {
    element.getBoundingClientRect = () =>
      ({
        left: 10,
        top: 10,
        width,
        height,
        right: 10 + width,
        bottom: 10 + height,
        x: 10,
        y: 10,
        toJSON: () => null,
      }) as DOMRect
  }
  const lookup = {
    objects: { 7: { id: 7, name: 'Forest' } as never, 8: { id: 8, name: 'Forest' } as never },
    usable: new Set([8]),
  }

  it('finds the one drawn element of an anchor', () => {
    document.body.innerHTML = '<button data-coach="pass.primary">Pass</button>'
    place(document.querySelector('button') as HTMLElement)
    expect(resolve('pass.primary', lookup)).toMatchObject({ kind: 'found', via: null })
  })

  it('reports several elements of one anchor', () => {
    document.body.innerHTML =
      '<button data-coach="pass.primary"></button><button data-coach="pass.primary"></button>'
    for (const button of document.querySelectorAll('button')) place(button)
    expect(resolve('pass.primary', lookup)).toEqual({ kind: 'many', count: 2 })
  })

  it('lights the opener of a hidden anchor', () => {
    document.body.innerHTML = '<button data-coach="pass.more"></button>'
    place(document.querySelector('button') as HTMLElement)
    expect(resolve('pass.end-turn', lookup)).toMatchObject({ kind: 'found', via: 'pass.more' })
  })

  it('reports an anchor without an element or an opener', () => {
    expect(resolve('opening.keep', lookup)).toEqual({ kind: 'none' })
  })

  it('finds the card the decision can use among cards of one name in a zone', () => {
    document.body.innerHTML =
      '<section data-coach-zone="hand"><div data-object-id="7"></div><div data-object-id="8"><span data-object-id="8"></span></div></section>'
    for (const card of document.querySelectorAll<HTMLElement>('[data-object-id]')) place(card)
    const found = resolve('hand:Forest', lookup)
    expect(found.kind === 'found' && found.element.dataset['objectId']).toBe('8')
  })
})
