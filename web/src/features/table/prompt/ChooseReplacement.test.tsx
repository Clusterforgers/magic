/** Goal: the replacement prompt lists each effect with its source card's name, picture and rules text, names effects of one card apart, answers the index of the effect chosen exactly once from a click, Enter or the keys 1 to 9 for 1, 2, 3, 5 and 9 effects, starts with focus on the first (the effect the clock takes), explains the choice in one sentence, and marks nothing on the table. */
import { screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import type { CardDetail, ObjectId, ReplacementCandidate } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { renderTable } from '@/test-utils/tableHarness'
import { setOracleFetcher } from './oracleWords'

const fixture = fixtureOf(2)
const permanents = fixture.view.objects
  .filter((o) => o.zone === 'Battlefield' && o.controller === 0 && !o.types.includes('Land'))
  .map((o) => o.id)
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((o) => o.id === id)).name

const effects = (count: number): ReplacementCandidate[] =>
  Array.from({ length: count }, (_, index) => ({
    source: must(permanents[index % permanents.length]),
    ability: 0,
  }))

function mount(candidates: ReplacementCandidate[], text?: string) {
  const table = renderDecision({
    ChooseReplacement: { candidates, ...(text === undefined ? {} : { text }) },
  })
  const rows = (): HTMLElement[] =>
    within(screen.getByRole('list', { name: 'Replacement effects' })).getAllByRole('button')
  return { ...table, rows }
}

afterEach(() => setOracleFetcher(() => Promise.reject(new Error('no server'))))

describe('the list', () => {
  it.each([1, 2, 3, 5, 9])(
    'of %i effects has a row for each, in the order offered, with its key',
    (count) => {
      const candidates = effects(count)
      const { rows, marks } = mount(candidates)
      expect(rows()).toHaveLength(count)
      rows().forEach((row, index) => {
        expect(row).toHaveAttribute('aria-keyshortcuts', String(index + 1))
        expect(row).toHaveTextContent(nameOf(must(candidates[index]).source))
      })
      // Nothing on the table is clickable: the answer is the list's.
      expect(marks().size).toBe(0)
    },
  )

  it('draws the source card as a picture beside its name', () => {
    const { rows } = mount(effects(2))
    for (const row of rows()) expect(row.querySelector('[data-slot="prompt-picture"]')).not.toBeNull()
  })

  it('draws nothing to press for an empty list', () => {
    const { container } = mount([])
    expect(container.querySelectorAll('[data-slot="replacement-effect"]')).toHaveLength(0)
  })
})

describe('the words', () => {
  it.each([
    ['Create 2 token(s).', 'Create 2 token(s). Choose which applies first; the others apply after it.'],
    [
      undefined,
      'Several replacement effects apply to one event. Choose which applies first; the others apply after it.',
    ],
  ])('with the event %j read "%s"', (event, sentence) => {
    const { text } = mount(effects(2), event)
    expect(text()).toBe(`${sentence} Press 1 or 2 to choose.`)
  })

  it.each([
    [1, 'Press 1 to choose it.'],
    [3, 'Press 1 to 3 to choose.'],
    [9, 'Press 1 to 9 to choose.'],
  ])('tell a screen reader the keys for %i effects', (count, keys) => {
    expect(mount(effects(count)).text()).toContain(keys)
  })

  it('names two effects of one card apart, and a lasting effect', () => {
    const source = must(permanents[0])
    const { rows } = mount([
      { source, ability: 0 },
      { source, ability: 1 },
      { source, ability: 0, shield: 4 },
    ])
    const name = nameOf(source)
    expect(rows().map((row) => row.getAttribute('aria-label'))).toEqual([
      `${name} (ability 1)`,
      `${name} (ability 2)`,
      `${name} (lasting effect): A lasting effect made by a spell or ability`,
    ])
  })

  it('writes the rules text of the source card when the card database has it', async () => {
    const text = 'If a token would be created, twice that many are created instead.'
    setOracleFetcher(() => Promise.resolve({ faces: [{ oracle_text: text }] } as CardDetail))
    const first = must(permanents[0])
    const view = {
      ...fixture.view,
      objects: fixture.view.objects.map((o) =>
        o.id === first ? { ...o, identity: { oracle_id: 'doubler' } } : o,
      ),
    }
    const answer = () => true
    const table = renderTable({
      fixture: { ...fixture, view },
      session: { answer },
      state: (state) => ({
        ...state,
        pending: {
          id: 7,
          decision: {
            player: 0,
            kind: { ChooseReplacement: { candidates: effects(2), text: 'Create 1 token(s).' } },
          },
        },
      }),
    })
    const row = (await screen.findByText(text)).closest('button')
    expect(row).toHaveAttribute('aria-label', `${nameOf(first)}: ${text}`)
    expect(table.container.querySelectorAll('[data-slot="replacement-wording"]')).toHaveLength(1)
  })
})

describe('answering', () => {
  it.each([
    [2, 0],
    [2, 1],
    [3, 2],
    [5, 4],
    [9, 8],
  ])('with %i effects a click on row %i sends that index once', async (count, index) => {
    const { rows, user, answer } = mount(effects(count))
    await user.dblClick(must(rows()[index]))
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseReplacement: index })
  })

  it.each([
    [2, '1', 0],
    [2, '2', 1],
    [5, '5', 4],
    [9, '9', 8],
  ])('with %i effects the key %s sends index %i', async (count, key, index) => {
    const { user, answer } = mount(effects(count))
    await user.keyboard(key)
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseReplacement: index })
  })

  it('ignores a key past the last effect and sends one answer for keys pressed again', async () => {
    const { user, answer } = mount(effects(2))
    await user.keyboard('3')
    expect(answer).not.toHaveBeenCalled()
    await user.keyboard('21')
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseReplacement: 1 })
  })

  it('leaves the digits to a text field', async () => {
    const { user, answer } = mount(effects(2))
    const field = document.createElement('input')
    document.body.append(field)
    field.focus()
    await user.keyboard('1')
    field.remove()
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('focus', () => {
  it('starts on the first effect, so Enter at once answers what the clock would', async () => {
    const { rows, user, answer } = mount(effects(3))
    expect(rows()[0]).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseReplacement: 0 })
  })

  it('takes Tab to the next effect and Enter answers it', async () => {
    const { rows, user, answer } = mount(effects(3))
    await user.tab()
    expect(rows()[1]).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseReplacement: 1 })
  })
})
