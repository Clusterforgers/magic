/** Applies a batch to a play store the way the session does, for the dev route and tests that have no socket. */
import { appendLog } from '@/play/store'
import type { PlayStore } from '@/play/store'
import { applyBatch, indexById } from '@/play/view'
import type { EventBatch } from '@/protocol/generated'

/** Folds `batch` into the store's view and log. False, changing nothing, when there is no view or the batch does not follow on. */
export function applyBatchToStore(store: PlayStore, batch: EventBatch): boolean {
  const state = store.getState()
  if (state.view === null || state.seq !== batch.from_seq) return false
  const view = applyBatch(state.view, batch)
  store.setState({
    view,
    objectsById: view.objects === state.view.objects ? state.objectsById : indexById(view.objects),
    seq: batch.from_seq + batch.events.length,
    log: appendLog(state.log, batch.from_seq, batch.events),
  })
  return true
}
