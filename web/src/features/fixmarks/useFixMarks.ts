import { useMutation, useQuery } from '@tanstack/react-query'
import type { FixMarksApi } from '@/net/fixMarks'
import type { FixMarkView } from '@/protocol/generated'
import { fixMarksClient } from './client'
import { marksUnavailable } from './errors'

const KEY = ['fix-marks'] as const

/** What a press asks for: the card, and either a mark (with its note) or taking the mark back. */
export interface MarkRequest {
  oracleId: string
  name: string
  /** True to mark the card, false to take the mark back. */
  marked: boolean
  /** The note of a mark; `null` or absent for none. */
  note?: string | null
}

/** The mark of a card with the note it has, kept in the list at once while the server answers. */
const optimisticView = (request: MarkRequest, kept: FixMarkView | undefined): FixMarkView => ({
  oracle_id: request.oracleId,
  name: request.name,
  note: request.note ?? null,
  created_at: kept?.created_at ?? new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
})

/** The list with `view` in place of the mark of the same card, or added first. */
const withMark = (marks: readonly FixMarkView[], view: FixMarkView): FixMarkView[] => [
  view,
  ...marks.filter((mark) => mark.oracle_id !== view.oracle_id),
]

/** What the button needs of the account's marks. */
export interface FixMarks {
  /** The account's mark on a card, if it made one; `undefined` until the list has loaded. */
  markOf: (oracleId: string) => FixMarkView | undefined
  /** True while the list loads for the first time. */
  loading: boolean
  /** True when marks cannot be made here at all: no session, or no card database. */
  unavailable: boolean
  /** Marks a card or takes the mark back; the list changes before the server answers and goes back if it refuses. */
  press: (request: MarkRequest) => void
  /** True while a press is on its way to the server. */
  pending: boolean
  /** Why the last press failed; `null` after a press that worked or before any. */
  error: unknown
  /** Forgets the error of the last press. */
  clearError: () => void
}

/**
 * The account's fix marks as a TanStack Query list (`GET /api/fix-marks`) and the press that changes it. A press is
 * optimistic: the card is marked or unmarked in the list at once, the server's answer replaces it, and a refusal puts the
 * list back as it was and keeps the error for the button to show.
 */
export function useFixMarks(api: FixMarksApi): FixMarks {
  const list = useQuery({ queryKey: KEY, queryFn: () => api.list() }, fixMarksClient)
  const press = useMutation(
    {
      mutationFn: async (request: MarkRequest): Promise<FixMarkView | null> => {
        if (!request.marked) {
          await api.unmark(request.oracleId)
          return null
        }
        return api.mark(request.oracleId, request.note ?? null)
      },
      onMutate: async (request) => {
        await fixMarksClient.cancelQueries({ queryKey: KEY })
        const previous = fixMarksClient.getQueryData<FixMarkView[]>(KEY)
        fixMarksClient.setQueryData<FixMarkView[]>(KEY, (marks = []) =>
          request.marked
            ? withMark(
                marks,
                optimisticView(
                  request,
                  marks.find((mark) => mark.oracle_id === request.oracleId),
                ),
              )
            : marks.filter((mark) => mark.oracle_id !== request.oracleId),
        )
        return { previous }
      },
      onSuccess: (view) => {
        if (view !== null) fixMarksClient.setQueryData<FixMarkView[]>(KEY, (marks = []) => withMark(marks, view))
      },
      onError: (_error, _request, context) => {
        if (context?.previous === undefined) fixMarksClient.removeQueries({ queryKey: KEY })
        else fixMarksClient.setQueryData(KEY, context.previous)
      },
    },
    fixMarksClient,
  )
  const marks = list.data
  return {
    markOf: (oracleId) => marks?.find((mark) => mark.oracle_id === oracleId),
    loading: list.isPending && !list.isError,
    unavailable: list.isError && marksUnavailable(list.error),
    press: press.mutate,
    pending: press.isPending,
    error: press.error,
    clearError: press.reset,
  }
}
