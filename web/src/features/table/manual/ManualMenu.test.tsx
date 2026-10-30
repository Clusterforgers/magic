// Goal: a card opens its manual menu by right click, long press and keyboard, lists the entries by group, sends the exact Manual message for each choice and each filled form at any time, closes with the focus back on the card, and says why a seat that lost can do nothing.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Answer, ManualAction, ObjectView } from '@/protocol/generated'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import { card, permanent } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from '../Card'
import { buildFixture } from '../dev/fixture'
import { ManualMenuProvider } from './ManualMenu'
import type { ManualResult } from '@/play/manualLedger'
import { usePick } from './panel/pick'
import { LONG_PRESS_MS } from './useManualMenu'

/** Says where the table's pick is, for a test to read. */
function PickProbe() {
  const pick = usePick()
  return <span data-testid="pick">{pick.phase === 'picked' ? `picked ${pick.card}` : pick.phase}</span>
}

const ID = 900

interface Shown {
  object?: ObjectView
  /** What the session's manual sender answers; true is taken as a yes from the server. */
  manual?: (action: ManualAction) => boolean
  /** What the server says to a sent action: yes, a refusal with its reason, or nothing yet. */
  answer?: ManualResult | 'silence'
  /** Changes the play state before the first render. */
  state?: (state: PlayState) => PlayState
  spectator?: boolean
}

/** One card (object 900) among the few permanents of a small fixture, inside the manual menu's provider. */
function renderMenu({
  object = card(),
  manual = () => true,
  answer = { ok: true },
  state,
  spectator,
}: Shown = {}) {
  const sent: ManualAction[] = []
  const answered: Answer[] = []
  const table = renderTable({
    fixture: buildFixture({ seats: 2, permanents: 4, hand: 2 }),
    ui: (
      <ManualMenuProvider>
        <Card id={ID} />
        <PickProbe />
      </ManualMenuProvider>
    ),
    session: {
      manual: (action, reply) => {
        sent.push(action)
        const taken = manual(action)
        if (taken && answer !== 'silence') reply?.(answer)
        return taken
      },
      answer: (given) => !!answered.push(given),
    },
    state: (first) => {
      const view = clone(must(first.view))
      view.objects = [...view.objects, { ...object, id: ID }]
      if (spectator === true) view.spectator = true
      const next = { ...first, view, objectsById: indexById(view.objects) }
      return state === undefined ? next : state(next)
    },
  })
  return { ...table, sent, answered }
}

const cardButton = (): HTMLElement =>
  document.querySelector<HTMLElement>(`button[data-object-id="${ID}"]`) as HTMLElement
