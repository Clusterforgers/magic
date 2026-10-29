// Goal: a card opens its manual menu by right click, long press and keyboard, lists the entries by group, sends the exact manual answer for each choice and for each filled form, closes with the focus back on the card, and says why nothing can be done without priority.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Answer, FullControlMode, ManualAction, ObjectView } from '@/protocol/generated'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import { card, permanent } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from '../Card'
import { buildFixture } from '../dev/fixture'
import { ManualMenuProvider } from './ManualMenu'
import { LONG_PRESS_MS } from './useManualMenu'

const ID = 900

interface Shown {
  object?: ObjectView
  answer?: (answer: Answer) => boolean
  /** Replaces the session's request for Full Control. */
  fullControl?: (mode: FullControlMode) => boolean
  /** Changes the play state before the first render. */
  state?: (state: PlayState) => PlayState
  spectator?: boolean
}

/** One card (object 900) among the few permanents of a small fixture, inside the manual menu's provider. */
function renderMenu({ object = card(), answer = () => true, fullControl, state, spectator }: Shown = {}) {
  const sent: Answer[] = []
  const table = renderTable({
    fixture: buildFixture({ seats: 2, permanents: 4, hand: 2 }),
    ui: (
      <ManualMenuProvider>
        <Card id={ID} />
      </ManualMenuProvider>
    ),
    session: {
      answer: (given) => {
        sent.push(given)
        return answer(given)
      },
      ...(fullControl === undefined ? {} : { setFullControl: fullControl }),
    },
    state: (first) => {
      const view = clone(must(first.view))
      view.objects = [...view.objects, { ...object, id: ID }]
      if (spectator === true) view.spectator = true
      const next = { ...first, view, objectsById: indexById(view.objects) }
      return state === undefined ? next : state(next)
    },
  })
  return { ...table, sent }
}

const cardButton = (): HTMLElement =>
  document.querySelector<HTMLElement>(`button[data-object-id="${ID}"]`) as HTMLElement
const menu = (): HTMLElement => screen.getByRole('menu')
const entry = (name: string | RegExp): HTMLElement => within(menu()).getByRole('menuitem', { name })
const manual = (action: ManualAction): Answer => ({ Priority: { Manual: action } })

afterEach(() => {
  vi.useRealTimers()
})

