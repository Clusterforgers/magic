import { useEffect } from 'react'
import { Undo2 } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { GoBackRequest } from '@/play/store'
import { useDockForm } from './dockLayout'
import { KeyHint } from './prompt/KeyHint'
import { isFocusFree } from './prompt/usePrimaryKey'
import { useSeatNamer } from './names'
import {
  GO_BACK_KEY,
  GO_BACK_REFUSAL_WORDS,
  goBackAsk,
  goBackLabel,
  goBackOutcomeWords,
  waitingFor,
} from './goBackText'
import { secondsLeft, TAKE_BACK_NOTICE_MS } from './takeBackText'
import { useNow } from './useNow'

/**
 * Going back a step at the table: "Back to Precombat main" while the server offers the seat an earlier step it passed
 * in (Backspace too, while nothing has the keyboard); while a request over other people's passes is open, Allow and
 * Decline for a seat asked, "Waiting for Bo…" with Cancel for the asker; then a short line saying how it ended or why a
 * go-back was refused, which goes by itself.
 */
export function GoBackControl() {
  const { session } = useGame()
  const { offer, refused, request, notice, me, online, playing } = usePlay(
    useShallow((state) => ({
      offer: state.goBackOffer,
      refused: state.goBackRefused,
      request: state.goBackRequest,
      notice: state.goBackNotice,
      me: state.seat,
      online: state.status === 'open',
      playing: state.phase === 'playing',
    })),
  )
  const seat = useSeatNamer()
  const form = useDockForm()
  const canGo = offer !== null && playing && online && offer.player === me && request === null

  useEffect(() => {
    if (refused === null) return
    const timer = window.setTimeout(() => session.dismissGoBackRefusal(), TAKE_BACK_NOTICE_MS)
    return () => window.clearTimeout(timer)
  }, [refused, session])

  useEffect(() => {
    if (notice === null) return
    const timer = window.setTimeout(() => session.dismissGoBackNotice(), TAKE_BACK_NOTICE_MS)
    return () => window.clearTimeout(timer)
  }, [notice, session])

  useEffect(() => {
    if (!canGo) return
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== GO_BACK_KEY || event.repeat || event.altKey || event.ctrlKey || event.metaKey) return
      if (event.shiftKey || event.defaultPrevented || !isFocusFree(event.target)) return
      event.preventDefault()
      session.goBack()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [canGo, session])

  const line =
    refused !== null
      ? GO_BACK_REFUSAL_WORDS[refused]
      : notice !== null
        ? goBackOutcomeWords(notice.offer, notice.outcome, seat)
        : null
  const open = request !== null && playing
  if (!canGo && line === null && !open) return null
  return (
    <div data-slot="go-back" className="flex max-w-full flex-col items-end gap-1">
      {line !== null && (
        <div data-slot="go-back-notice" className="flex items-center gap-2 text-sm text-muted-foreground">
          <p role="status" aria-live="polite">
            {line}
          </p>
          <Button
            size="sm"
            variant="ghost"
            onClick={() =>
              refused !== null ? session.dismissGoBackRefusal() : session.dismissGoBackNotice()
            }
          >
            OK
          </Button>
        </div>
      )}
      {open && <GoBackRequestLine request={request} me={me} online={online} />}
      {canGo && (
        <Button
          size="sm"
          variant="outline"
          className="max-w-full max-sm:h-9 max-sm:px-3"
          aria-keyshortcuts={GO_BACK_KEY}
          title={`${goBackLabel(offer.step)} (${GO_BACK_KEY})`}
          onClick={() => session.goBack()}
        >
          <Undo2 aria-hidden className="size-4 shrink-0" />
          <span className="min-w-0 truncate">{goBackLabel(offer.step)}</span>
          {form === 'full' ? <KeyHint>⌫</KeyHint> : null}
        </Button>
      )}
    </div>
  )
}

/** The open request: Allow and Decline for a seat asked, Cancel for the asker, who is waited on for everyone else. */
function GoBackRequestLine({
  request,
  me,
  online,
}: {
  request: GoBackRequest
  me: number | null
  online: boolean
}) {
  const { session } = useGame()
  const seat = useSeatNamer()
  const now = useNow(true)
  // A phone's bar has one line for it: the question and the clock say the least there.
  const compact = useDockForm() === 'compact'
  const left = `${secondsLeft(request, now)} s${compact ? '' : ' left'}`
  const clock = (
    <span data-slot="go-back-clock" className="text-muted-foreground tabular-nums">
      {left}
    </span>
  )
  const ask = goBackAsk(request.offer, seat)
  if (me !== null && request.waiting.includes(me)) {
    return (
      <div
        role="group"
        aria-label="Go-back request"
        data-request
        className="flex max-w-full flex-wrap items-center justify-end gap-x-2 gap-y-1 text-sm"
      >
        <p title={ask} className={compact ? undefined : 'basis-full text-right'}>
          {compact ? `${seat(request.offer.player)}: ${goBackLabel(request.offer.step)}?` : ask}
        </p>
        {clock}
        <Button size="sm" disabled={!online} onClick={() => session.voteGoBack(true)}>
          Allow
        </Button>
        <Button size="sm" variant="outline" disabled={!online} onClick={() => session.voteGoBack(false)}>
          Decline
        </Button>
      </div>
    )
  }
  const waiting = waitingFor(request.waiting, seat)
  if (me === request.offer.player) {
    return (
      <div
        data-slot="go-back-waiting"
        data-request
        className="flex max-w-full flex-wrap items-center justify-end gap-x-2 gap-y-1 text-sm"
      >
        <p className={cn('text-muted-foreground', !compact && 'basis-full text-right')} title={ask}>
          {waiting}
        </p>
        {clock}
        <Button size="sm" variant="outline" disabled={!online} onClick={() => session.cancelGoBack()}>
          Cancel
        </Button>
      </div>
    )
  }
  return (
    <p data-slot="go-back-waiting" data-request className="text-sm text-muted-foreground">
      {ask}. {waiting} {clock}
    </p>
  )
}
