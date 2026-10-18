/** Goal: for every combination of what priority offers (lands, spells, mana abilities, pass or not) exactly those cards of the player's hand and battlefield are marked legal and the rest dimmed; a click, Enter or Space on a legal card answers PlayLand, CastSpell or ActivateMana, a permanent with several mana abilities asks which first, Pass answers Pass, and dimmed cards do nothing. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, ManaOption, ObjectId, PriorityOptions } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const view = fixture.view
const byId = new Map(view.objects.map((object) => [object.id, object]))
const isLand = (id: ObjectId): boolean => byId.get(id)?.types.includes('Land') === true

const hand = must(view.players[0]?.hand.cards)
const handLands = hand.filter(isLand)
const handSpells = hand.filter((id) => !isLand(id))
const battlefield = view.objects.filter((object) => object.zone === 'Battlefield' && object.controller === 0)
const untapped = battlefield.filter(
  (object) => object.permanent?.tapped === false && object.abilities.length > 0,
)
const lands = untapped.filter((object) => isLand(object.id)).map((object) => object.id)
const rock = must(untapped.find((object) => !isLand(object.id)))
const OWN: ObjectId[] = [...hand, ...battlefield.map((object) => object.id)]

const [land1, land2, land3] = lands as [ObjectId, ObjectId, ObjectId]
const [play1, play2] = handLands as [ObjectId, ObjectId]
const [spell1, spell2, spell3] = handSpells as [ObjectId, ObjectId, ObjectId]
const tap = (object: ObjectId, ability = 0, produces: ManaOption['produces'] = 'Green'): ManaOption => ({
  source: { object, ability },
  produces,
})
const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: false,
  can_pass: true,
  ...over,
})

/** What each combination offers, and what a click on each legal card answers. */
const CASES: [string, PriorityOptions, [ObjectId, Answer][]][] = [
  ['only a pass', options(), []],
  ['a land', options({ playable_lands: [play1] }), [[play1, { Priority: { PlayLand: play1 } }]]],
  [
    'two lands',
    options({ playable_lands: [play1, play2] }),
    [
      [play1, { Priority: { PlayLand: play1 } }],
      [play2, { Priority: { PlayLand: play2 } }],
    ],
  ],
  ['a spell', options({ castable: [spell1] }), [[spell1, { Priority: { CastSpell: spell1 } }]]],
  [
    'three spells',
    options({ castable: [spell1, spell2, spell3] }),
    [spell1, spell2, spell3].map((id): [ObjectId, Answer] => [id, { Priority: { CastSpell: id } }]),
  ],
  [
    'one mana ability',
    options({ mana_abilities: [tap(land1)] }),
    [[land1, { Priority: { ActivateMana: { object: land1, ability: 0 } } }]],
  ],
  [
    'mana on three lands and a rock',
    options({
      mana_abilities: [tap(land1), tap(land2, 0, 'Blue'), tap(land3), tap(rock.id, 0, 'Colorless')],
    }),
    [land1, land2, land3, rock.id].map((id): [ObjectId, Answer] => [
      id,
      { Priority: { ActivateMana: { object: id, ability: 0 } } },
    ]),
  ],
  [
    'a land, a spell and mana',
    options({ playable_lands: [play1], castable: [spell2], mana_abilities: [tap(land1)] }),
    [
      [play1, { Priority: { PlayLand: play1 } }],
      [spell2, { Priority: { CastSpell: spell2 } }],
      [land1, { Priority: { ActivateMana: { object: land1, ability: 0 } } }],
    ],
  ],
  [
    'a land with no way to pass',
    options({ playable_lands: [play2], can_pass: false }),
    [[play2, { Priority: { PlayLand: play2 } }]],
  ],
]

/** The table of seat 0 facing `given`, with an `answer` spy that accepts everything. */
function renderPriority(given: PriorityOptions, { emptyStack = false } = {}) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state) => {
      const next = { ...state, pending: { id: 4, decision: { player: 0, kind: { Priority: given } } } }
      if (!emptyStack) return next
      const emptied = clone(must(state.view))
      emptied.stack = { count: 0, cards: [] }
      return { ...next, view: emptied }
    },
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const marks = (): Map<number, string> =>
    new Map(
      [...table.container.querySelectorAll<HTMLElement>('[data-highlight]')].map((element) => [
        Number(element.getAttribute('data-object-id')),
        must(element.getAttribute('data-highlight')),
      ]),
    )
  const pass = (): HTMLElement => screen.getByRole('button', { name: 'Pass priority' })
  return { ...table, answer, card, marks, pass }
}

