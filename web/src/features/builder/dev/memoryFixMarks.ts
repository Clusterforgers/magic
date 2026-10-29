import type { FixMarksApi } from '@/net/fixMarks'
import type { CardHit, FixMarkView } from '@/protocol/generated'

/** The fix-mark endpoints in memory for `/dev/builder`: marks last for the page, the names come from `cards`. */
export function memoryFixMarks(cards: readonly CardHit[]): FixMarksApi {
  const names = new Map(cards.map((card) => [card.oracle_id, card.name]))
  const marks = new Map<string, FixMarkView>()
  return {
    list: () => Promise.resolve([...marks.values()].toReversed()),
    mark: (oracleId, note = null) => {
      const view: FixMarkView = {
        oracle_id: oracleId,
        name: names.get(oracleId) ?? oracleId,
        note,
        created_at: marks.get(oracleId)?.created_at ?? new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
      }
      marks.set(oracleId, view)
      return Promise.resolve(view)
    },
    unmark: (oracleId) => {
      marks.delete(oracleId)
      return Promise.resolve()
    },
  }
}
