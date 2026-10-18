// Goal: on the real table, a batch decorates exactly what it changed (a lunge and glow on attackers and blockers, a flash and "-N" on damaged creatures and on a life total that drops, "+N" on one that rises, a ghost where a permanent died, a pop on one that entered) while the table already shows the true state, the decoration ends on its own timer, nothing is announced twice or in the way of a click, reduced motion keeps only still highlights, and with the layer off (the test default) nothing is added at all.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { applyBatchToStore } from '@/features/table/dev/applyBatch'
import type { TableOptions } from '@/features/table/tableOptions'
import type { Answer, Event, EventView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import {
  batchFrom,
  FOE,
  FOE_BEAR,
  FOE_GOLEM,
  FOE_IMP,
  FOE_SPELL,
  ME,
  miniFixture,
  MY_BEAR,
  MY_GIANT,
  MY_WALL,
  pub,
} from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import { primaryButton } from '@/test-utils/passControls'
import { renderTable } from '@/test-utils/tableHarness'
import { FX_MS } from './model'
import { EFFECTS_BY_DEFAULT } from './useTableFx'

const PASS: EventView[] = pub({ PriorityPassed: { player: ME } })

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

/** The table over the mini game with the decoration layer on (or as `options` say), and a way to send it batches. */
function mount(options: TableOptions = {}, answer?: (answer: Answer) => boolean) {
  const table = renderTable({
    fixture: miniFixture(),
    options: { effects: true, ...options },
    session: answer === undefined ? {} : { answer },
  })
  const send = (change: (draft: Draft) => void, events: EventView[] = PASS): void => {
    const { view, seq } = table.store.getState()
    act(() => {
      applyBatchToStore(table.store, batchFrom(must(view), change, events, must(seq)))
    })
  }
  return { ...table, send }
}

const advance = (ms: number): void => {
  act(() => {
    vi.advanceTimersByTime(ms)
  })
}

const cardOf = (container: HTMLElement, id: number): HTMLElement =>
  must(container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`), `card ${id}`)
const lifeOf = (container: HTMLElement, seat: number): HTMLElement =>
  must(
    container.querySelector<HTMLElement>(`section[data-seat="${seat}"] [data-slot="life"]`),
    `life ${seat}`,
  )
const floats = (element: HTMLElement): string[] =>
  [...element.querySelectorAll('[data-slot="fx-float"]')].map((node) => node.textContent ?? '')
const decorated = (container: HTMLElement): HTMLElement[] => [
  ...container.querySelectorAll<HTMLElement>('[data-fx]'),
]
const ghosts = (container: HTMLElement): HTMLElement[] => [
  ...container.querySelectorAll<HTMLElement>('li[data-ghost]'),
]

const hit = (source: number, target: number, amount: number): Event => ({
  DamageDealt: { source, target: { Object: target }, amount },
})
const died = (object: number): Event => ({ CreatureDied: { object, cause: 'LethalDamage' } })
const hurt = (id: number, damage: number) => (draft: Draft) =>
  draft.edit(id, (object) => {
    must(object.permanent).damage = damage
  })

function stubReducedMotion(initial: boolean): (next: boolean) => void {
  let matches = initial
  vi.stubGlobal('matchMedia', (query: string) => ({
    media: query,
    get matches() {
      return matches
    },
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
  return (next) => {
    matches = next
  }
}

describe('the switch', () => {
  it('is off by default under vitest, so every other test sees the table as the game says it', () => {
    expect(EFFECTS_BY_DEFAULT).toBe(false)
    const { container, store } = renderTable({ fixture: miniFixture() })
    act(() => {
      applyBatchToStore(
        store,
        batchFrom(
          must(store.getState().view),
          (draft) => {
            hurt(MY_GIANT, 2)(draft)
            draft.die(MY_BEAR)
            draft.life(FOE, 12)
          },
          pub(hit(FOE_GOLEM, MY_GIANT, 2), died(MY_BEAR)),
          0,
        ),
      )
    })
    expect(decorated(container)).toEqual([])
    expect(ghosts(container)).toEqual([])
    expect(container.querySelector('[data-slot="fx-float"]')).toBeNull()
    expect(cardOf(container, MY_GIANT)).toHaveAccessibleName(/2 damage/)
  })

  it('is off when the table is told so, even outside vitest’s default', () => {
    const { container, send } = mount({ effects: false })
    send(hurt(MY_GIANT, 2), pub(hit(FOE_GOLEM, MY_GIANT, 2)))
    expect(decorated(container)).toEqual([])
  })

  it('is on when the table is told so', () => {
    const { container, send } = mount()
    send(hurt(MY_GIANT, 2), pub(hit(FOE_GOLEM, MY_GIANT, 2)))
    expect(decorated(container)).toHaveLength(1)
  })

  it('adds nothing to a table nothing has happened to', () => {
    const { container } = mount()
    expect(decorated(container)).toEqual([])
    expect(container.querySelector('[data-slot="fx-float"]')).toBeNull()
    expect(ghosts(container)).toEqual([])
  })
})

describe('damage to a creature', () => {
  it('flashes the card and floats the amount, while the card already says the damage in words', () => {
    const { container, send } = mount()
    send(hurt(MY_GIANT, 2), pub(hit(FOE_GOLEM, MY_GIANT, 2)))
    const giant = cardOf(container, MY_GIANT)
    expect(giant).toHaveAttribute('data-fx', 'damaged')
    expect(floats(giant)).toEqual(['-2'])
    expect(giant).toHaveAccessibleName(/2 damage/)
    expect(within(giant).getByText('2 dmg')).toBeInTheDocument()
    expect(decorated(container)).toEqual([giant])
  })

  it('ends by itself and leaves the card as the game has it', () => {
    const { container, send } = mount()
    send(hurt(MY_GIANT, 2), pub(hit(FOE_GOLEM, MY_GIANT, 2)))
    advance(FX_MS.damaged - 1)
    expect(cardOf(container, MY_GIANT)).toHaveAttribute('data-fx', 'damaged')
    advance(1)
    const giant = cardOf(container, MY_GIANT)
    expect(giant).not.toHaveAttribute('data-fx')
    expect(floats(giant)).toEqual([])
    expect(giant).toHaveAccessibleName(/2 damage/)
    expect(vi.getTimerCount()).toBe(0)
  })

  it('restarts the float for a second hit while the first still plays', () => {
    const { container, send } = mount()
    send(hurt(MY_WALL, 1), pub(hit(FOE_IMP, MY_WALL, 1)))
    advance(200)
    send(hurt(MY_WALL, 3), pub(hit(FOE_GOLEM, MY_WALL, 2)))
    expect(floats(cardOf(container, MY_WALL))).toEqual(['-2'])
    advance(FX_MS.damaged - 200)
    expect(cardOf(container, MY_WALL)).toHaveAttribute('data-fx', 'damaged')
    advance(200)
    expect(cardOf(container, MY_WALL)).not.toHaveAttribute('data-fx')
  })
})

describe('attacking and blocking', () => {
  const attack = (ids: number[], target: number) => (draft: Draft) => {
    draft.view.combat = {
      attackers: ids.map((object) => ({ object, target: { Player: target }, blocked: false, blockers: [] })),
      attackers_declared: true,
      blockers_declared: [],
    }
    for (const id of ids) {
      draft.edit(id, (object) => {
        must(object.permanent).tapped = true
      })
    }
  }
  const declared = (player: number, ids: number[], target: number): EventView[] =>
    pub({ AttackersDeclared: { player, attackers: ids.map((id) => [id, { Player: target }]) } })

  it('makes an attacker lean up the screen from the viewer’s side and glow, while it already reads “attacking”', () => {
    const { container, send } = mount()
    send(attack([MY_BEAR, MY_GIANT], FOE), declared(ME, [MY_BEAR, MY_GIANT], FOE))
    for (const id of [MY_BEAR, MY_GIANT]) {
      const attacker = cardOf(container, id)
      expect(attacker).toHaveAttribute('data-fx', 'attacked')
      expect(attacker).toHaveAttribute('data-fx-side', 'own')
      expect(attacker).toHaveAccessibleName(/attacking/)
    }
    expect(decorated(container)).toHaveLength(2)
  })

  it('makes an opponent’s attacker lean down the screen', () => {
    const { container, send } = mount()
    send(attack([FOE_BEAR], ME), declared(FOE, [FOE_BEAR], ME))
    const attacker = cardOf(container, FOE_BEAR)
    expect(attacker).toHaveAttribute('data-fx', 'attacked')
    expect(attacker).toHaveAttribute('data-fx-side', 'other')
  })

  it('makes a blocker lean toward the attacker', () => {
    const { container, send } = mount()
    send(attack([FOE_BEAR, FOE_GOLEM], ME), declared(FOE, [FOE_BEAR, FOE_GOLEM], ME))
    advance(FX_MS.attacked)
    send(
      (draft) => {
        draft.view.combat = {
          attackers: [
            { object: FOE_BEAR, target: { Player: ME }, blocked: true, blockers: [MY_GIANT] },
            { object: FOE_GOLEM, target: { Player: ME }, blocked: false, blockers: [] },
          ],
          attackers_declared: true,
          blockers_declared: [ME],
        }
      },
      pub({ BlockersDeclared: { player: ME, blocks: [[MY_GIANT, FOE_BEAR]] } }),
    )
    const blocker = cardOf(container, MY_GIANT)
    expect(blocker).toHaveAttribute('data-fx', 'blocked')
    expect(blocker).toHaveAttribute('data-fx-side', 'own')
    expect(blocker).toHaveAccessibleName(/blocking/)
    expect(decorated(container)).toEqual([blocker])
  })

  it('ends on its own', () => {
    const { container, send } = mount()
    send(attack([MY_BEAR], FOE), declared(ME, [MY_BEAR], FOE))
    advance(FX_MS.attacked)
    expect(cardOf(container, MY_BEAR)).not.toHaveAttribute('data-fx')
    expect(cardOf(container, MY_BEAR)).not.toHaveAttribute('data-fx-side')
  })
})

describe('a life total', () => {
  it('flashes and floats “-N” when it drops, showing the new total at once', () => {
    const { container, send } = mount()
    send((draft) => draft.life(FOE, 15))
    const life = lifeOf(container, FOE)
    expect(life).toHaveAttribute('data-fx', 'damaged')
    expect(floats(life)).toEqual(['-5'])
    expect(life).toHaveTextContent('15')
    expect(screen.getByRole('region', { name: 'Bo' })).toHaveTextContent('Life15')
  })

  it('flashes and floats “+N” when it rises', () => {
    const { container, send } = mount()
    send((draft) => draft.life(ME, 24))
    const life = lifeOf(container, ME)
    expect(life).toHaveAttribute('data-fx', 'healed')
    expect(floats(life)).toEqual(['+4'])
    expect(life).toHaveTextContent('24')
  })

  it('shows both seats of one batch, each on its own total', () => {
    const { container, send } = mount()
    send((draft) => {
      draft.life(ME, 22)
      draft.life(FOE, 17)
    })
    expect(lifeOf(container, ME)).toHaveAttribute('data-fx', 'healed')
    expect(lifeOf(container, FOE)).toHaveAttribute('data-fx', 'damaged')
  })

  it('ends on its own', () => {
    const { container, send } = mount()
    send((draft) => draft.life(FOE, 15))
    advance(FX_MS.damaged)
    expect(lifeOf(container, FOE)).not.toHaveAttribute('data-fx')
    expect(floats(lifeOf(container, FOE))).toEqual([])
    expect(lifeOf(container, FOE)).toHaveTextContent('15')
  })

  it('also decorates the life on an opponent’s tile in focus mode, whose spoken label keeps saying the true life', () => {
    const { container, send } = mount({ mode: 'focus' })
    send((draft) => draft.life(FOE, 12))
    const tile = must(container.querySelector<HTMLElement>('[data-tile-seat="1"]'))
    expect(within(tile).getByText('12').closest('[data-slot="life"]')).toHaveAttribute('data-fx', 'damaged')
    expect(floats(tile)).toEqual(['-8'])
    expect(tile).toHaveAccessibleName(/Life 12/)
  })

  it('shows the elimination as the life drop it is, the vanished permanents drawing nothing', () => {
    const { container, send } = mount()
    send((draft) => draft.life(FOE, 1))
    advance(FX_MS.damaged)
    send(
      (draft) => {
        draft.life(FOE, 0)
        must(draft.view.players[FOE]).lost = 'ZeroLife'
        for (const id of [11, 12, 13, 14, 15]) draft.leave(id)
      },
      pub({ PlayerLost: { player: FOE, reason: 'ZeroLife' } }),
    )
    expect(lifeOf(container, FOE)).toHaveAttribute('data-fx', 'damaged')
    expect(ghosts(container)).toEqual([])
    expect(decorated(container)).toEqual([lifeOf(container, FOE)])
  })
})

describe('a permanent that dies', () => {
  const lethal = pub(hit(FOE_GOLEM, MY_BEAR, 4), died(MY_BEAR))
  const kill = (draft: Draft): void => draft.die(MY_BEAR)
  const myCreatures = (): HTMLElement => screen.getByRole('list', { name: "Alice's creatures" })

  it('leaves a ghost where it stood, while the game already has it in the graveyard', () => {
    const { container, send, store } = mount()
    send(kill, lethal)
    const row = myCreatures()
    expect(ghosts(container)).toHaveLength(1)
    const [ghost] = ghosts(container)
    expect(ghost).toHaveAttribute('data-ghost', String(MY_BEAR))
    expect(ghost).toHaveAttribute('data-fx', 'died')
    // The ghost stands first, in the old place, and the real cards follow it.
    expect(
      [...row.children].map(
        (child) =>
          child.getAttribute('data-ghost') ??
          child.querySelector('[data-object-id]')?.getAttribute('data-object-id'),
      ),
    ).toEqual([String(MY_BEAR), String(MY_GIANT), String(MY_WALL)])
    expect(within(row).queryByRole('button', { name: /Ash Bear/ })).toBeNull()
    // The true state: gone from the battlefield, in the graveyard, at once.
    expect(must(store.getState().view).battlefield.cards).not.toContain(MY_BEAR)
    expect(store.getState().objectsById[MY_BEAR]?.zone).toEqual({ Graveyard: ME })
    const mine = within(screen.getByRole('region', { name: 'Alice (you)' }))
    expect(mine.getByRole('button', { name: /^Graveyard 1/ })).toBeEnabled()
  })

  it('is never a second copy for a screen reader, the keyboard or a lookup of cards', () => {
    const { container, send } = mount()
    send(kill, lethal)
    const [ghost] = ghosts(container)
    expect(ghost).toHaveAttribute('aria-hidden', 'true')
    expect(ghost?.querySelector('[data-object-id], button, a, input, [tabindex]')).toBeNull()
    expect(within(myCreatures()).getAllByRole('listitem')).toHaveLength(2)
    expect(container.querySelectorAll(`[data-object-id="${MY_BEAR}"]`)).toHaveLength(1)
    expect(cardOf(container, MY_BEAR).closest('[data-battlefield-seat]')).toBeNull()
    expect(screen.queryAllByRole('button', { name: /Ash Bear/ })).toHaveLength(1)
  })

  it('floats the damage that killed it', () => {
    const { container, send } = mount()
    send(kill, lethal)
    expect(floats(must(ghosts(container)[0]))).toEqual(['-4'])
  })

  it('closes up when its time is over, leaving exactly the real cards', () => {
    const { container, send } = mount()
    send(kill, lethal)
    advance(FX_MS.died - 1)
    expect(ghosts(container)).toHaveLength(1)
    advance(1)
    expect(ghosts(container)).toEqual([])
    expect(
      [...myCreatures().children].map((child) =>
        child.querySelector('[data-object-id]')?.getAttribute('data-object-id'),
      ),
    ).toEqual([String(MY_GIANT), String(MY_WALL)])
    expect(vi.getTimerCount()).toBe(0)
  })

  it('stands in the opponent’s row for the opponent’s creature', () => {
    const { container, send } = mount()
    send((draft) => draft.die(FOE_GOLEM), pub(hit(MY_GIANT, FOE_GOLEM, 4), died(FOE_GOLEM)))
    const row = screen.getByRole('list', { name: "Bo's creatures" })
    expect(row).toContainElement(ghosts(container)[0] ?? null)
    expect(
      [...row.children].map(
        (child) =>
          child.getAttribute('data-ghost') ??
          child.querySelector('[data-object-id]')?.getAttribute('data-object-id'),
      ),
    ).toEqual([String(FOE_BEAR), String(FOE_GOLEM), String(FOE_IMP)])
  })

  it('draws a ghost for a row whose last creature died, and tells a screen reader of no list', () => {
    const { container, send } = mount()
    send(
      (draft) => {
        for (const id of [MY_BEAR, MY_GIANT, MY_WALL]) draft.die(id)
      },
      pub(died(MY_BEAR), died(MY_GIANT), died(MY_WALL)),
    )
    expect(ghosts(container)).toHaveLength(3)
    const row = must(ghosts(container)[0]?.closest('ul'))
    expect(row).toHaveAttribute('aria-hidden', 'true')
    advance(FX_MS.died)
    expect(container.querySelector(`ul[aria-label="Alice's creatures"]`)).toBeNull()
  })

  it('does not move the keyboard’s one tab stop in the row or take focus', () => {
    const { container, send } = mount()
    cardOf(container, MY_GIANT).focus()
    send(kill, lethal)
    expect(cardOf(container, MY_GIANT)).toHaveFocus()
    const stops = within(myCreatures())
      .getAllByRole('button')
      .filter((button) => button.getAttribute('tabindex') === '0')
    expect(stops).toEqual([cardOf(container, MY_GIANT)])
  })

  it('keeps the dead card in the graveyard whatever the ghost does', () => {
    const { container, send, store } = mount()
    send(kill, lethal)
    advance(FX_MS.died / 2)
    expect(store.getState().objectsById[MY_BEAR]?.zone).toEqual({ Graveyard: ME })
    advance(FX_MS.died)
    expect(store.getState().objectsById[MY_BEAR]?.zone).toEqual({ Graveyard: ME })
    expect(cardOf(container, MY_BEAR)).toHaveAccessibleName(/Ash Bear/)
  })
})

describe('a permanent that enters', () => {
  it('pops in, and the card is there to use from the first moment', () => {
    const { container, send } = mount()
    send((draft) => draft.enter(FOE_SPELL), pub({ SpellResolved: { object: FOE_SPELL } }))
    const entered = cardOf(container, FOE_SPELL)
    expect(entered).toHaveAttribute('data-fx', 'entered')
    expect(entered).toHaveAccessibleName(/Hill Giant/)
    advance(FX_MS.entered)
    expect(cardOf(container, FOE_SPELL)).not.toHaveAttribute('data-fx')
  })
})

describe('under reduced motion', () => {
  it('keeps the still highlights and draws no ghost and no entrance', () => {
    stubReducedMotion(true)
    const { container, send } = mount()
    send(
      (draft) => {
        hurt(MY_GIANT, 3)(draft)
        draft.die(MY_BEAR)
        draft.enter(FOE_SPELL)
        draft.life(FOE, 15)
      },
      pub(hit(FOE_GOLEM, MY_GIANT, 3), hit(FOE_GOLEM, MY_BEAR, 4), died(MY_BEAR)),
    )
    expect(cardOf(container, MY_GIANT)).toHaveAttribute('data-fx', 'damaged')
    expect(lifeOf(container, FOE)).toHaveAttribute('data-fx', 'damaged')
    expect(ghosts(container)).toEqual([])
    expect(cardOf(container, FOE_SPELL)).not.toHaveAttribute('data-fx')
    // The state itself is the same: the dead card is in the graveyard, the new one on the battlefield.
    const mine = within(screen.getByRole('region', { name: 'Alice (you)' }))
    expect(mine.getByRole('button', { name: /^Graveyard 1/ })).toBeEnabled()
  })

  it('follows a change of the setting from the next batch on', () => {
    const set = stubReducedMotion(true)
    const { container, send } = mount()
    send(hurt(MY_GIANT, 1), pub(hit(FOE_IMP, MY_GIANT, 1)))
    advance(FX_MS.damaged)
    set(false)
    send((draft) => draft.die(MY_BEAR), pub(died(MY_BEAR)))
    expect(ghosts(container)).toHaveLength(1)
    advance(FX_MS.died)
    set(true)
    send((draft) => draft.die(MY_WALL), pub(died(MY_WALL)))
    expect(ghosts(container)).toEqual([])
  })
})

describe('what the screen reader and the keyboard get', () => {
  const play = (draft: Draft): void => {
    hurt(MY_GIANT, 2)(draft)
    draft.die(FOE_BEAR)
    draft.enter(FOE_SPELL)
    draft.life(ME, 17)
    draft.life(FOE, 21)
  }
  const events = pub(hit(FOE_GOLEM, MY_GIANT, 2), died(FOE_BEAR))

  /** The text and the labels of everything that is not hidden from assistive technology. */
  const spoken = (effects: boolean): string => {
    const { container, send, unmount } = mount({ effects })
    send(play, events)
    const copy = container.cloneNode(true) as HTMLElement
    for (const hidden of copy.querySelectorAll('[aria-hidden="true"]')) hidden.remove()
    const labels = [...copy.querySelectorAll('[aria-label]')].map((node) => node.getAttribute('aria-label'))
    unmount()
    return [copy.textContent, ...labels].join('\n')
  }

  it('reads the same with the layer on and off', () => {
    expect(spoken(true)).toBe(spoken(false))
  })

  it('hides every piece of decoration from assistive technology', () => {
    const { container, send } = mount()
    send(play, events)
    const pieces = [...container.querySelectorAll('[data-slot="fx-float"], li[data-ghost], .fx-crack')]
    expect(pieces.length).toBeGreaterThan(2)
    for (const piece of pieces) expect(piece.closest('[aria-hidden="true"]'), piece.outerHTML).not.toBeNull()
  })
})

describe('never in the way of a decision', () => {
  it('answers a pass at once while a ghost and a flash still play', () => {
    const answer = vi.fn(() => true)
    const { container, send } = mount({}, answer)
    send(
      (draft) => {
        draft.die(MY_BEAR)
        draft.life(FOE, 14)
      },
      pub(hit(FOE_GOLEM, MY_BEAR, 4), died(MY_BEAR)),
    )
    expect(ghosts(container)).toHaveLength(1)
    fireEvent.click(primaryButton())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer).toHaveBeenCalledWith({ Priority: 'Pass' })
  })

  it('leaves a card that is playing an effect as clickable as any other', () => {
    const select = vi.fn()
    const { container, send } = mount()
    send(hurt(MY_GIANT, 2), pub(hit(FOE_GOLEM, MY_GIANT, 2)))
    const giant = cardOf(container, MY_GIANT)
    giant.addEventListener('click', select)
    fireEvent.click(giant)
    expect(select).toHaveBeenCalledTimes(1)
    expect(giant).toBeEnabled()
  })
})

describe('what is not play', () => {
  it('draws nothing for a resync that jumps ahead', () => {
    const { container, store } = mount()
    act(() => {
      const state = store.getState()
      const view = structuredClone(must(state.view))
      must(view.players[FOE]).life = 3
      store.setState({ view, seq: 50 })
    })
    expect(decorated(container)).toEqual([])
    expect(lifeOf(container, FOE)).toHaveTextContent('3')
  })

  it('draws nothing for a decision, a chat line or a status change', () => {
    const { container, store } = mount()
    act(() => store.setState({ notice: 'hello', status: 'reconnecting', chat: [{ from: FOE, text: 'hi' }] }))
    act(() => store.setState({ pending: null }))
    expect(decorated(container)).toEqual([])
  })
})

describe('leaving the table', () => {
  it('cancels every timer that is still running', () => {
    const { send, unmount } = mount()
    send((draft) => draft.die(MY_BEAR), pub(died(MY_BEAR)))
    expect(vi.getTimerCount()).toBeGreaterThan(0)
    unmount()
    expect(vi.getTimerCount()).toBe(0)
  })
})
