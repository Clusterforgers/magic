/** Goal: for every number of mulligans taken and every opening-hand progress of the other seats, the Mulligan prompt says in plain words what the hand is, how many mulligans were taken and how many cards the next one puts on the bottom, names the seats still deciding, and answers Keep or Mulligan by click or key (Mulligan only while allowed) without hiding or marking the hand strip. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, MulliganOptions, MulliganStatus, ObjectId } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'
import { decidingText, mulliganText } from './mulliganText'

const hand = must(buildFixture({ seats: 2 }).view.players[0]?.hand.cards)
const options = (over: Partial<MulliganOptions> = {}): MulliganOptions => ({
  hand,
  mulligans_taken: 0,
  can_mulligan: true,
  ...over,
})

interface Others {
  /** The mulligan progress of each other seat, from seat 1. */
  status?: MulliganStatus[]
  /** Seats that have lost already. */
  lost?: number[]
  /** The viewer's own progress. */
  own?: MulliganStatus
}

/** The table of seat 0 (of `seats`) facing the Mulligan decision, with an `answer` spy that accepts everything. */
function renderMulligan(given: MulliganOptions, { status = [], lost = [], own = 'Deciding' }: Others = {}) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    seats: Math.max(2, status.length + 1),
    session: { answer },
    state: (state) => {
      const view = clone(must(state.view))
      view.stage = 'Mulligan'
      must(view.players[0]).mulligan = own
      for (const [index, progress] of status.entries()) must(view.players[index + 1]).mulligan = progress
      for (const seat of lost) must(view.players[seat]).lost = 'Conceded'
      return { ...state, view, pending: { id: 3, decision: { player: 0, kind: { Mulligan: given } } } }
    },
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const text = (): string =>
    must(table.container.querySelector('[data-slot="prompt-text"]')).textContent ?? ''
  const waiting = (): string | null =>
    table.container.querySelector('[data-slot="mulligan-waiting"]')?.textContent ?? null
  const keep = (): HTMLElement => screen.getByRole('button', { name: 'Keep' })
  const mulligan = (): HTMLElement => screen.getByRole('button', { name: 'Mulligan' })
  return { ...table, answer, card, text, waiting, keep, mulligan }
}

describe('the words', () => {
  const TAKEN = [0, 1, 2, 3, 4, 5, 6]

  it.each(TAKEN)('after %i mulligans it says how many were taken and what the next one costs', (taken) => {
    const { text } = renderMulligan(options({ mulligans_taken: taken }))
    const next = taken + 1
    expect(text()).toBe(
      [
        'Keep this hand of 7 cards, or take a mulligan?',
        taken === 0
          ? 'You have not taken a mulligan yet.'
          : `You have taken ${taken} ${taken === 1 ? 'mulligan' : 'mulligans'} so far.`,
        `${taken === 0 ? 'A' : 'Another'} mulligan draws 7 new cards, then puts ${next} ${next === 1 ? 'card' : 'cards'} on the bottom of your library.`,
      ].join(' '),
    )
  })

  it('says a mulligan is no longer possible when it is not', () => {
    const { text } = renderMulligan(options({ mulligans_taken: 7, can_mulligan: false }))
    expect(text()).toBe(
      'Keep this hand of 7 cards? You have taken 7 mulligans so far. You cannot take another.',
    )
  })

  it.each([
    [
      0,
      'Keep this hand, or take a mulligan? You have not taken a mulligan yet. A mulligan draws a new hand, then puts 1 card on the bottom of your library.',
    ],
    [
      1,
      'Keep this hand of 1 card, or take a mulligan? You have not taken a mulligan yet. A mulligan draws 1 new card, then puts 1 card on the bottom of your library.',
    ],
    [
      5,
      'Keep this hand of 5 cards, or take a mulligan? You have not taken a mulligan yet. A mulligan draws 5 new cards, then puts 1 card on the bottom of your library.',
    ],
  ])('writes a hand of %i cards', (size, words) => {
    expect(mulliganText(options({ hand: hand.slice(0, size) }))).toBe(words)
  })

  it('is one polite live sentence under the name of the decision', () => {
    const { container } = renderMulligan(options())
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveTextContent('Your decision: Mulligan')
    expect(line.querySelectorAll('[aria-live]')).toHaveLength(1)
    expect(must(line.querySelector('[aria-live]'))).toHaveAttribute('aria-atomic', 'true')
  })
})

