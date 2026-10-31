// Goal: a draw by hand names the cards drawn for the seat that is told them, as an ordinary draw does, and says only how many to a seat that is not (hidden draws, or a run it was told only in part), so no card name reaches a viewer that may not see it.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { EventNames } from '../eventText'
import type { EventView } from '@/protocol/generated'
import { hid, pub } from '@/test-utils/turnEvents'
import { foldLog } from '../logFold'

const CARDS: Record<number, string> = { 1: 'Forest', 2: 'Island', 3: 'Sol Ring' }
const names: EventNames = {
  seat: (seat) => ['Ana', 'Bo'][seat] ?? `Seat ${seat + 1}`,
  card: (id) => CARDS[id] ?? 'a card',
}
const draw = (count: number): EventView => pub({ ManualAction: { player: 1, action: { Draw: { count } } } })
const drawn = (object: number): EventView => pub({ CardDrawn: { player: 1, object } })
const unseen: EventView = hid({ CardDrawn: { player: 1 } })
const fold = (events: readonly EventView[]): string[] => {
  const log: LogEntry[] = events.map((event, seq) => ({ seq, event }))
  return foldLog(log, names, { seats: [0, 1] }).map((line) => line.text)
}

describe('a draw by hand', () => {
  it.each<[string, EventView[], string[]]>([
    ['one card the drawer sees', [draw(1), drawn(1)], ['Bo draws Forest (by hand)']],
    ['two cards the drawer sees', [draw(2), drawn(1), drawn(2)], ['Bo draws Forest and Island (by hand)']],
    [
      'three cards the drawer sees',
      [draw(3), drawn(1), drawn(2), drawn(3)],
      ['Bo draws Forest, Island and Sol Ring (by hand)'],
    ],
    ['one card another seat cannot see', [draw(1), unseen], ['Bo draws 1 card (by hand)']],
    ['two cards another seat cannot see', [draw(2), unseen, unseen], ['Bo draws 2 cards (by hand)']],
    ['cards of which only one is told', [draw(2), drawn(1), unseen], ['Bo draws 2 cards (by hand)']],
    ['a draw with no card told yet', [draw(2)], ['Bo draws 2 cards (by hand)']],
  ])('reads %s', (_what, events, lines) => {
    expect(fold(events)).toEqual(lines)
  })

  it('leaves the events after the draw as they were', () => {
    expect(fold([draw(1), drawn(1), pub({ LifeLost: { player: 0, amount: 1 } })])).toEqual([
      'Bo draws Forest (by hand)',
      'Ana loses 1 life',
    ])
  })

  it('names no card for a viewer who got hidden draws, whatever the table calls the objects', () => {
    const lines = fold([draw(2), unseen, unseen])
    expect(lines.join(' ')).not.toMatch(/Forest|Island|Sol Ring|a card/)
  })

  it('reads an ordinary draw in the same words as the cards of a draw by hand', () => {
    const ordinary = fold([drawn(1)])
    expect(ordinary).toEqual(['Bo draws Forest'])
    expect(fold([draw(1), drawn(1)])).toEqual([`${ordinary[0]} (by hand)`])
  })
})
