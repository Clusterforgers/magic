// Goal: the legend lists exactly the kinds of line that are on screen (attacks, blocks without a forecast, each outcome that occurs, and unsent choices), once each, in a fixed order, so it never explains a colour that is not there and never misses one that is.
import { describe, expect, it } from 'vitest'
import { legendEntries } from './legend'
import type { Outcome } from './outcome'
import type { LineKind, LineSpec } from './plan'

let next = 0
const line = (kind: LineKind, over: { pending?: boolean; outcome?: Outcome } = {}): LineSpec => {
  next += 1
  return {
    key: `line:${next}`,
    kind,
    pending: over.pending ?? false,
    from: { kind: 'card', id: next },
    to: { kind: 'seat', seat: 0 },
    ...(over.outcome === undefined ? {} : { outcome: over.outcome }),
  }
}

describe('legendEntries', () => {
  it.each<[string, LineSpec[], string[]]>([
    ['no lines', [], []],
    ['attacks only', [line('attack'), line('attack')], ['Attack']],
    ['blocks with no forecast', [line('attack'), line('block')], ['Attack', 'Block']],
    ['one outcome', [line('attack'), line('block', { outcome: 'both-die' })], ['Attack', 'Both die']],
    [
      'every outcome, in the legend order whatever the order of the lines',
      [
        line('attack'),
        line('block', { outcome: 'both-survive' }),
        line('block', { outcome: 'attacker-dies' }),
        line('block', { outcome: 'both-die' }),
        line('block', { outcome: 'blocker-dies' }),
      ],
      ['Attack', 'Attacker dies', 'Blocker dies', 'Both die', 'Both survive'],
    ],
    [
      'an outcome twice',
      [line('block', { outcome: 'blocker-dies' }), line('block', { outcome: 'blocker-dies' })],
      ['Blocker dies'],
    ],
    [
      'blocks with and without an outcome',
      [line('block', { outcome: 'both-die' }), line('block')],
      ['Block', 'Both die'],
    ],
    ['unsent blocks', [line('block', { pending: true })], ['Chosen, not confirmed yet']],
    ['unsent attacks', [line('attack', { pending: true })], ['Chosen, not confirmed yet']],
    [
      'unsent choices on top of a declared combat',
      [line('attack'), line('block', { outcome: 'attacker-dies' }), line('block', { pending: true })],
      ['Attack', 'Attacker dies', 'Chosen, not confirmed yet'],
    ],
  ])('for %s lists %j', (_name, lines, labels) => {
    expect(legendEntries(lines).map((entry) => entry.label)).toEqual(labels)
  })

  it('gives each entry the look of the lines it explains, and an id of its own', () => {
    const entries = legendEntries([
      line('attack'),
      line('block', { outcome: 'both-die' }),
      line('block', { pending: true }),
    ])
    expect(entries).toEqual([
      { id: 'attack', label: 'Attack', kind: 'attack', pending: false },
      { id: 'both-die', label: 'Both die', kind: 'block', pending: false, outcome: 'both-die' },
      { id: 'pending', label: 'Chosen, not confirmed yet', kind: 'block', pending: true },
    ])
  })
})
