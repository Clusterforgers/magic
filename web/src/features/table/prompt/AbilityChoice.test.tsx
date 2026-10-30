// Goal: a permanent or a card of the viewer's graveyard or command zone with an activatable ability is marked legal like a hand card and named "Activate <ability>", a click activates it or, when the card has several plays, opens a chooser at the card that picks one by mouse or keyboard and closes on Escape, Tab, a press outside or a second click on the card; a mana ability is no play, an opponent's card is never marked.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { indexById } from '@/play/view'
import type { PlayState } from '@/play/store'
import type {
  AbilityRef,
  ActivatedView,
  Answer,
  ObjectId,
  ObjectView,
  PriorityOptions,
  StateView,
} from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const objects = fixture.view.objects
const mine = objects.filter((object) => object.zone === 'Battlefield' && object.controller === 0)
const creatures = mine.filter((object) => object.types.includes('Creature'))
const [creature, other] = creatures as [ObjectView, ObjectView]
const theirs = must(objects.find((object) => object.zone === 'Battlefield' && object.controller === 1))
const rock = must(mine.find((object) => !object.types.includes('Land') && object.abilities.length > 0))
const yard = must(
  objects.find(
    (object) => typeof object.zone === 'object' && 'Graveyard' in object.zone && object.zone.Graveyard === 0,
  ),
)
const commander = must(
  objects.find(
    (object) => typeof object.zone === 'object' && 'Command' in object.zone && object.zone.Command === 0,
  ),
)
const spell = must(
  objects.find(
    (object) =>
      typeof object.zone === 'object' &&
      'Hand' in object.zone &&
      object.zone.Hand === 0 &&
      !object.types.includes('Land'),
  ),
)

const ref = (object: ObjectId, ability = 0): AbilityRef => ({ object, ability })
const draw = (of: AbilityRef): ActivatedView => ({ ability: of, cost: '{2}, {T}', effect: 'Draw a card.' })
const scry = (of: AbilityRef): ActivatedView => ({ ability: of, cost: '{T}', effect: 'Scry 1.' })
const options = (abilities: AbilityRef[], over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  abilities,
  can_act: true,
  can_pass: true,
  ...over,
})

/** The table of seat 0 facing `given`, `words` set as the cards' activated abilities and `edit` applied to the view. */
function renderAbilities(
  given: PriorityOptions,
  words: [ObjectView, ...ActivatedView[]][] = [],
  edit: (view: StateView) => void = () => undefined,
) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state: PlayState) => {
      const view = clone(must(state.view))
      for (const [object, ...activated] of words)
        must(view.objects.find((o) => o.id === object.id)).activated = activated
      edit(view)
      return {
        ...state,
        view,
        objectsById: indexById(view.objects),
        pending: { id: 4, decision: { player: 0, kind: { Priority: given } } },
      }
    },
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const highlight = (id: ObjectId): string | null => card(id).getAttribute('data-highlight')
  const menu = (name: string) => screen.getByRole('menu', { name: `What to do with ${name}` })
  const items = (name: string): string[] =>
    within(menu(name))
      .getAllByRole('menuitem')
      .map((item) => item.getAttribute('aria-label') ?? '')
  return { ...table, answer, card, highlight, menu, items }
}

