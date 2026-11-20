/** Goal: with "Always pay for me" on, a payment the game still asks about shows one short line saying why (a Treasure, a Phyrexian symbol, convoke), named with the card; with the setting off, or when the game finds no payment at all, the line is left out. */
import { describe, expect, it } from 'vitest'
import type { AskedWhy, PayOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { cost, poolOf, source } from '@/test-utils/mana'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const view = fixture.view
const land = must(
  view.objects.find(
    (object) => object.zone === 'Battlefield' && object.controller === 0 && object.types.includes('Land'),
  ),
)
const spell = must(view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === 0))

const payOptions = (text: string, asked: AskedWhy | undefined, auto = true): PayOptions => ({
  spell: spell.id,
  cost: cost(text),
  pool: poolOf(''),
  sources: [source(land.id, 'R')],
  auto_available: auto,
  can_cancel: true,
  life: 20,
  max_life: 0,
  ...(asked === undefined ? {} : { asked }),
})

function askedLine(given: PayOptions, autoPay: boolean): string | null {
  const table = renderTable({
    state: (state) => ({
      ...state,
      autoChoices: { ...state.autoChoices, auto_pay: autoPay },
      pending: { id: 4, decision: { player: 0, kind: { PayMana: given } } },
    }),
  })
  return table.container.querySelector('[data-slot="pay-asked"]')?.textContent ?? null
}

describe('the line saying why a payment is asked', () => {
  it.each<[string, string, AskedWhy, string]>([
    [
      'a Treasure',
      '{R}',
      { GivesUp: [land.id] },
      `Asked: only ${land.name} can pay the rest (sacrificed or exiled).`,
    ],
    [
      'a Phyrexian symbol',
      '{B/P}',
      { Life: { Phyrexian: 'Black' } },
      'Asked: paying {B/P} needs life or black mana.',
    ],
    ['convoke', '{3}', 'Convoke', 'Asked: the rest needs convoke.'],
    [
      'a painland',
      '{R}',
      { DoesMore: [land.id] },
      `Asked: only ${land.name} can pay the rest (does more than add mana).`,
    ],
  ])('names %s while the setting is on', (_, text, asked, line) => {
    expect(askedLine(payOptions(text, asked), true)).toBe(line)
  })

  it.each<[string, PayOptions, boolean]>([
    ['the setting is off', payOptions('{R}', { GivesUp: [land.id] }), false],
    ['plain mana pays', payOptions('{R}', undefined), true],
    ['no payment exists', payOptions('{5}', 'NoPayment', false), true],
  ])('is left out when %s', (_, given, autoPay) => {
    expect(askedLine(given, autoPay)).toBeNull()
  })
})
