// Goal: over a real session on a fake socket, choosing blocks or attacks asks the engine what they would do after a quiet 80 ms and only for the latest choice, says "Checking outcome…" without changing the height of the section until the reply comes, draws the reply (doomed creatures, life after, lethal marks) and a sentence for a screen reader, drops every mark at once when the choice changes, ignores the reply to an outdated choice, falls back to "Outcome shown after you confirm." when the engine has none, when nothing comes for 1.5 s and when the server is too old to know previews, and forgets all of it when the decision is answered or replaced.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CombatForecast, ObjectId, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { DEBOUNCE_MS, GIVE_UP_MS } from '../combat/usePreviewRequest'
import { CHECKING_WORDS, FALLBACK_WORDS } from './PreviewOutcome'

const SEATS = 4
const fixture = fixtureOf(SEATS)
const [m1, m2, m3] = creaturesOf(fixture, 0) as [ObjectId, ObjectId, ObjectId]
const [t1, t2] = creaturesOf(fixture, 1) as [ObjectId, ObjectId]
const defenders = [{ Player: 1 }, { Player: 2 }, { Player: 3 }]

const attackers = (id: number, ids: ObjectId[]): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player: 0,
    kind: { DeclareAttackers: { attackers: ids.map((object) => ({ object, defenders })) } },
  },
})
const blockers = (id: number, ids: ObjectId[], canBlock: ObjectId[]): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player: 0,
    kind: { DeclareBlockers: { blockers: ids.map((object) => ({ object, can_block: canBlock })) } },
  },
})

/** What the engine might say: these creatures die and `seat` takes `damage` off `life`. Its numbers are made up here. */
function forecastOf({
  dies = [],
  seat = 0,
  damage = 0,
  life = 20,
}: {
  dies?: ObjectId[]
  seat?: number
  damage?: number
  life?: number
}): CombatForecast {
  return {
    attackers: [],
    creatures: dies.map((object) => ({ object, damage: 3, dies: true })),
    players: [{ player: seat, damage, life, life_after: life - damage, lethal: life - damage <= 0 }],
  }
}
const reply = (decision: number, forecast: CombatForecast | null): ServerMessage => ({
  type: 'Forecast',
  decision,
  forecast,
})

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  vi.useRealTimers()
})

/** A live table of seat 0 over the fixture, which may already carry the table's own forecast. */
function mount(tableForecast: CombatForecast | null = null) {
  const h = startSession()
  h.join(0, SEATS)
  act(() =>
    h.deliver({ type: 'Snapshot', seq: 1, state: { ...fixture.view, combat_forecast: tableForecast } }),
  )
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const root = table.container
  const card = (id: ObjectId): HTMLElement =>
    must(root.querySelector<HTMLElement>(`button[data-object-id="${id}"]`), `card ${id}`)
  const click = (element: HTMLElement): void => void fireEvent.click(element)
  const section = (slot: string): HTMLElement | null => root.querySelector(`[data-slot="${slot}"]`)
  return {
    h,
    ...table,
    card,
    click,
    send: (message: ServerMessage): void => void act(() => h.deliver(message)),
    wait: (ms: number): void => void act(() => vi.advanceTimersByTime(ms)),
    /** Chooses a blocker and the attacker it blocks. */
    block(blocker: ObjectId, attacker: ObjectId): void {
      click(card(blocker))
      click(card(attacker))
    },
    /** The state of an outcome section: checking, ready, unavailable or absent. */
    state: (slot = 'block-outcome'): string | null => section(slot)?.getAttribute('data-state') ?? null,
    /** What the live region of a section says. */
    spoken: (slot = 'block-outcome'): string | null =>
      section(slot)?.querySelector('[data-slot="outcome-live"]')?.textContent ?? null,
    doomed: (): ObjectId[] =>
      [...root.querySelectorAll<HTMLElement>('button[data-doomed]')].map((element) =>
        Number(element.getAttribute('data-object-id')),
      ),
    life: (seat: number): HTMLElement | null =>
      root.querySelector(`[data-seat="${seat}"] [data-slot="life-forecast"]`),
    previews: () => h.sentOf('Preview'),
    section,
  }
}