const menu = (): HTMLElement => screen.getByRole('menu')
const entry = (name: string | RegExp): HTMLElement => within(menu()).getByRole('menuitem', { name })

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
    expect(groups).toEqual(['move', 'counters', 'tap', 'effect', 'show', 'token', 'panel'])
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
    expect(sent).toEqual([action])
    expect(screen.queryByRole('menu')).toBeNull()
    expect(cardButton()).toHaveFocus()
  })

  it('sends an untap for a tapped permanent, and a discard from the hand', async () => {
    const tapped = renderMenu({ object: permanent({ tapped: true }) })
    cardButton().focus()
    await tapped.user.keyboard('m')
    await tapped.user.click(entry('Untap'))
    expect(tapped.sent).toEqual([{ Untap: { object: ID } }])
  })

  it('sends a discard for a card in the hand', async () => {
    const { user, sent } = renderMenu({ object: card({ zone: { Hand: 0 }, permanent: null }) })
    cardButton().focus()
    await user.keyboard('m')
    await user.click(entry('Discard'))
    expect(sent).toEqual([{ Discard: { object: ID } }])
  })

  it('keeps the menu and says so when the connection refuses the action', async () => {
    const { user } = renderMenu({ manual: () => false })
    cardButton().focus()
    await user.keyboard('m')
    await user.click(entry('Tap'))
    expect(screen.getByRole('alert')).toHaveTextContent(/connection is not open/)
    expect(menu()).toBeInTheDocument()
  })

  it('stays usable when the decision on screen goes away while it is open', async () => {
    const { user, sent, update } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    update(() => ({ pending: null }))
    await user.click(entry('Tap'))
    expect(sent).toEqual([{ Tap: { object: ID } }])
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
    expect(sent).toEqual([{ Counters: { object: ID, counter: 'charge', delta: 4 } }])
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('names a counter of another kind', async () => {
    const { user, sent } = await open()
    await user.click(entry('Add counters…'))
    await user.selectOptions(field('Kind of counter'), 'other')
    await user.type(field('Name of the counter'), 'oil')
    await user.click(within(dialog()).getByRole('button', { name: 'Add' }))
    expect(sent).toEqual([{ Counters: { object: ID, counter: { Other: 'oil' }, delta: 1 } }])
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
    expect(sent).toEqual([{ Counters: { object: ID, counter: '+1/+1', delta: -2 } }])
  })

  it('creates the tokens chosen', async () => {
    const { user, sent } = await open()
    await user.click(entry('Create a token…'))
    await user.selectOptions(field('Token'), 'Soldier')
    await user.clear(field('How many'))
    await user.type(field('How many'), '2')
    await user.click(within(dialog()).getByRole('button', { name: 'Create' }))
    expect(sent).toEqual([{ CreateToken: { token: { Common: 'Soldier' }, count: 2 } }])
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
      {
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
      },
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
    const [action] = sent
    expect(action).toMatchObject({ Attach: { object: ID } })
    expect(sent).toHaveLength(1)
  })

  it('deals the damage asked for to the player or permanent chosen', async () => {
    const { user, sent } = await open()
    await user.click(entry('Deal damage…'))
    expect(dialog()).toHaveAccessibleName('Deal damage with Grizzly Bears')
    expect(within(dialog()).getByRole('button', { name: 'Deal' })).toBeDisabled()
    const to = field('Deal it to')
    expect(to).toHaveFocus()
    expect(within(to).getByRole('option', { name: 'Serra Angel (Bo)' })).toBeInTheDocument()
    await user.selectOptions(to, 'player:1')
    await user.clear(field('Damage'))
    await user.type(field('Damage'), '3')
    await user.click(within(dialog()).getByRole('button', { name: 'Deal' }))
    expect(sent).toEqual([{ Damage: { source: ID, target: { Player: 1 }, amount: 3 } }])
  })

  it('pumps a creature, also one of an opponent, and refuses a change of nothing', async () => {
    const { user, sent } = await open({ object: card({ owner: 1, controller: 1 }) })
    await user.click(entry(/^Pump…/))
    await user.clear(field('Power change'))
    await user.type(field('Power change'), '0')
    await user.clear(field('Toughness change'))
    await user.type(field('Toughness change'), '0')
    expect(within(dialog()).getByRole('button', { name: 'Pump' })).toBeDisabled()
    await user.clear(field('Power change'))
    await user.type(field('Power change'), '-2')
    await user.clear(field('Toughness change'))
    await user.type(field('Toughness change'), '3')
    await user.click(within(dialog()).getByRole('button', { name: 'Pump' }))
    expect(sent).toEqual([{ Pump: { object: ID, power: -2, toughness: 3 } }])
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
    await user.click(entry('Add counters…'))
    await user.keyboard('{Escape}')
    expect(menu()).toBeInTheDocument()
    await user.click(entry('Add counters…'))
    await user.click(within(dialog()).getByRole('button', { name: 'Back' }))
    expect(menu()).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menu')).toBeNull()
  })
})

