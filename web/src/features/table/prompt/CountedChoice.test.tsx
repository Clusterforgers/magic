/** Goal: for BottomCards and Discard alike, over every count from 0 to 7 and several offered sets, exactly the offered cards of the hand are marked, a click or Enter/Space toggles a card and the sentence counts "k of N chosen", Confirm waits until exactly N are chosen and then answers with them in the order chosen, a card beyond N is refused with a spoken hint that drops nothing, and a new decision starts with nothing chosen. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, DecisionKind, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const view = fixture.view
const hand = must(view.players[0]?.hand.cards)
const battlefield = view.objects
  .filter((object) => object.zone === 'Battlefield' && object.controller === 0)
  .map((object) => object.id)
const OWN: ObjectId[] = [...hand, ...battlefield]
const COUNTS = [0, 1, 2, 3, 4, 5, 6, 7]

interface Kind {
  decide(offered: ObjectId[], count: number): DecisionKind
  answer(chosen: ObjectId[]): Answer
  /** What the sentence says is done with the cards. */
  words: string
}
const KINDS: [string, Kind][] = [
  [
    'BottomCards',
    {
      decide: (offered, count) => ({ BottomCards: { hand: offered, count } }),
      answer: (chosen) => ({ BottomCards: chosen }),
      words: 'put on the bottom of your library',
    },
  ],
  [
    'Discard',
    {
      decide: (offered, count) => ({ Discard: { hand: offered, count } }),
      answer: (chosen) => ({ Discard: chosen }),
      words: 'discard',
    },
  ],
]

/** The sets of cards a decision may offer. */
const OFFERS: [string, ObjectId[]][] = [
  ['the whole hand', hand],
  ['all but the last two', hand.slice(0, -2)],
]

/** The table of seat 0 facing a decision of `kind`, with an `answer` spy that accepts everything. */
function renderChoice(kind: Kind, offered: ObjectId[], count: number, id = 4) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state) => ({
      ...state,
      pending: { id, decision: { player: 0, kind: kind.decide(offered, count) } },
    }),
  })
  const card = (target: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${target}"]`))
  const marks = (): Map<number, string> =>
    new Map(
      [...table.container.querySelectorAll<HTMLElement>('[data-highlight]')].map((element) => [
        Number(element.getAttribute('data-object-id')),
        must(element.getAttribute('data-highlight')),
      ]),
    )
  const live = (): HTMLElement =>
    must(table.container.querySelector<HTMLElement>('[data-slot="prompt-text"]'))
  const text = (): string => live().textContent ?? ''
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm' })
  const pressed = (): ObjectId[] =>
    hand.filter((target) => card(target).getAttribute('aria-pressed') === 'true')
  const decide = (to: number, offeredTo = offered, countTo = count): void =>
    table.update(() => ({
      pending: { id: to, decision: { player: 0, kind: kind.decide(offeredTo, countTo) } },
    }))
  return { ...table, answer, card, marks, live, text, confirm, pressed, decide }
}