describe('who is still deciding', () => {
  const STATUSES: [MulliganStatus, boolean][] = [
    ['NeedsHand', false],
    ['Deciding', true],
    ['Mulliganing', false],
    [{ Bottoming: 2 }, true],
    ['Kept', false],
  ]

  it.each(STATUSES)('names Bo when Bo is %j: %s', (status, named) => {
    const { waiting } = renderMulligan(options(), { status: [status] })
    expect(waiting()).toBe(named ? 'Still deciding: Bo.' : null)
  })

  it.each([
    ['two seats', ['Deciding', 'Kept', 'Deciding'], 'Still deciding: Bo and Dee.'],
    ['three seats', ['Deciding', { Bottoming: 1 }, 'Deciding'], 'Still deciding: Bo, Cy and Dee.'],
    ['nobody', ['Kept', 'Mulliganing', 'NeedsHand'], null],
  ] as [string, MulliganStatus[], string | null][])('with %s', (_who, status, words) => {
    expect(renderMulligan(options(), { status }).waiting()).toBe(words)
  })

  it('leaves out a seat that has lost', () => {
    const { waiting } = renderMulligan(options(), { status: ['Deciding', 'Deciding'], lost: [1] })
    expect(waiting()).toBe('Still deciding: Cy.')
  })

  it('never names the viewer', () => {
    expect(renderMulligan(options(), { own: 'Deciding', status: ['Kept'] }).waiting()).toBeNull()
  })

  it('follows the table as the other seats decide', () => {
    const { waiting, update } = renderMulligan(options(), { status: ['Deciding'] })
    expect(waiting()).toBe('Still deciding: Bo.')
    update((state) => {
      const view = clone(must(state.view))
      must(view.players[1]).mulligan = 'Kept'
      return { view }
    })
    expect(waiting()).toBeNull()
  })

  it.each([
    [['Bo'], 'Still deciding: Bo.'],
    [['Bo', 'Cy'], 'Still deciding: Bo and Cy.'],
    [['Bo', 'Cy', 'Dee'], 'Still deciding: Bo, Cy and Dee.'],
  ] as [string[], string][])('writes %j', (names, words) => {
    expect(decidingText(names)).toBe(words)
  })
})

describe('the hand', () => {
  it('stays in its strip with every card, unmarked and not pressable', () => {
    const { card, container } = renderMulligan(options())
    for (const id of hand) {
      expect(card(id)).toBeVisible()
      expect(card(id)).not.toHaveAttribute('data-highlight')
      expect(card(id)).not.toHaveAttribute('aria-pressed')
    }
    expect(container.querySelectorAll('[data-highlight]')).toHaveLength(0)
  })

  it('does nothing when a card is clicked, apart from previewing it', async () => {
    const { card, answer, user } = renderMulligan(options())
    await user.click(card(must(hand[0])))
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('Keep and Mulligan', () => {
  it.each([
    ['Keep', 'Keep'],
    ['Mulligan', 'Mulligan'],
  ] as const)('%s answers Mulligan: %s by click', async (label, choice) => {
    const { answer, user } = renderMulligan(options())
    await user.click(screen.getByRole('button', { name: label }))
    expect(answer.mock.calls).toEqual([[{ Mulligan: choice }]])
  })

  it.each([
    ['Keep', 'Enter', '{Enter}', 'Keep'],
    ['Keep', 'Space', ' ', 'Keep'],
    ['Mulligan', 'Enter', '{Enter}', 'Mulligan'],
    ['Mulligan', 'Space', ' ', 'Mulligan'],
  ] as const)('%s answers on %s from the keyboard alone', async (label, _key, keys, choice) => {
    const { answer, user, keep } = renderMulligan(options())
    act(() => keep().focus())
    if (label === 'Mulligan') await user.keyboard('{Tab}')
    expect(screen.getByRole('button', { name: label })).toHaveFocus()
    await user.keyboard(keys)
    expect(answer.mock.calls).toEqual([[{ Mulligan: choice }]])
  })

  it('answers once for a double click', async () => {
    const { answer, user, keep } = renderMulligan(options())
    await user.dblClick(keep())
    expect(answer.mock.calls).toEqual([[{ Mulligan: 'Keep' }]])
  })

  it('cannot mulligan when the engine says no, but can still keep', async () => {
    const { answer, user, keep, mulligan } = renderMulligan(
      options({ mulligans_taken: 7, can_mulligan: false }),
    )
    expect(mulligan()).toBeDisabled()
    await user.click(mulligan())
    expect(answer).not.toHaveBeenCalled()
    await user.click(keep())
    expect(answer.mock.calls).toEqual([[{ Mulligan: 'Keep' }]])
  })

  it('leaves both buttons enabled while a mulligan is allowed', () => {
    const { keep, mulligan } = renderMulligan(options())
    expect(keep()).toBeEnabled()
    expect(mulligan()).toBeEnabled()
  })
})

describe('a new decision', () => {
  it('shows the new hand and count when the engine asks again after a mulligan', () => {
    const { text, update, answer } = renderMulligan(options())
    expect(text()).toContain('You have not taken a mulligan yet.')
    update(() => ({
      pending: { id: 4, decision: { player: 0, kind: { Mulligan: options({ mulligans_taken: 1 }) } } },
    }))
    expect(text()).toContain('You have taken 1 mulligan so far.')
    expect(text()).toContain('puts 2 cards on the bottom')
    expect(answer).not.toHaveBeenCalled()
  })

  it('takes the prompt away when the decision is answered elsewhere', () => {
    const { container, update } = renderMulligan(options())
    update(() => ({ pending: null }))
    expect(container.querySelector('[data-decision]')).toBeNull()
  })
})