describe('blocks being chosen', () => {
  it('are asked about after a quiet moment, once, with the latest choice', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    expect(t.previews()).toEqual([])
    t.wait(DEBOUNCE_MS - 1)
    expect(t.previews()).toEqual([])
    // Another block restarts the quiet time.
    t.block(m2, t1)
    t.wait(DEBOUNCE_MS - 1)
    expect(t.previews()).toEqual([])
    t.wait(1)
    expect(t.previews()).toEqual([
      {
        type: 'Preview',
        decision: 31,
        answer: {
          DeclareBlockers: [
            [m1, t1],
            [m2, t1],
          ],
        },
      },
    ])
    // Nothing more is asked while the choice stays as it is.
    t.wait(GIVE_UP_MS * 2)
    expect(t.previews()).toHaveLength(1)
  })

  it('are not asked about when they are taken back before the quiet moment ends, or never chosen', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.wait(DEBOUNCE_MS * 5)
    t.block(m1, t1)
    t.click(screen.getByRole('button', { name: /^Remove the block of/ }))
    t.wait(DEBOUNCE_MS * 5)
    expect(t.previews()).toEqual([])
    expect(t.state()).toBeNull()
  })

  it('say "Checking outcome…" until the engine answers, then show what it said on cards, seats and in words', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    expect(t.state()).toBe('checking')
    expect(must(t.section('block-outcome'))).toHaveTextContent(CHECKING_WORDS)
    t.wait(DEBOUNCE_MS)
    expect(t.state()).toBe('checking')
    expect(t.doomed()).toEqual([])
    expect(t.life(0)).toBeNull()

    t.send(reply(31, forecastOf({ dies: [t1], damage: 3 })))
    expect(t.state()).toBe('ready')
    const outcome = must(t.section('block-outcome'))
    expect(outcome).toHaveTextContent('If you block like this')
    expect(outcome).not.toHaveTextContent(CHECKING_WORDS)
    expect(within(outcome).getByText('You would take 3 damage, from 20 to 17 life.')).toBeInTheDocument()
    expect(t.doomed()).toEqual([t1])
    expect(t.life(0)).toHaveTextContent('20')
    expect(t.life(0)).toHaveTextContent('17')
    // The same facts for a screen reader, in one sentence.
    expect(t.spoken()).toMatch(/^If you block like this: .* would die\. You would take 3 damage\.$/)
  })

  it('draw the reply of the latest choice and drop every mark the moment the choice changes', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS)
    t.send(reply(31, forecastOf({ dies: [t1] })))
    expect(t.doomed()).toEqual([t1])

    // A second block: the first reply says nothing of it, so its marks go at once, before anything is asked.
    t.block(m2, t2)
    expect(t.doomed()).toEqual([])
    expect(t.state()).toBe('checking')
    t.wait(DEBOUNCE_MS)
    expect(t.previews()).toHaveLength(2)
    t.send(reply(31, forecastOf({ dies: [m2, t2], damage: 1 })))
    expect(t.doomed().sort((a, b) => a - b)).toEqual([m2, t2].sort((a, b) => a - b))

    // Taking the second block back asks again for the first, and its marks return with the answer.
    t.click(must(screen.getAllByRole('button', { name: /^Remove the block of/ }).at(-1)))
    expect(t.doomed()).toEqual([])
    t.wait(DEBOUNCE_MS)
    expect(t.previews()).toHaveLength(3)
    t.send(reply(31, forecastOf({ dies: [t1] })))
    expect(t.doomed()).toEqual([t1])
  })

  it('ignore the reply to a choice that has been replaced', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS)
    t.block(m2, t1)
    t.wait(DEBOUNCE_MS)
    expect(t.previews()).toHaveLength(2)

    // The engine answers the first question first.
    t.send(reply(31, forecastOf({ dies: [t2] })))
    expect(t.doomed()).toEqual([])
    expect(t.state()).toBe('checking')
    t.send(reply(31, forecastOf({ dies: [t1] })))
    expect(t.doomed()).toEqual([t1])
    expect(t.state()).toBe('ready')
  })

  it('fall back to the old words when the engine has no forecast for them', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS)
    t.send(reply(31, null))
    expect(t.state()).toBe('unavailable')
    expect(must(t.section('block-outcome'))).toHaveTextContent(FALLBACK_WORDS)
    expect(t.spoken()).toBe(FALLBACK_WORDS)
    expect(t.doomed()).toEqual([])
    expect(t.life(0)).toBeNull()
  })

  it('fall back after 1.5 s without an answer, and still take a late one', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS + GIVE_UP_MS - 1)
    expect(t.state()).toBe('checking')
    t.wait(1)
    expect(t.state()).toBe('unavailable')
    expect(must(t.section('block-outcome'))).toHaveTextContent(FALLBACK_WORDS)
    t.send(reply(31, forecastOf({ dies: [t1] })))
    expect(t.state()).toBe('ready')
    expect(t.doomed()).toEqual([t1])
  })

  it('fall back, with no notice, when the server is too old to know previews, and stop asking', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS)
    t.send({ type: 'Error', code: 'BadMessage', message: 'send each message as one JSON text frame' })
    expect(t.h.state().notice).toBeNull()
    t.wait(GIVE_UP_MS)
    expect(t.state()).toBe('unavailable')
    // The next choice is not even sent: it falls back as soon as the quiet moment is over.
    t.block(m2, t2)
    expect(t.state()).toBe('checking')
    t.wait(DEBOUNCE_MS)
    expect(t.state()).toBe('unavailable')
    expect(t.previews()).toHaveLength(1)
    expect(t.h.state().notice).toBeNull()
  })

  it('keep the room of the last forecast while the engine is asked, unseen and unread', () => {
    const table = forecastOf({ dies: [t1], damage: 4 })
    const t = mount({
      ...table,
      attackers: [
        {
          attacker: t1,
          target: { Player: 0 },
          blocked: false,
          blockers: [],
          deals: [{ source: t1, target: { Player: 0 }, amount: 4 }],
          receives: [],
        },
      ],
    })
    t.send(blockers(31, [m1, m2], [t1, t2]))
    const items = (): number => t.section('block-outcome')?.querySelectorAll('ul > li').length ?? 0
    expect(t.state()).toBe('ready')
    expect(must(t.section('block-outcome'))).toHaveTextContent('If you do not block')
    const before = items()
    expect(before).toBeGreaterThan(1)

    t.block(m1, t1)
    expect(t.state()).toBe('checking')
    const kept = must(t.section('block-outcome')).querySelector('ul')
    expect(kept).toHaveAttribute('aria-hidden', 'true')
    expect(kept).toHaveClass('invisible')
    expect(items()).toBe(before)
    // Nothing of it can be read: the list is hidden from the accessibility tree.
    expect(
      screen.queryAllByRole('listitem').some((item) => item.closest('[data-slot="block-outcome"]')),
    ).toBe(false)
  })

  it('say nothing while checking, then one sentence for each change of the outcome', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    expect(t.spoken()).toBe('')
    t.wait(DEBOUNCE_MS)
    t.send(reply(31, forecastOf({ dies: [t1] })))
    const first = must(t.spoken())
    expect(first).toMatch(/would die\./)
    t.block(m2, t2)
    expect(t.spoken()).toBe('')
    t.wait(DEBOUNCE_MS)
    t.send(reply(31, forecastOf({ dies: [] })))
    expect(t.spoken()).toMatch(/^If you block like this: No creature would die\./)
    expect(t.spoken()).not.toBe(first)
  })
})