describe('at any time', () => {
  it.each([
    ['with no decision on screen', (state: PlayState): PlayState => ({ ...state, pending: null })],
    [
      'while a decision of another kind waits',
      (state: PlayState): PlayState => ({
        ...state,
        pending: state.pending === null ? null : { ...state.pending, decision: decisionFor('YesNo') },
      }),
    ],
  ])('the entries can be used %s and send a Manual message, not an answer', async (_name, state) => {
    const { user, sent, answered } = renderMenu({ state })
    cardButton().focus()
    await user.keyboard('m')
    expect(entry('Tap')).not.toHaveAttribute('aria-disabled')
    await user.click(entry('Tap'))
    expect(sent).toEqual([{ Tap: { object: ID } }])
    expect(answered).toEqual([])
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('has no entry that asks for the next window', async () => {
    const { user } = renderMenu({ state: (state) => ({ ...state, pending: null }) })
    cardButton().focus()
    await user.keyboard('m')
    expect(within(menu()).queryByRole('menuitem', { name: /next window/ })).toBeNull()
  })

  it('lists the entries inert, with the reason, for a seat that lost', async () => {
    const { user, sent } = renderMenu({
      state: (state) => {
        const view = clone(must(state.view))
        view.players = view.players.map((player) =>
          player.seat === 0 ? { ...player, lost: 'ZeroLife' } : player,
        )
        return { ...state, view }
      },
    })
    cardButton().focus()
    await user.keyboard('m')
    expect(screen.getByText(/left the game/)).toBeInTheDocument()
    expect(entry('Tap')).toHaveAttribute('aria-disabled', 'true')
    await user.click(entry('Tap'))
    expect(sent).toEqual([])
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
})

describe('what is on offer for other cards', () => {
  it('shows only the notes, tapping and the way into the panel for a card of an opponent', async () => {
    const { user } = renderMenu({ object: card({ owner: 1, controller: 1 }) })
    cardButton().focus()
    await user.keyboard('m')
    expect(within(menu()).getByText(/Only .* moves this card/)).toBeInTheDocument()
    expect(entry('Tap')).not.toHaveAttribute('aria-disabled')
    expect(entry(/^In the panel/)).not.toHaveAttribute('aria-disabled')
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

describe('the way into the panel', () => {
  it('is the last entry, picks this card for the Commands panel and closes the menu', async () => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    const items = within(menu()).getAllByRole('menuitem')
    expect(items.at(-1)).toHaveAccessibleName(/In the panel/)
    await user.click(entry(/^In the panel/))
    expect(screen.getByTestId('pick')).toHaveTextContent(`picked ${ID}`)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(cardButton()).toHaveFocus()
  })

  it.each([
    'Draw cards…',
    'Mill cards…',
    'Move top cards of library…',
    'Gain life…',
    'Lose life…',
    'Add mana…',
  ])('no longer lists %s, which the panel has', async (name) => {
    const { user } = renderMenu()
    cardButton().focus()
    await user.keyboard('m')
    expect(within(menu()).queryByRole('menuitem', { name })).toBeNull()
  })
})

describe('the server answers an action of the menu', () => {
  const tap = async (over: Shown) => {
    const harness = renderMenu(over)
    cardButton().focus()
    await harness.user.keyboard('m')
    await harness.user.click(entry('Tap'))
    return harness
  }

  it('closes the menu when the server took it', async () => {
    const { sent } = await tap({})
    expect(sent).toEqual([{ Tap: { object: ID } }])
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('keeps the menu up with "Sending" until the server answers', async () => {
    await tap({ answer: 'silence' })
    expect(menu()).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent(/^Sending: /)
  })

  it('shows a refusal in the menu with its reason, keeps the menu and leaves the table notice alone', async () => {
    const { store } = await tap({ answer: { ok: false, reason: 'Grizzly Bears is already tapped' } })
    expect(screen.getByRole('alert')).toHaveTextContent('Refused: Grizzly Bears is already tapped.')
    expect(menu()).toBeInTheDocument()
    expect(store.getState().notice).toBeNull()
  })

  it('clears the refusal when another entry is used', async () => {
    const { user } = await tap({ answer: { ok: false, reason: 'no' } })
    await user.click(entry('Add counters…'))
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('says a connection that went was not confirmed', async () => {
    await tap({ answer: { ok: false, reason: 'the connection was interrupted.', lost: true } })
    expect(screen.getByRole('alert')).toHaveTextContent('Not confirmed: the connection was interrupted.')
  })
})