describe('a permanent with one ability', () => {
  const ability = ref(creature.id)
  const given = options([ability])
  const words: [ObjectView, ...ActivatedView[]][] = [[creature, draw(ability)]]

  it('is marked legal and the rest of the viewer’s board dimmed', () => {
    const { highlight } = renderAbilities(given, words)
    expect(highlight(creature.id)).toBe('legal')
    expect(highlight(other.id)).toBe('dimmed')
  })

  it('is named by the ability it activates', () => {
    const { card } = renderAbilities(given, words)
    expect(card(creature.id).getAttribute('aria-label')).toContain(
      ', available, Activate 2, tap: Draw a card.',
    )
  })

  it('activates on a click, without a chooser', async () => {
    const { card, answer, user } = renderAbilities(given, words)
    await user.click(card(creature.id))
    expect(answer.mock.calls).toEqual([[{ Priority: { Activate: ability } }]])
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('activates on Enter', async () => {
    const { card, answer, user } = renderAbilities(given, words)
    act(() => card(creature.id).focus())
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([[{ Priority: { Activate: ability } }]])
  })

  it('does nothing on a click on a card the offer leaves out', async () => {
    const { card, answer, user } = renderAbilities(given, words)
    await user.click(card(other.id))
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('which cards are marked', () => {
  const mana = { ...draw(ref(creature.id)), mana: true }
  const cases: [string, PriorityOptions, [ObjectView, ...ActivatedView[]][], ObjectId, string | null][] = [
    [
      'your permanent',
      options([ref(creature.id)]),
      [[creature, draw(ref(creature.id))]],
      creature.id,
      'legal',
    ],
    ['no ability in the offer', options([]), [[creature, draw(ref(creature.id))]], creature.id, 'dimmed'],
    [
      'a mana ability, which is no play',
      options([ref(creature.id)]),
      [[creature, mana]],
      creature.id,
      'dimmed',
    ],
    ['an opponent’s permanent', options([ref(theirs.id)]), [[theirs, draw(ref(theirs.id))]], theirs.id, null],
    [
      'a card the offer names twice',
      options([ref(creature.id), ref(creature.id, 1)]),
      [],
      creature.id,
      'legal',
    ],
  ]
  it.each(cases)('%s', (_name, given, words, id, mark) => {
    expect(renderAbilities(given, words).highlight(id)).toBe(mark)
  })

  it('leaves a land that taps for mana marked for its mana, and no more', () => {
    const land = must(mine.find((object) => object.types.includes('Land') && object.abilities.length > 0))
    const given = options([], {
      mana_abilities: [{ source: { object: land.id, ability: 0 }, produces: 'Green' }],
    })
    const { highlight, card } = renderAbilities(given)
    expect(highlight(land.id)).toBe('legal')
    expect(card(land.id).getAttribute('aria-label')).not.toContain('Activate')
  })
})

describe('a permanent with several abilities', () => {
  const [first, second] = [ref(creature.id, 0), ref(creature.id, 1)]
  const given = options([first, second])
  const words: [ObjectView, ...ActivatedView[]][] = [[creature, scry(first), draw(second)]]
  const NAME = creature.name

  it('is named by how many abilities it has', () => {
    const { card } = renderAbilities(given, words)
    expect(card(creature.id).getAttribute('aria-label')).toContain('Activate one of 2 abilities')
  })

  it('opens a chooser at the card instead of answering, with the first line focused', async () => {
    const { card, answer, user, items } = renderAbilities(given, words)
    await user.click(card(creature.id))
    expect(answer).not.toHaveBeenCalled()
    expect(items(NAME)).toEqual(['Activate tap: Scry 1.', 'Activate 2, tap: Draw a card.'])
    expect(screen.getByRole('menuitem', { name: 'Activate tap: Scry 1.' })).toHaveFocus()
  })

  it('answers with the ability that is clicked', async () => {
    const { card, answer, user } = renderAbilities(given, words)
    await user.click(card(creature.id))
    await user.click(screen.getByRole('menuitem', { name: 'Activate 2, tap: Draw a card.' }))
    expect(answer.mock.calls).toEqual([[{ Priority: { Activate: second } }]])
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('answers from the keyboard alone', async () => {
    const { card, answer, user } = renderAbilities(given, words)
    act(() => card(creature.id).focus())
    await user.keyboard('{Enter}')
    await user.keyboard('{ArrowDown}{Enter}')
    expect(answer.mock.calls).toEqual([[{ Priority: { Activate: second } }]])
  })

  it('moves along its lines with the arrows, Home and End, and wraps', async () => {
    const { card, user } = renderAbilities(given, words)
    act(() => card(creature.id).focus())
    await user.keyboard('{Enter}')
    const lines = screen.getAllByRole('menuitem')
    await user.keyboard('{ArrowUp}')
    expect(lines[1]).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(lines[0]).toHaveFocus()
    await user.keyboard('{End}')
    expect(lines[1]).toHaveFocus()
    await user.keyboard('{Home}')
    expect(lines[0]).toHaveFocus()
  })

  type Table = ReturnType<typeof renderAbilities>
  const CLOSES: [string, (table: Table) => Promise<void>][] = [
    ['Escape', async ({ user }) => user.keyboard('{Escape}')],
    ['Tab', async ({ user }) => user.keyboard('{Tab}')],
    ['a second click on the card', async ({ user, card }) => user.click(card(creature.id))],
  ]
  it.each(CLOSES)('closes on %s, answers nothing and leaves focus on the card', async (_how, close) => {
    const table = renderAbilities(given, words)
    await table.user.click(table.card(creature.id))
    expect(screen.getByRole('menu')).toBeInTheDocument()
    await close(table)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(table.answer).not.toHaveBeenCalled()
    expect(table.card(creature.id)).toHaveFocus()
  })

  it('closes on a press outside, which takes the focus where it was pressed', async () => {
    const table = renderAbilities(given, words)
    await table.user.click(table.card(creature.id))
    await table.user.click(document.body)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(table.answer).not.toHaveBeenCalled()
  })

  it('answers nothing when another card is clicked while it is open, and leaves the card for that click', async () => {
    const table = renderAbilities(options([first, second, ref(other.id)]), [
      ...words,
      [other, draw(ref(other.id))],
    ])
    await table.user.click(table.card(creature.id))
    await table.user.click(table.card(other.id))
    expect(table.answer.mock.calls).toEqual([[{ Priority: { Activate: ref(other.id) } }]])
  })

  it('shows the effect under a keyword’s line', async () => {
    const equip: ActivatedView = {
      ability: second,
      cost: '{1}',
      effect: 'Attach to target creature you control.',
      keyword: 'Equip {1}',
    }
    const { card, user } = renderAbilities(given, [[creature, scry(first), equip]])
    await user.click(card(creature.id))
    const line = screen.getByRole('menuitem', { name: 'Activate Equip 1' })
    expect(line).toHaveTextContent('Equip {1}')
    expect(line).toHaveTextContent('{1}: Attach to target creature you control.')
  })
})

describe('a permanent with an ability and mana', () => {
  const ability = ref(rock.id, 5)
  const tap = { source: { object: rock.id, ability: 0 }, produces: 'Green' as const }
  const given = options([ability], { mana_abilities: [tap] })
  const words: [ObjectView, ...ActivatedView[]][] = [[rock, scry(ability)]]

  it('asks which, with the mana among the lines', async () => {
    const { card, user, items } = renderAbilities(given, words)
    expect(card(rock.id)).toHaveAttribute('data-highlight', 'legal')
    await user.click(card(rock.id))
    expect(items(rock.name)).toEqual(['Activate tap: Scry 1.', 'Add green mana'])
  })

  it('taps for the mana that is chosen', async () => {
    const { card, answer, user } = renderAbilities(given, words)
    await user.click(card(rock.id))
    await user.click(screen.getByRole('menuitem', { name: 'Add green mana' }))
    expect(answer.mock.calls).toEqual([[{ Priority: { ActivateMana: tap.source } }]])
  })

  it('still taps at once with no ability beside the mana', async () => {
    const { card, answer, user } = renderAbilities(options([], { mana_abilities: [tap] }))
    await user.click(card(rock.id))
    expect(answer.mock.calls).toEqual([[{ Priority: { ActivateMana: tap.source } }]])
  })
})

describe('a card of the graveyard', () => {
  const ability = ref(yard.id)
  const given = options([ability])
  const words: [ObjectView, ...ActivatedView[]][] = [
    [yard, { ...draw(ability), cost: '{3}{B}, Exile Gravedigger' }],
  ]

  async function openYard(table: ReturnType<typeof renderAbilities>) {
    const chip = must(
      table.container.querySelector<HTMLElement>('[data-seat="0"] button[data-zone="graveyard"]'),
    )
    await table.user.click(chip)
    return screen.getByRole('dialog')
  }

  it('is marked legal in the list of the graveyard, which is no part of the viewer’s own cards', async () => {
    const table = renderAbilities(given, words)
    const dialog = await openYard(table)
    const entry = must(dialog.querySelector<HTMLElement>(`button[data-object-id="${yard.id}"]`))
    expect(entry).toHaveAttribute('data-highlight', 'legal')
    expect(entry.getAttribute('aria-label')).toContain('Activate 3 black, Exile Gravedigger')
  })

  it('activates on a click', async () => {
    const table = renderAbilities(given, words)
    const dialog = await openYard(table)
    await table.user.click(must(dialog.querySelector<HTMLElement>(`button[data-object-id="${yard.id}"]`)))
    expect(table.answer.mock.calls).toEqual([[{ Priority: { Activate: ability } }]])
  })

  it('opens its chooser inside the list, so the list does not swallow the focus', async () => {
    const two = [ability, ref(yard.id, 1)]
    const table = renderAbilities(options(two), [
      [yard, scry(two[0] as AbilityRef), draw(two[1] as AbilityRef)],
    ])
    const dialog = await openYard(table)
    await table.user.click(must(dialog.querySelector<HTMLElement>(`button[data-object-id="${yard.id}"]`)))
    expect(dialog).toContainElement(table.menu(yard.name))
    expect(screen.getAllByRole('menuitem')[0]).toHaveFocus()
  })

  it('is not marked for a card in an opponent’s graveyard, though the offer names it', () => {
    const table = renderAbilities(given, words, (view) => {
      const card = must(view.objects.find((object) => object.id === yard.id))
      card.zone = { Graveyard: 1 }
      card.owner = 1
      card.controller = 1
    })
    expect(
      table.container.querySelector(`button[data-object-id="${yard.id}"][data-highlight="legal"]`),
    ).toBeNull()
  })
})

describe('a commander in the command zone', () => {
  const ability = ref(commander.id)

  it('is marked legal in the hand dock, named by its ability, and activates on a click', async () => {
    const table = renderAbilities(options([ability]), [[commander, scry(ability)]])
    expect(table.highlight(commander.id)).toBe('legal')
    expect(table.card(commander.id).getAttribute('aria-label')).toContain('Activate tap: Scry 1.')
    await table.user.click(table.card(commander.id))
    expect(table.answer.mock.calls).toEqual([[{ Priority: { Activate: ability } }]])
  })

  it('asks which when it can also be cast', async () => {
    const table = renderAbilities(options([ability], { castable: [commander.id] }), [
      [commander, scry(ability)],
    ])
    await table.user.click(table.card(commander.id))
    expect(table.items(commander.name)).toEqual([`Cast ${commander.name}`, 'Activate tap: Scry 1.'])
    await table.user.click(screen.getByRole('menuitem', { name: `Cast ${commander.name}` }))
    expect(table.answer.mock.calls).toEqual([[{ Priority: { CastSpell: commander.id } }]])
  })
})

describe('a card in the hand', () => {
  it('is not offered its ability, so it keeps its own mark', () => {
    const table = renderAbilities(options([ref(spell.id)]), [[spell, scry(ref(spell.id))]])
    expect(table.highlight(spell.id)).toBe('dimmed')
  })
})