describe('when the decision ends', () => {
  it('a decision the server replaces starts with no preview, no marks and no question asked', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS)
    t.send(reply(31, forecastOf({ dies: [t1] })))
    expect(t.doomed()).toEqual([t1])

    t.send(blockers(32, [m1, m2, m3], [t1, t2]))
    expect(t.h.state().preview).toBeNull()
    expect(t.doomed()).toEqual([])
    expect(t.section('block-outcome')).toBeNull()
    t.wait(GIVE_UP_MS * 2)
    expect(t.previews()).toHaveLength(1)
    // A reply that was still on its way for the old decision changes nothing.
    t.send(reply(31, forecastOf({ dies: [t2] })))
    expect(t.h.state().preview).toBeNull()
    expect(t.doomed()).toEqual([])
  })

  it('an answer sent before the quiet moment is over leaves no question behind', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS - 1)
    t.click(screen.getByRole('button', { name: 'Confirm blocks' }))
    expect(t.h.answeredIds()).toEqual([31])
    t.wait(GIVE_UP_MS * 2)
    expect(t.previews()).toEqual([])
    expect(t.h.state().preview).toBeNull()
  })

  it('an answer clears the preview, and the marks with it', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS)
    t.send(reply(31, forecastOf({ dies: [t1] })))
    t.click(screen.getByRole('button', { name: 'Confirm blocks' }))
    expect(t.h.state().preview).toBeNull()
    expect(t.doomed()).toEqual([])
  })

  it('a lost connection takes the prompt, the preview and the marks away together', () => {
    const t = mount()
    t.send(blockers(31, [m1, m2], [t1, t2]))
    t.block(m1, t1)
    t.wait(DEBOUNCE_MS)
    t.send(reply(31, forecastOf({ dies: [t1] })))
    act(() => t.h.net.last.fail())
    expect(t.h.state().preview).toBeNull()
    expect(t.doomed()).toEqual([])
    expect(t.section('block-outcome')).toBeNull()
  })
})

