// Goal: `useBlockPreview` is the only place the table asks which forecast to draw: when it hands back a forecast for the blocks being chosen (as the engine's preview of a pending answer will), the prompt shows it as "if you block like this", cards look doomed and seats show their life by it, with no other change; and when it hands back nothing the table and the prompt show no forecast, however many the table holds.
import { act, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { CombatForecast, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderForecastTable } from '@/test-utils/forecastTable'

// What the seam returns while a test says so, as a store the mocked seam listens to the way a real preview would be
// listened to; the real seam still runs underneath.
const override = vi.hoisted(() => {
  type State = { on: boolean; forecast: CombatForecast | null }
  const listeners = new Set<() => void>()
  let state: State = { on: false, forecast: null }
  return {
    get: (): State => state,
    set(next: State): void {
      state = next
      for (const listener of listeners) listener()
    },
    subscribe(listener: () => void): () => void {
      listeners.add(listener)
      return () => void listeners.delete(listener)
    },
  }
})

vi.mock('../combat/forecast', async (importOriginal) => {
  const original = await importOriginal<typeof import('../combat/forecast')>()
  const { useSyncExternalStore } = await import('react')
  return {
    ...original,
    useBlockPreview() {
      const real = original.useBlockPreview()
      const given = useSyncExternalStore(override.subscribe, override.get)
      return given.on ? given.forecast : real
    },
  }
})

afterEach(() => override.set({ on: false, forecast: null }))

/** The table of the dev block prompt, with the first creature of seat 0 and the first attacker it may block. */
function block() {
  const table = renderForecastTable({ seats: 3, prompt: 'block' })
  const kind = must(table.store.getState().pending).decision.kind
  const options = must('DeclareBlockers' in kind ? kind.DeclareBlockers : null)
  const [blocker] = options.blockers.map((entry) => entry.object) as [ObjectId]
  const [attacker] = must(options.blockers[0]).can_block as [ObjectId]
  return { ...table, blocker, attacker }
}

/** A preview of the pending block: both creatures would die, and nobody takes damage. */
const previewOf = (blocker: ObjectId, attacker: ObjectId): CombatForecast => ({
  attackers: [
    {
      attacker,
      target: { Player: 0 },
      blocked: true,
      blockers: [blocker],
      deals: [{ source: attacker, target: { Object: blocker }, amount: 3 }],
      receives: [{ source: blocker, target: { Object: attacker }, amount: 3 }],
    },
  ],
  creatures: [
    { object: blocker, damage: 3, dies: true },
    { object: attacker, damage: 3, dies: true },
  ],
  players: [{ player: 0, damage: 0, life: 11, life_after: 11, lethal: false }],
})

describe('the seam of the block preview', () => {
  it('lets a preview for the pending block through to the prompt, the cards and the seat', async () => {
    const t = block()
    await t.user.click(t.card(t.blocker))
    await t.user.click(t.card(t.attacker))
    expect(screen.getByText('Outcome shown after you confirm.')).toBeInTheDocument()
    expect(t.doomed()).toEqual([])

    // The engine answers: the pending block now has a forecast, and the table draws it.
    act(() => override.set({ on: true, forecast: previewOf(t.blocker, t.attacker) }))

    const outcome = must(t.container.querySelector<HTMLElement>('[data-slot="block-outcome"]'))
    expect(outcome).toHaveTextContent('If you block like this')
    expect(outcome).not.toHaveTextContent('Outcome shown after you confirm')
    const items = within(outcome).getAllByRole('listitem')
    expect(items).toHaveLength(2)
    expect(items[0]).toHaveTextContent('You would take no damage.')
    expect(items[1]).toHaveTextContent(/would die, your .* would die\.$/)
    expect(t.doomed().sort((a, b) => a - b)).toEqual([t.blocker, t.attacker].sort((a, b) => a - b))
    expect(t.life(0)).toHaveTextContent('No combat damage')
    expect(screen.getByRole('list', { name: 'Combat forecast' })).toBeInTheDocument()
    // The block lines stay what they are: dashed, with no colour of their own.
    expect(
      t
        .lines()
        .filter((line) => line.pending)
        .map((line) => line.outcome),
    ).toEqual([null])

    act(() => override.set({ on: false, forecast: null }))
    expect(t.doomed()).toEqual([])
    expect(screen.getByText('Outcome shown after you confirm.')).toBeInTheDocument()
  })

  it('shows nothing of a forecast the seam does not give, whatever the table holds', () => {
    override.set({ on: true, forecast: null })
    const t = block()
    expect(t.forecast().players.length).toBeGreaterThan(0)
    expect(t.life(0)).toBeNull()
    expect(t.damage().size).toBe(0)
    expect(t.doomed()).toEqual([])
    expect(t.container.querySelector('[data-slot="block-outcome"]')).toBeNull()
    expect(screen.queryByRole('list', { name: 'Combat forecast' })).toBeNull()
  })
})