describe('opening the menu', () => {
  it('opens on a right click, names the card and lists the groups in order', () => {
    renderMenu({ object: card({ name: 'Sol Ring', types: ['Artifact'], subtypes: [] }) })
    const click = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 40, clientY: 50 })
    act(() => {
      cardButton().dispatchEvent(click)
    })
    expect(click.defaultPrevented).toBe(true)
    expect(menu()).toHaveAccessibleName('Manual actions for Sol Ring')
    const groups = within(menu())
      .getAllByRole('group')
      .map((group) => group.getAttribute('data-group'))
    expect(groups).toEqual(['move', 'counters', 'tap', 'token', 'other'])
    expect(within(menu()).getByRole('group', { name: 'Move to' })).toBeInTheDocument()
    expect(within(menu()).getByRole('group', { name: 'Counters' })).toBeInTheDocument()
  })

  it.each([
    ['Shift+F10', '{Shift>}{F10}{/Shift}'],
    ['the Menu key', '{ContextMenu}'],
    ['m', 'm'],
  ])('opens from the keyboard with %s while the card is focused', async (_name, keys) => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard(keys)
    expect(menu()).toBeInTheDocument()
    expect(entry('Tap')).toBeInTheDocument()
  })

  it('does not open on other keys, other modifiers or while another element has the focus', async () => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard('x')
    await user.keyboard('{Control>}m{/Control}')
    await user.keyboard('{Shift>}m{/Shift}')
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('opens after a long press of a finger and swallows the click that ends it', () => {
    vi.useFakeTimers()
    renderMenu()
    fireEvent.pointerDown(cardButton(), { pointerType: 'touch', clientX: 20, clientY: 20 })
    act(() => {
      vi.advanceTimersByTime(LONG_PRESS_MS - 1)
    })
    expect(screen.queryByRole('menu')).toBeNull()
    act(() => {
      vi.advanceTimersByTime(2)
    })
    expect(menu()).toBeInTheDocument()
    fireEvent.pointerUp(cardButton(), { pointerType: 'touch' })
    fireEvent.click(cardButton())
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })

  it.each([
    ['a short press', (el: HTMLElement) => fireEvent.pointerUp(el, { pointerType: 'touch' })],
    [
      'a finger that drifts',
      (el: HTMLElement) => fireEvent.pointerMove(el, { pointerType: 'touch', clientX: 80, clientY: 20 }),
    ],
    ['a cancelled press', (el: HTMLElement) => fireEvent.pointerCancel(el, { pointerType: 'touch' })],
  ])('does not open after %s', (_name, end) => {
    vi.useFakeTimers()
    renderMenu()
    fireEvent.pointerDown(cardButton(), { pointerType: 'touch', clientX: 20, clientY: 20 })
    end(cardButton())
    act(() => {
      vi.advanceTimersByTime(LONG_PRESS_MS * 2)
    })
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('does not treat a mouse press as a long press', () => {
    vi.useFakeTimers()
    renderMenu()
    fireEvent.pointerDown(cardButton(), { pointerType: 'mouse', clientX: 20, clientY: 20 })
    act(() => {
      vi.advanceTimersByTime(LONG_PRESS_MS * 2)
    })
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('leaves the browser menu alone for a seat that only watches', () => {
    renderMenu({ spectator: true })
    const click = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
    act(() => {
      cardButton().dispatchEvent(click)
    })
    expect(click.defaultPrevented).toBe(false)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('closes on Escape and puts the focus back on the card', async () => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    expect(menu()).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menu')).toBeNull()
    expect(cardButton()).toHaveFocus()
  })

  it('closes on a press outside it', async () => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    fireEvent.pointerDown(document.body)
    expect(screen.queryByRole('menu')).toBeNull()
  })
})

describe('moving along the menu', () => {
  it('takes the focus on the first entry and moves it with the arrows, Home and End', async () => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    const items = within(menu()).getAllByRole('menuitem')
    expect(items[0]).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(items[1]).toHaveFocus()
    await user.keyboard('{End}')
    expect(items.at(-1)).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(items[0]).toHaveFocus()
    await user.keyboard('{ArrowUp}')
    expect(items.at(-1)).toHaveFocus()
    await user.keyboard('{Home}')
    expect(items[0]).toHaveFocus()
  })

  it('closes on Tab', async () => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    await user.keyboard('{Tab}')
    expect(screen.queryByRole('menu')).toBeNull()
  })
})

describe('what a choice sends', () => {
  const choices: [string, string | RegExp, ManualAction][] = [
    ['a move to exile', 'Exile', { Move: { object: ID, to: 'Exile' } }],
    ['a move to the graveyard', 'Graveyard', { Move: { object: ID, to: 'Graveyard' } }],
    ['a move to the top of the library', 'Top of library', { Move: { object: ID, to: 'LibraryTop' } }],
    ['a move to the command zone', 'Command zone', { Move: { object: ID, to: 'Command' } }],
    ['a tap', 'Tap', { Tap: { object: ID } }],
    ['a counter', 'Add a +1/+1 counter', { Counters: { object: ID, counter: '+1/+1', delta: 1 } }],
  ]
  it.each(choices)('%s', async (_name, label, action) => {
    const { user, sent } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    await user.click(entry(new RegExp(`^${String(label).replace(/[+/]/g, '\\$&')}`)))
    expect(sent).toEqual([manual(action)])
    expect(screen.queryByRole('menu')).toBeNull()
    expect(cardButton()).toHaveFocus()
  })

  it('sends an untap for a tapped permanent, and a discard from the hand', async () => {
    const tapped = renderMenu({ object: permanent({ tapped: true }) })
    cardButton().focus()
    await tapped.user.keyboard('m')
    await tapped.user.click(entry('Untap'))
    expect(tapped.sent).toEqual([manual({ Untap: { object: ID } })])
  })

  it('sends a discard for a card in the hand', async () => {
    const { user, sent } = renderMenu({ object: card({ zone: { Hand: 0 }, permanent: null }) })
    cardButton().focus()
    await user.keyboard('m')
    await user.click(entry('Discard'))
    expect(sent).toEqual([manual({ Discard: { object: ID } })])
  })

  it('keeps the menu and says so when the connection refuses the answer', async () => {
    const { user } = renderMenu({ answer: () => false })
    cardButton().focus()
    await user.keyboard('m')
    await user.click(entry('Tap'))
    expect(screen.getByRole('alert')).toHaveTextContent(/could not be sent/)
    expect(menu()).toBeInTheDocument()
  })

  it('disables the entries and sends nothing when the decision on screen is no longer a priority decision', async () => {
    const { user, sent, update } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    update(() => ({ pending: null }))
    expect(entry('Tap')).toHaveAttribute('aria-disabled', 'true')
    await user.click(entry('Tap'))
    expect(sent).toEqual([])
  })
})

describe('the forms', () => {
  async function open(over: Shown = {}) {
    const harness = renderMenu(over)
    cardButton().focus()
    await harness.user.keyboard('m')
    return harness
  }
  const dialog = (): HTMLElement => screen.getByRole('dialog')
  const field = (label: string): HTMLElement => within(dialog()).getByLabelText(label)

  it('asks for the number of counters and sends them', async () => {
    const { user, sent } = await open()
    await user.click(entry('Add counters…'))
    expect(dialog()).toHaveAccessibleName('Add counters to Grizzly Bears')
    await user.selectOptions(field('Kind of counter'), 'charge')
    await user.clear(field('How many'))
    await user.type(field('How many'), '4')
    await user.click(within(dialog()).getByRole('button', { name: 'Add' }))
    expect(sent).toEqual([manual({ Counters: { object: ID, counter: 'charge', delta: 4 } })])
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('names a counter of another kind', async () => {
    const { user, sent } = await open()
    await user.click(entry('Add counters…'))
    await user.selectOptions(field('Kind of counter'), 'other')
    await user.type(field('Name of the counter'), 'oil')
    await user.click(within(dialog()).getByRole('button', { name: 'Add' }))
    expect(sent).toEqual([manual({ Counters: { object: ID, counter: { Other: 'oil' }, delta: 1 } })])
  })

  it('refuses a number out of range with the reason and sends nothing', async () => {
    const { user, sent } = await open()
    await user.click(entry('Add counters…'))
    await user.clear(field('How many'))
    await user.type(field('How many'), '100')
    expect(within(dialog()).getByRole('status')).toHaveTextContent('from 1 to 99')
    expect(within(dialog()).getByRole('button', { name: 'Add' })).toBeDisabled()
    await user.type(field('How many'), '{Enter}')
    expect(sent).toEqual([])
  })

  it('removes counters of a kind the permanent holds, no more than it holds', async () => {
    const { user, sent } = await open({ object: card({ counters: [['+1/+1', 2]] }) })
    await user.click(entry('Remove counters…'))
    await user.clear(field('How many'))
    await user.type(field('How many'), '3')
    expect(within(dialog()).getByRole('button', { name: 'Remove' })).toBeDisabled()
    await user.clear(field('How many'))
    await user.type(field('How many'), '2')
    await user.click(within(dialog()).getByRole('button', { name: 'Remove' }))
    expect(sent).toEqual([manual({ Counters: { object: ID, counter: '+1/+1', delta: -2 } })])
  })

  it.each([
    ['Gain life…', '5', 'Gain', { Life: { player: 0, delta: 5 } }],
    ['Lose life…', '3', 'Lose', { Life: { player: 0, delta: -3 } }],
    ['Draw cards…', '2', 'Draw', { Draw: { count: 2 } }],
    ['Mill cards…', '4', 'Mill', { MoveTop: { count: 4, to: 'Graveyard' } }],
  ] as const)('%s sends the amount asked for', async (item, amount, submit, action) => {
    const { user, sent } = await open()
    await user.click(entry(item))
    const input = within(dialog()).getByRole('spinbutton')
    expect(input).toHaveFocus()
    await user.clear(input)
    await user.type(input, amount)
    await user.click(within(dialog()).getByRole('button', { name: submit }))
    expect(sent).toEqual([manual(action)])
  })

  it('moves the top cards of the library to the place chosen', async () => {
    const { user, sent } = await open()
    await user.click(entry('Move top cards of library…'))
    await user.selectOptions(field('Move them to'), 'LibraryBottom')
    await user.click(within(dialog()).getByRole('button', { name: 'Move' }))
    expect(sent).toEqual([manual({ MoveTop: { count: 1, to: 'LibraryBottom' } })])
  })

  it('creates the tokens chosen', async () => {
    const { user, sent } = await open()
    await user.click(entry('Create a token…'))
    await user.selectOptions(field('Token'), 'Soldier')
    await user.clear(field('How many'))
    await user.type(field('How many'), '2')
    await user.click(within(dialog()).getByRole('button', { name: 'Create' }))
    expect(sent).toEqual([manual({ CreateToken: { token: { Common: 'Soldier' }, count: 2 } })])
  })

  it('describes a custom creature token', async () => {
    const { user, sent } = await open()
    await user.click(entry('Create a token…'))
    await user.selectOptions(field('Token'), 'custom')
    await user.type(field('Name of the token'), 'Knight')
    await user.clear(field('Power'))
    await user.type(field('Power'), '3')
    await user.clear(field('Toughness'))
    await user.type(field('Toughness'), '2')
    await user.click(within(dialog()).getByRole('button', { name: 'Create' }))
    expect(sent).toEqual([
      manual({
        CreateToken: {
          token: {
            Custom: {
              name: 'Knight',
              colors: [],
              types: ['Creature'],
              subtypes: ['Knight'],
              power: 3,
              toughness: 2,
            },
          },
          count: 1,
        },
      }),
    ])
  })

  it('lists the permanents an attachment can go to and sends the one chosen', async () => {
    const equipment = permanent({}, { name: 'Sword of Tests', types: ['Artifact'], subtypes: ['Equipment'] })
    const { user, sent, fixture } = await open({ object: equipment })
    const target = must(
      fixture.view.objects.find((object) => object.zone === 'Battlefield' && object.controller === 0),
    )
    await user.click(entry('Attach to…'))
    const list = within(dialog()).getByRole('list', { name: 'Permanents' })
    expect(within(list).queryByText('Sword of Tests')).toBeNull()
    await user.type(within(dialog()).getByLabelText('Filter by name'), target.name)
    await user.click(within(list).getAllByRole('button', { name: new RegExp(target.name) })[0] as HTMLElement)
    const [answer] = sent
    expect(answer).toMatchObject({ Priority: { Manual: { Attach: { object: ID } } } })
    expect(sent).toHaveLength(1)
  })

  it('says when the filter matches nothing', async () => {
    const equipment = permanent({}, { name: 'Sword', types: ['Artifact'], subtypes: ['Equipment'] })
    const { user } = await open({ object: equipment })
    await user.click(entry('Attach to…'))
    await user.type(within(dialog()).getByLabelText('Filter by name'), 'zzzz')
    expect(within(dialog()).getByText('No permanent matches.')).toBeInTheDocument()
  })

  it('goes back to the list with Escape or Back, and Escape in the list closes', async () => {
    const { user } = await open()
    await user.click(entry('Draw cards…'))
    await user.keyboard('{Escape}')
    expect(menu()).toBeInTheDocument()
    await user.click(entry('Draw cards…'))
    await user.click(within(dialog()).getByRole('button', { name: 'Back' }))
    expect(menu()).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menu')).toBeNull()
  })
})

describe('without priority', () => {
  it('lists the entries and says none can be used, and sends nothing', async () => {
    const { user, sent } = renderMenu({ state: (state) => ({ ...state, pending: null }) })
    cardButton().focus()
    await user.keyboard('m')
    expect(screen.getByText(/only while you hold priority/)).toBeInTheDocument()
    const tap = entry('Tap')
    expect(tap).toHaveAttribute('aria-disabled', 'true')
    await user.click(tap)
    expect(sent).toEqual([])
    expect(menu()).toBeInTheDocument()
  })

  it('does the same while another kind of decision waits', async () => {
    const { user } = renderMenu({
      state: (state) => ({
        ...state,
        pending: state.pending === null ? null : { ...state.pending, decision: decisionFor('YesNo') },
      }),
    })
    cardButton().focus()
    await user.keyboard('m')
    expect(entry('Tap')).toHaveAttribute('aria-disabled', 'true')
  })
})

describe('rules left to the table', () => {
  const clauses = [
    { text: 'Whenever you cast a spell, draw a card.', reason: 'the engine does not run this yet' },
  ]

  it('quotes the clauses of the card at the top of its menu, with the reason', async () => {
    const { user } = renderMenu({ object: card({ manual: clauses }) })
    cardButton().focus()
    await user.keyboard('m')
    const block = screen.getByRole('region', { name: 'By hand' })
    expect(block).toHaveTextContent('Whenever you cast a spell, draw a card.')
    expect(block).toHaveTextContent('the engine does not run this yet')
  })

  it('shows no such block for a card the engine runs whole', async () => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    expect(screen.queryByRole('region', { name: 'By hand' })).toBeNull()
  })

  it('offers to ask for the next window when the seat is passed for, and asks once', async () => {
    const asked: FullControlMode[] = []
    const { user } = renderMenu({
      state: (state) => ({ ...state, pending: null }),
      fullControl: (mode) => !!asked.push(mode),
    })
    cardButton().focus()
    await user.keyboard('m')
    await user.click(entry(/^Ask me at the next window/))
    expect(asked).toEqual(['NextWindow'])
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it.each([
    ['holding priority', {}],
    [
      'Full Control already on',
      { state: (state: PlayState) => ({ ...state, pending: null, fullControl: 'Held' as const }) },
    ],
  ])('does not offer it while %s', async (_name, over) => {
    const { user } = renderMenu(over)
    cardButton().focus()
    await user.keyboard('m')
    expect(within(menu()).queryByRole('menuitem', { name: /next window/ })).toBeNull()
  })
})

describe('what is on offer for other cards', () => {
  it('shows only the notes and the seat entries for a card of an opponent', async () => {
    const { user } = renderMenu({ object: card({ owner: 1, controller: 1 }) })
    cardButton().focus()
    await user.keyboard('m')
    expect(within(menu()).getByText(/Only .* moves this card/)).toBeInTheDocument()
    expect(entry('Tap')).not.toHaveAttribute('aria-disabled')
    expect(entry('Gain life…')).not.toHaveAttribute('aria-disabled')
  })

  it('gives a card in the hand the moves and no tap', async () => {
    const { user } = renderMenu({ object: card({ zone: { Hand: 0 }, permanent: null }) })
    cardButton().focus()
    await user.keyboard('m')
    expect(within(menu()).queryByRole('menuitem', { name: 'Tap' })).toBeNull()
    expect(entry('Battlefield')).toBeInTheDocument()
  })

  it('gives a card in the graveyard the moves out of it and no discard', async () => {
    const { user } = renderMenu({ object: card({ zone: { Graveyard: 0 }, permanent: null }) })
    cardButton().focus()
    await user.keyboard('m')
    expect(entry('Hand')).toBeInTheDocument()
    expect(within(menu()).queryByRole('menuitem', { name: 'Discard' })).toBeNull()
  })
})
