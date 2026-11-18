// Goal: a boss's abilities come from its public view (innate with its level, modifier, relics, phases by round) in reading order with a one-line summary each, the table's bar shows them as chips named for the boss, and the log names an ability of a boss's command zone as the boss's.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectView, PlayerView } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from '../GameTable'
import { buildFixture } from '../dev/fixture'
import { withBoss } from '../dev/devGame'
import { describeEvent } from '../eventText'
import { bossAbilities, bossOwner, summaryOf } from './bossAbilities'

describe('bossAbilities', () => {
  it('reads the boss view in order: innate, modifier, relics, then the phases', () => {
    const fixture = withBoss(buildFixture({ seats: 4 }), 0, 2)
    const boss = fixture.view.players.find((player) => player.boss !== undefined)
    if (boss === undefined) throw new Error('no boss')
    const abilities = bossAbilities(boss)
    expect(abilities.map((ability) => ability.kind)).toEqual(['innate', 'modifier', 'relic', 'phase', 'next-phase'])
    expect(abilities[0]?.when).toBe('Level III')
    expect(abilities[2]?.name).toBe('Whetstone ×2')
    expect(bossAbilities({ ...boss, boss: undefined })).toEqual([])
  })

  it('summarises a rule by its first sentence, without reminder text', () => {
    expect(summaryOf('Flying. (It soars.)\nMore.')).toBe('Flying.')
    expect(summaryOf('One.\nTwo.')).toBe('One.')
    expect(summaryOf('')).toBe('')
  })
})

describe('the boss abilities at the table', () => {
  it('shows a chip for the innate, the modifier and the relics, named for the boss', () => {
    renderTable({ fixture: withBoss(buildFixture({ seats: 4 }), 0), ui: <GameTable /> })
    const bar = within(screen.getByRole('list', { name: /abilities$/ }))
    for (const name of ['Brood III', 'Deep Pockets', 'Whetstone ×2']) expect(bar.getByText(name)).toBeInTheDocument()
  })
})

describe('the log names a boss ability', () => {
  it('as "Boss: ability triggers", and any other source as before', () => {
    const emblem = { id: 7, name: 'Brood III', zone: { Command: 3 } } as ObjectView
    const players = [{ seat: 3, boss: { phases: [], begun: 0 } }] as unknown as PlayerView[]
    const names = {
      seat: (seat: number) => (seat === 3 ? 'Skrix, the Rat Queen' : `Seat ${seat}`),
      card: (id: number) => (id === 7 ? 'Brood III' : 'Llanowar Elves'),
      boss: (id: number) =>
        bossOwner(id === 7 ? emblem : undefined, players, (seat) => (seat === 3 ? 'Skrix, the Rat Queen' : '')),
    }
    const triggered = (id: number) =>
      ({
        Public: {
          AbilityTriggered: {
            source: { id, counter: 0 },
            ability: 0,
            controller: 3,
            cause: { subjects: [], snapshots: [] },
            snapshot: {
              id,
              counter: 0,
              controller: 3,
              owner: 3,
              zone: { Command: 3 },
              characteristics: { types: 0, colors: 0, abilities: [] },
              counters: [],
            },
          },
        },
      }) as unknown as Parameters<typeof describeEvent>[0]
    expect(describeEvent(triggered(7), names)?.text).toBe('Skrix, the Rat Queen: Brood III triggers')
    expect(describeEvent(triggered(8), names)?.text).toBe('An ability of Llanowar Elves triggers')
  })
})