const legalOf = (given: PriorityOptions): Set<ObjectId> =>
  new Set([...given.playable_lands, ...given.castable, ...given.mana_abilities.map((m) => m.source.object)])

describe('what is marked', () => {
  it.each(CASES)(
    'with %s the legal cards are marked legal, the rest of hand and battlefield dimmed',
    (_name, given) => {
      const { marks } = renderPriority(given)
      const legal = legalOf(given)
      expect(marks()).toEqual(new Map(OWN.map((id) => [id, legal.has(id) ? 'legal' : 'dimmed'])))
    },
  )

  it('marks nothing of an opponent, the stack, a graveyard or a command zone', () => {
    const { marks } = renderPriority(options({ playable_lands: [play1], castable: [spell1] }))
    const marked = new Set(marks().keys())
    for (const object of view.objects) if (!OWN.includes(object.id)) expect(marked.has(object.id)).toBe(false)
  })

  it('also marks a legal card outside the hand and battlefield, such as a commander', () => {
    const commander = must(view.players[0]?.command.cards?.[0])
    const { marks } = renderPriority(options({ castable: [commander] }))
    expect(marks().get(commander)).toBe('legal')
  })

  it('marks the cards for the whole time the decision is on screen and not after', () => {
    const { marks, update } = renderPriority(options({ playable_lands: [play1] }))
    expect(marks().size).toBe(OWN.length)
    update(() => ({ pending: null }))
    expect(marks().size).toBe(0)
  })

  it('says in each name whether a card is available', () => {
    const { card } = renderPriority(options({ playable_lands: [play1] }))
    expect(card(play1)).toHaveAccessibleName(/, available$/)
    expect(card(spell1)).toHaveAccessibleName(/, not available$/)
    expect(card(play1)).not.toHaveAttribute('aria-pressed')
  })
})

