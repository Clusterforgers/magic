// Goal: the game log names a card that has left the view from the names the page keeps, prefers the view's own name while it holds the object, and says "a card" only for an object nobody named.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { EventView } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { LogPanel } from './LogPanel'

const GONE = 987_654

const died = (object: number): EventView => ({
  Public: { CreatureDied: { object, cause: 'LethalDamage' } },
})

function lines(names: Record<number, string>, object = GONE): string[] {
  renderTable({
    seats: 2,
    ui: <LogPanel />,
    state: (first) => ({
      ...first,
      log: [{ seq: 0, event: died(object) }],
      logNames: names,
    }),
  })
  return within(screen.getByRole('log', { name: 'Game log' }))
    .getAllByRole('listitem')
    .map((item) => item.textContent ?? '')
}

describe('the log lines about objects that left the view', () => {
  it.each([
    ['a name the page kept', { [GONE]: 'Goblin token' }, 'Goblin token dies from lethal damage'],
    ['no name at all', {}, 'a card dies from lethal damage'],
    ['a name of another object', { 5: 'Elf' }, 'a card dies from lethal damage'],
  ])('read %s', (_what, names, expected) => {
    expect(lines(names)).toContain(expected)
  })

  it('prefers the name the view gives to an object it holds', () => {
    const probe = renderTable({ seats: 2, ui: <LogPanel /> })
    const held = Object.values(probe.store.getState().objectsById).find((object) => object.name !== '')
    probe.unmount()
    expect(held).toBeDefined()
    const id = held?.id ?? 0
    const text = lines({ [id]: 'Stale name' }, id)
    expect(text.some((line) => line.includes('Stale name'))).toBe(false)
    expect(text.some((line) => line.includes(`${held?.name} dies`))).toBe(true)
  })
})
