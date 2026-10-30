// Goal: a card draws the counters on a permanent as badges (a number for loyalty), cues what an Aura or Equipment is attached to and what a host has attached, says all of it in words to a screen reader, keeps permanents that differ in them out of one stack, and the wire check accepts the new fields and refuses malformed ones.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { CounterKind, ObjectView } from '@/protocol/generated'
import { indexById } from '@/play/view'
import { isStateView } from '@/net/parse'
import { card } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { fixtureOf } from '@/test-utils/combatTable'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from './Card'
import { counterLabels, counterName, counterWords, linkWords, loyaltyOf } from './cardModel'
import { groupPermanents } from './group'

const ID = 900

/** Card 900 on its own beside `others` in the store. */
function renderWith(object: Partial<ObjectView>, others: ObjectView[] = []) {
  return renderTable({
    ui: <Card id={ID} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, card({ ...object, id: ID }), ...others]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

const face = (container: HTMLElement): HTMLElement =>
  must(container.querySelector<HTMLElement>('[data-slot="card-face"]'))

describe('counters', () => {
  it('shows a badge a kind, with the number when there are several', () => {
    const { container } = renderWith({
      counters: [
        ['+1/+1', 2],
        ['charge', 1],
        [{ Other: 'time' }, 3],
      ],
    })
    const badges = [...container.querySelectorAll('[data-slot="card-counter"]')].map((b) => b.textContent)
    expect(badges).toEqual(['+1/+1 ×2', 'charge', 'time ×3'])
    expect(screen.getByRole('button').getAttribute('aria-label')).toMatch(
      /2 \+1\/\+1 counters, 1 charge counter, 3 time counters/,
    )
  })

  it('shows loyalty as a number in place of power and toughness, not as a badge', () => {
    const { container } = renderWith({
      types: ['Planeswalker'],
      subtypes: [],
      power: null,
      toughness: null,
      counters: [['loyalty', 4]],
    })
    const loyalty = must(container.querySelector('[data-slot="card-loyalty"]'))
    expect(loyalty).toHaveTextContent('4')
    expect(container.querySelector('[data-slot="card-counter"]')).toBeNull()
    expect(screen.getByRole('button').getAttribute('aria-label')).toMatch(/loyalty 4/)
  })

  it('draws nothing for a card without counters', () => {
    const { container } = renderWith({})
    expect(container.querySelector('[data-slot="card-counter"]')).toBeNull()
    expect(within(face(container)).getByText('2/2')).toBeInTheDocument()
  })
})

describe('attachments', () => {
  it('cues the host on an Aura or Equipment and names it to a screen reader', () => {
    const { container } = renderWith(
      { name: 'Pacifism', types: ['Enchantment'], power: null, toughness: null, attached_to: 901 },
      [card({ id: 901, name: 'Hill Giant' })],
    )
    expect(must(container.querySelector('[data-slot="card-attached-to"]'))).toHaveTextContent('On Hill Giant')
    expect(screen.getByRole('button').getAttribute('aria-label')).toMatch(/attached to Hill Giant/)
  })

  it('cues what is attached on the host', () => {
    const { container } = renderWith({ attachments: [901, 902] }, [
      card({ id: 901, name: 'Pacifism', types: ['Enchantment'] }),
      card({ id: 902, name: 'Bonesplitter', types: ['Artifact'] }),
    ])
    const cues = [...container.querySelectorAll('[data-slot="card-grant"]')].map((c) => c.textContent)
    expect(cues).toEqual(['Pacifism', 'Bonesplitt.'])
    expect(screen.getByRole('button').getAttribute('aria-label')).toMatch(
      /Pacifism attached, Bonesplitter attached/,
    )
  })
})

describe('the words', () => {
  it.each<[CounterKind, string]>([
    ['+1/+1', '+1/+1'],
    ['-1/-1', '-1/-1'],
    ['loyalty', 'loyalty'],
    [{ Other: 'time' }, 'time'],
  ])('names the kind %j "%s"', (kind, name) => {
    expect(counterName(kind)).toBe(name)
  })

  it('reads counters and links in order, and loyalty apart from the badges', () => {
    const walker = card({
      counters: [
        ['loyalty', 3],
        ['+1/+1', 1],
      ],
    })
    expect(counterLabels(walker)).toEqual(['+1/+1'])
    expect(counterWords(walker)).toEqual(['loyalty 3', '1 +1/+1 counter'])
    expect(loyaltyOf(walker)).toBe(3)
    expect(loyaltyOf(card())).toBeNull()
    expect(linkWords({})).toEqual([])
    expect(linkWords({ host: 'Bear', attached: ['A', 'B'] })).toEqual([
      'attached to Bear',
      'A and B attached',
    ])
  })
})

describe('stacks', () => {
  const token = (id: number, over: Partial<ObjectView> = {}) =>
    card({ id, name: 'Soldier', kind: 'Token', types: ['Creature'], ...over })
  const stacksOf = (...members: ObjectView[]) =>
    groupPermanents(members).map((stack) => stack.members.map((m) => m.id))

  it('merges equal tokens and keeps a token with a counter, an attachment or a host apart', () => {
    expect(stacksOf(token(1), token(2))).toEqual([[1, 2]])
    expect(stacksOf(token(1), token(2, { counters: [['+1/+1', 1]] }))).toEqual([[1], [2]])
    expect(stacksOf(token(1, { counters: [['+1/+1', 1]] }), token(2, { counters: [['+1/+1', 1]] }))).toEqual([
      [1, 2],
    ])
    expect(stacksOf(token(1), token(2, { attachments: [7] }))).toEqual([[1], [2]])
    expect(stacksOf(token(1, { attached_to: 7 }), token(2, { attached_to: 8 }))).toEqual([[1], [2]])
    expect(stacksOf(token(1, { attached_to: 7 }), token(2, { attached_to: 7 }))).toEqual([[1, 2]])
  })
})

describe('the wire check', () => {
  const withObject = (over: Record<string, unknown>) => {
    const view = clone(fixtureOf(2).view)
    return { ...view, objects: [{ ...must(view.objects[0]), ...over }, ...view.objects.slice(1)] }
  }

  it.each([
    [{}, true],
    [
      {
        counters: [
          ['+1/+1', 2],
          [{ Other: 'time' }, 1],
        ],
        attached_to: 3,
        attachments: [4, 5],
      },
      true,
    ],
    [{ counters: [['+1/+1', -1]] }, false],
    [{ counters: [['+1/+1']] }, false],
    [{ counters: '+1/+1' }, false],
    [{ attached_to: 'Bear' }, false],
    [{ attachments: [1, 'x'] }, false],
  ])('%j is %s', (over, ok) => {
    expect(isStateView(withObject(over))).toBe(ok)
  })
})