describe('what a click answers', () => {
  const rows = CASES.flatMap(([name, given, answers]) =>
    answers.map(([id, expected]): [string, ObjectId, PriorityOptions, Answer] => [name, id, given, expected]),
  )

  it.each(rows)('with %s a click on card %i sends the answer for it', async (_name, id, given, expected) => {
    const { card, answer, user } = renderPriority(given)
    await user.click(card(id))
    expect(answer.mock.calls).toEqual([[expected]])
  })

  it.each(CASES)('with %s a click on a dimmed card sends nothing', async (_name, given) => {
    const { card, answer, user } = renderPriority(given)
    const legal = legalOf(given)
    for (const id of OWN.filter((own) => !legal.has(own))) await user.click(card(id))
    expect(answer).not.toHaveBeenCalled()
  })

  it('does nothing when an opponent card is clicked', async () => {
    const { answer, user, container } = renderPriority(options({ playable_lands: [play1] }))
    const theirs = must(
      container.querySelector<HTMLElement>('[data-battlefield-seat="1"] button[data-object-id]'),
    )
    await user.click(theirs)
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('the keyboard', () => {
  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('%s on a legal card answers like a click', async (_key, keys) => {
    const { card, answer, user } = renderPriority(options({ playable_lands: [play1], castable: [spell1] }))
    act(() => card(spell1).focus())
    await user.keyboard(keys)
    expect(answer.mock.calls).toEqual([[{ Priority: { CastSpell: spell1 } }]])
  })

  it('does nothing on a dimmed card', async () => {
    const { card, answer, user } = renderPriority(options({ playable_lands: [play1] }))
    act(() => card(spell1).focus())
    await user.keyboard('{Enter} ')
    expect(answer).not.toHaveBeenCalled()
  })

  it('moves along the hand with the arrows, and Enter on the Pass button passes', async () => {
    const { card, answer, user, pass } = renderPriority(options({ playable_lands: [play1, play2] }))
    act(() => card(play1).focus())
    await user.keyboard('{ArrowRight}')
    expect(card(play2)).toHaveFocus()
    act(() => pass().focus())
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([[{ Priority: 'Pass' }]])
  })
})

describe('Pass', () => {
  it('answers Pass', async () => {
    const { answer, user, pass } = renderPriority(options())
    await user.click(pass())
    expect(answer.mock.calls).toEqual([[{ Priority: 'Pass' }]])
  })

  it('cannot be pressed when passing is not allowed', async () => {
    const { answer, user, pass } = renderPriority(options({ playable_lands: [play1], can_pass: false }))
    expect(pass()).toBeDisabled()
    await user.click(pass())
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('the words', () => {
  const text = (container: HTMLElement): string =>
    must(container.querySelector('[data-slot="prompt-text"]')).textContent ?? ''

  it('says it is a response when there is a spell on the stack', () => {
    const { container } = renderPriority(options({ castable: [spell1] }))
    expect(text(container)).toBe(
      'You are responding to your Lightning Bolt. You can cast a spell, or pass to let it resolve.',
    )
  })

  it('says what may be done when the stack is empty', () => {
    const { container } = renderPriority(options({ playable_lands: [play1] }), { emptyStack: true })
    expect(text(container)).toBe('You have priority. You can play a land, or pass.')
  })

  it('is one polite live region under the name of the decision', () => {
    const { container } = renderPriority(options())
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(within(line).getByText('Your decision: Priority')).toBeInTheDocument()
    expect(line.querySelectorAll('[aria-live]')).toHaveLength(1)
    expect(must(line.querySelector('[aria-live]'))).toHaveAttribute('aria-atomic', 'true')
  })
})

describe('a permanent with several mana abilities', () => {
  const choices = [tap(rock.id, 0, 'Colorless'), tap(rock.id, 1, 'Red')]
  const given = options({ mana_abilities: [tap(land1), ...choices] })
  const group = (): HTMLElement => screen.getByRole('group', { name: `Mana from ${rock.name}` })

  it('asks which mana instead of answering', async () => {
    const { card, answer, user, marks } = renderPriority(given)
    await user.click(card(rock.id))
    expect(answer).not.toHaveBeenCalled()
    expect(
      within(group())
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-label') ?? button.textContent),
    ).toEqual(['Add colorless mana', 'Add red mana', 'Cancel'])
    expect(marks().get(rock.id)).toBe('selected')
    expect(marks().get(land1)).toBe('legal')
  })

  it('moves focus into the choice, and the choice answers with the chosen ability', async () => {
    const { card, answer, user } = renderPriority(given)
    await user.click(card(rock.id))
    expect(screen.getByRole('button', { name: 'Add colorless mana' })).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Add red mana' }))
    expect(answer.mock.calls).toEqual([[{ Priority: { ActivateMana: { object: rock.id, ability: 1 } } }]])
  })

  it('answers from the keyboard alone', async () => {
    const { card, answer, user } = renderPriority(given)
    act(() => card(rock.id).focus())
    await user.keyboard('{Enter}')
    await user.keyboard('{Tab}{Enter}')
    expect(answer.mock.calls).toEqual([[{ Priority: { ActivateMana: { object: rock.id, ability: 1 } } }]])
  })

  type Table = ReturnType<typeof renderPriority>
  const CANCELS: [string, (table: Table) => Promise<void>][] = [
    ['Escape', async ({ user }) => user.keyboard('{Escape}')],
    ['the Cancel button', async ({ user }) => user.click(screen.getByRole('button', { name: 'Cancel' }))],
    ['clicking the card again', async ({ user, card }) => user.click(card(rock.id))],
  ]
  it.each(CANCELS)('is cancelled by %s, which leaves focus on the card', async (_how, cancel) => {
    const table = renderPriority(given)
    await table.user.click(table.card(rock.id))
    await cancel(table)
    expect(screen.queryByRole('group', { name: `Mana from ${rock.name}` })).toBeNull()
    expect(table.card(rock.id)).toHaveAttribute('data-highlight', 'legal')
    expect(table.card(rock.id)).toHaveFocus()
    expect(table.answer).not.toHaveBeenCalled()
  })

  it('still lets another legal card be chosen while asking', async () => {
    const { card, answer, user } = renderPriority(options({ ...given, playable_lands: [play1] }))
    await user.click(card(rock.id))
    await user.click(card(play1))
    expect(answer.mock.calls).toEqual([[{ Priority: { PlayLand: play1 } }]])
  })

  it('moves the choice to another permanent with several abilities', async () => {
    const two = [tap(land1, 0, 'Green'), tap(land1, 1, 'Blue')]
    const { card, user, marks } = renderPriority(options({ mana_abilities: [...choices, ...two] }))
    await user.click(card(rock.id))
    await user.click(card(land1))
    expect(marks().get(rock.id)).toBe('legal')
    expect(marks().get(land1)).toBe('selected')
    expect(screen.getByRole('group', { name: `Mana from ${must(byId.get(land1)).name}` })).toBeInTheDocument()
  })

  it('tells two abilities of one colour apart', async () => {
    const same = [tap(rock.id, 0, 'Red'), tap(rock.id, 1, 'Red')]
    const { card, user } = renderPriority(options({ mana_abilities: same }))
    await user.click(card(rock.id))
    expect(screen.getByRole('button', { name: 'Add red (ability 1) mana' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Add red (ability 2) mana' })).toBeInTheDocument()
  })
})