describe.each(KINDS)('%s', (_name, kind) => {
  describe('what is marked', () => {
    const rows = OFFERS.flatMap(([offer, offered]) =>
      COUNTS.map((count): [string, ObjectId[], number] => [offer, offered, count]),
    )
    it.each(rows)(
      'offering %s and asking for %i marks the offered cards, dims the rest of hand and board',
      (_offer, offered, count) => {
        const { marks } = renderChoice(kind, offered, count)
        const legal = new Set(count > 0 ? offered : [])
        expect(marks()).toEqual(new Map(OWN.map((id) => [id, legal.has(id) ? 'legal' : 'dimmed'])))
      },
    )

    it('reads each card as a toggle that is pressed or not', () => {
      const { card } = renderChoice(kind, hand, 2)
      expect(card(must(hand[0]))).toHaveAttribute('aria-pressed', 'false')
      expect(card(must(hand[0]))).toHaveAccessibleName(/, available$/)
    })

    it('marks nothing outside the viewer hand and board', () => {
      const { marks } = renderChoice(kind, hand, 2)
      for (const object of view.objects)
        if (!OWN.includes(object.id)) expect(marks().has(object.id)).toBe(false)
    })
  })

  describe('choosing', () => {
    it.each(COUNTS.slice(1))(
      'with %i to choose, Confirm waits for the last card and answers in the order chosen',
      async (count) => {
        const picks = [...hand].reverse().slice(0, count)
        const { card, confirm, text, pressed, answer, user } = renderChoice(kind, hand, count)
        expect(text()).toContain(`0 of ${count} chosen.`)
        expect(confirm()).toBeDisabled()
        for (const [index, pick] of picks.entries()) {
          expect(confirm()).toBeDisabled()
          await user.click(card(pick))
          expect(text()).toContain(`${index + 1} of ${count} chosen.`)
          expect(card(pick)).toHaveAttribute('data-highlight', 'selected')
        }
        expect(pressed().toSorted()).toEqual(picks.toSorted())
        expect(confirm()).toBeEnabled()
        await user.click(confirm())
        expect(answer.mock.calls).toEqual([[kind.answer(picks)]])
      },
    )

    it('says what is being done with the cards', () => {
      const { text } = renderChoice(kind, hand, 3)
      expect(text()).toBe(
        `${kind.words === 'discard' ? 'Your hand is over the limit. ' : ''}Choose 3 cards to ${kind.words}. 0 of 3 chosen.`,
      )
    })

    it('puts a chosen card back when it is chosen again', async () => {
      const [first, second] = hand as [ObjectId, ObjectId]
      const { card, text, pressed, confirm, user } = renderChoice(kind, hand, 2)
      await user.click(card(first))
      await user.click(card(second))
      expect(confirm()).toBeEnabled()
      await user.click(card(first))
      expect(pressed()).toEqual([second])
      expect(text()).toContain('1 of 2 chosen.')
      expect(confirm()).toBeDisabled()
      expect(card(first)).toHaveAttribute('data-highlight', 'legal')
    })

    it('does not answer when Confirm is pressed too early', async () => {
      const { card, confirm, answer, user } = renderChoice(kind, hand, 3)
      await user.click(card(must(hand[0])))
      await user.click(confirm())
      expect(answer).not.toHaveBeenCalled()
    })

    it('answers once for a double click on Confirm', async () => {
      const { card, confirm, answer, user } = renderChoice(kind, hand, 1)
      await user.click(card(must(hand[3])))
      await user.dblClick(confirm())
      expect(answer.mock.calls).toEqual([[kind.answer([must(hand[3])])]])
    })

    it('has one polite live sentence, which carries the count', async () => {
      const { card, live, container, user } = renderChoice(kind, hand, 2)
      expect(container.querySelectorAll('[data-slot="decision-line"] [aria-live]')).toHaveLength(1)
      expect(live()).toHaveAttribute('aria-live', 'polite')
      await user.click(card(must(hand[0])))
      expect(live()).toHaveTextContent('1 of 2 chosen. Choose 1 more card to')
    })
  })

  describe('a card beyond the count', () => {
    it.each(COUNTS.slice(1, -1))(
      'with %i chosen is refused with a hint and nothing is dropped',
      async (count) => {
        const picks = hand.slice(0, count)
        const extra = must(hand[count])
        const { card, text, pressed, confirm, answer, user } = renderChoice(kind, hand, count)
        for (const pick of picks) await user.click(card(pick))
        await user.click(card(extra))
        expect(pressed()).toEqual(picks)
        expect(card(extra)).toHaveAttribute('aria-pressed', 'false')
        expect(text()).toContain(`Only ${count} ${count === 1 ? 'card' : 'cards'} can be chosen.`)
        expect(text()).toContain(`${count} of ${count} chosen.`)
        expect(confirm()).toBeEnabled()
        expect(answer).not.toHaveBeenCalled()
      },
    )

    it('is refused again each time, with a fresh hint element so it is spoken again', async () => {
      const [first, second, third] = hand as [ObjectId, ObjectId, ObjectId]
      const { card, live, user } = renderChoice(kind, hand, 2)
      await user.click(card(first))
      await user.click(card(second))
      await user.click(card(third))
      const hint = must(live().querySelector('[data-slot="prompt-hint"]'))
      await user.click(card(third))
      const again = must(live().querySelector('[data-slot="prompt-hint"]'))
      expect(again).not.toBe(hint)
      expect(again.textContent).toBe(hint.textContent)
    })

    it('is refused from the keyboard too', async () => {
      const [first, second] = hand as [ObjectId, ObjectId]
      const { card, text, pressed, user } = renderChoice(kind, hand, 1)
      act(() => card(first).focus())
      await user.keyboard('{Enter}')
      act(() => card(second).focus())
      await user.keyboard(' ')
      expect(pressed()).toEqual([first])
      expect(text()).toContain('Only 1 card can be chosen.')
    })

    it('lets a different card in once one is put back, and the hint goes', async () => {
      const [first, second, third] = hand as [ObjectId, ObjectId, ObjectId]
      const { card, live, text, pressed, user } = renderChoice(kind, hand, 2)
      await user.click(card(first))
      await user.click(card(second))
      await user.click(card(third))
      expect(live().querySelector('[data-slot="prompt-hint"]')).not.toBeNull()
      await user.click(card(first))
      expect(live().querySelector('[data-slot="prompt-hint"]')).toBeNull()
      expect(text()).toContain('1 of 2 chosen.')
      await user.click(card(third))
      expect(pressed().toSorted()).toEqual([second, third].toSorted())
    })
  })

  describe('the keyboard', () => {
    it.each([
      ['Enter', '{Enter}'],
      ['Space', ' '],
    ])('%s on a card toggles it, Tab reaches Confirm and Enter answers', async (_key, keys) => {
      const [first, second] = hand as [ObjectId, ObjectId]
      const { card, confirm, pressed, answer, user } = renderChoice(kind, hand, 2)
      act(() => card(first).focus())
      await user.keyboard(keys)
      await user.keyboard('{ArrowRight}')
      expect(card(second)).toHaveFocus()
      await user.keyboard(keys)
      expect(pressed()).toEqual([first, second])
      act(() => confirm().focus())
      await user.keyboard('{Enter}')
      expect(answer.mock.calls).toEqual([[kind.answer([first, second])]])
    })

    it('leaves Confirm out of reach until the count is met', async () => {
      const { card, confirm, user } = renderChoice(kind, hand, 1)
      expect(confirm()).toBeDisabled()
      act(() => card(must(hand[0])).focus())
      await user.keyboard('{Enter}')
      expect(confirm()).toBeEnabled()
    })
  })

  describe('cards that are not offered', () => {
    it('are dimmed and a click or key press on one chooses nothing', async () => {
      const offered = hand.slice(0, 3)
      const closed = must(hand[5])
      const { card, text, pressed, marks, answer, user } = renderChoice(kind, offered, 2)
      expect(marks().get(closed)).toBe('dimmed')
      const before = text()
      await user.click(card(closed))
      act(() => card(closed).focus())
      await user.keyboard('{Enter} ')
      expect(pressed()).toEqual([])
      expect(text()).toBe(before)
      expect(answer).not.toHaveBeenCalled()
    })

    it('include the battlefield, which is dimmed and does nothing', async () => {
      const land = must(battlefield[0])
      const { card, marks, text, user } = renderChoice(kind, hand, 2)
      expect(marks().get(land)).toBe('dimmed')
      const before = text()
      await user.click(card(land))
      expect(text()).toBe(before)
    })
  })

  describe('odd counts', () => {
    it('asks for no more cards than are offered', async () => {
      const [first, second] = hand as [ObjectId, ObjectId]
      const { card, text, confirm, answer, user } = renderChoice(kind, [first, second], 5)
      expect(text()).toContain('Choose 2 cards')
      await user.click(card(first))
      expect(confirm()).toBeDisabled()
      await user.click(card(second))
      await user.click(confirm())
      expect(answer.mock.calls).toEqual([[kind.answer([first, second])]])
    })

    it('answers with no cards at once when none are asked for', async () => {
      const { confirm, text, marks, answer, user } = renderChoice(kind, hand, 0)
      expect(text()).toBe(
        `${kind.words === 'discard' ? 'Your hand is over the limit. ' : ''}There are no cards to ${kind.words}. Press Confirm to go on.`,
      )
      expect(confirm()).toBeEnabled()
      expect([...marks().values()].every((mark) => mark === 'dimmed')).toBe(true)
      await user.click(confirm())
      expect(answer.mock.calls).toEqual([[kind.answer([])]])
    })

    it('has no cards to choose from when the hand is empty', async () => {
      const { confirm, answer, user } = renderChoice(kind, [], 2)
      expect(confirm()).toBeEnabled()
      await user.click(confirm())
      expect(answer.mock.calls).toEqual([[kind.answer([])]])
    })
  })

  describe('a new decision', () => {
    it.each([
      ['with the same options', hand, 3],
      ['with other options', hand.slice(2), 2],
    ] as [string, ObjectId[], number][])('%s starts with nothing chosen', async (_how, offered, count) => {
      const { card, text, pressed, marks, decide, confirm, user } = renderChoice(kind, hand, 3)
      await user.click(card(must(hand[3])))
      await user.click(card(must(hand[4])))
      expect(pressed()).toHaveLength(2)
      decide(5, offered, count)
      expect(pressed()).toEqual([])
      expect(text()).toContain(`0 of ${Math.min(count, offered.length)} chosen.`)
      expect(confirm()).toBeDisabled()
      expect([...marks().values()]).not.toContain('selected')
      expect(marks().get(must(offered[0]))).toBe('legal')
    })

    it('forgets a refusal along with the choice', async () => {
      const { card, live, decide, user } = renderChoice(kind, hand, 1)
      await user.click(card(must(hand[0])))
      await user.click(card(must(hand[1])))
      expect(live().querySelector('[data-slot="prompt-hint"]')).not.toBeNull()
      decide(6)
      expect(live().querySelector('[data-slot="prompt-hint"]')).toBeNull()
    })

    it('removes the marks and the prompt when the decision is answered elsewhere', async () => {
      const { marks, update, container } = renderChoice(kind, hand, 1)
      expect(marks().size).toBe(OWN.length)
      update(() => ({ pending: null }))
      expect(marks().size).toBe(0)
      expect(container.querySelector('[data-slot="prompt-hint"]')).toBeNull()
    })
  })
})