describe('an attack being chosen', () => {
  it('is asked about with the defender each creature goes at, and its lethal mark is drawn on the seat', () => {
    const t = mount()
    t.send(attackers(41, [m1, m2]))
    t.click(t.card(m1))
    expect(t.state('attack-outcome')).toBe('checking')
    t.wait(DEBOUNCE_MS)
    expect(t.previews()).toEqual([
      { type: 'Preview', decision: 41, answer: { DeclareAttackers: [[m1, { Player: 1 }]] } },
    ])

    t.send(reply(41, forecastOf({ seat: 1, damage: 5, life: 5 })))
    expect(t.state('attack-outcome')).toBe('ready')
    expect(must(t.section('attack-outcome'))).toHaveTextContent('If you attack like this')
    expect(
      within(must(t.section('attack-outcome'))).getByText(
        'Bo would take 5 damage, from 5 to 0 life, which is lethal.',
      ),
    ).toBeInTheDocument()
    expect(t.life(1)).toHaveAttribute('data-lethal', 'true')
    expect(t.spoken('attack-outcome')).toMatch(
      /^If you attack like this: .*Bo would take 5 damage, which is lethal\.$/,
    )
  })

  it('is asked about again when another creature joins it, aimed where the player said', () => {
    const t = mount()
    t.send(attackers(41, [m1, m2]))
    t.click(t.card(m1))
    t.wait(DEBOUNCE_MS)
    t.send(reply(41, forecastOf({ seat: 1, damage: 2, life: 20 })))
    expect(t.life(1)).not.toBeNull()

    t.click(t.card(m2))
    expect(t.life(1)).toBeNull()
    t.wait(DEBOUNCE_MS)
    expect(t.previews().map((p) => (p as { answer: unknown }).answer)).toEqual([
      { DeclareAttackers: [[m1, { Player: 1 }]] },
      {
        DeclareAttackers: [
          [m1, { Player: 1 }],
          [m2, { Player: 1 }],
        ],
      },
    ])
  })

  it('asks nothing for no attackers, and clearing the attack takes the numbers off the seats', () => {
    const t = mount()
    t.send(attackers(41, [m1]))
    t.wait(DEBOUNCE_MS * 5)
    expect(t.previews()).toEqual([])
    expect(t.state('attack-outcome')).toBeNull()
    t.click(t.card(m1))
    t.wait(DEBOUNCE_MS)
    t.send(reply(41, forecastOf({ seat: 1, damage: 5, life: 5 })))
    expect(t.life(1)).not.toBeNull()
    t.click(screen.getByRole('button', { name: 'Clear' }))
    expect(t.life(1)).toBeNull()
    expect(t.state('attack-outcome')).toBeNull()
    t.wait(DEBOUNCE_MS * 5)
    expect(t.previews()).toHaveLength(1)
  })

  it('falls back to the old words when the engine has no forecast', () => {
    const t = mount()
    t.send(attackers(41, [m1]))
    t.click(t.card(m1))
    t.wait(DEBOUNCE_MS)
    t.send(reply(41, null))
    expect(t.state('attack-outcome')).toBe('unavailable')
    expect(must(t.section('attack-outcome'))).toHaveTextContent(FALLBACK_WORDS)
  })
})
