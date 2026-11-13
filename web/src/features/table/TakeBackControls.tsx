import { useEffect, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { TakeBackNotice } from '@/play/store'
import { useSeatNamer } from './names'
import { outcomeWords, REFUSAL_WORDS, secondsLeft, TAKE_BACK_NOTICE_MS, takeBackAction } from './takeBackText'
import type { SeatNamer } from './names'

/** The time now, read again every second while `running`. */
function useNow(running: boolean): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!running) return
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [running])
  return now
}

const noticeText = (notice: TakeBackNotice, seat: SeatNamer): string =>
  'refused' in notice ? REFUSAL_WORDS[notice.refused] : outcomeWords(notice.offer, notice.outcome, seat)

/**
 * Take-backs at the table: "Take back casting Shock" while the seat has something to take back; while a request is
 * open, Allow and Decline with the time left for a seat the table waits on, and "Waiting for the table…" for the
 * others; then a short notice of how it ended or why it was refused, which goes by itself.
 */
export function TakeBackControls() {
  const { session } = useGame()
  const seat = useSeatNamer()
  const { offer, request, notice, me, online, playing } = usePlay(
    useShallow((state) => ({
      offer: state.takeBackOffer,
      request: state.takeBackRequest,
      notice: state.takeBackNotice,
      me: state.seat,
      online: state.status === 'open',
      playing: state.phase === 'playing',
    })),
  )
  const now = useNow(request !== null)
  useEffect(() => {
    if (notice === null) return
    const timer = window.setTimeout(() => session.dismissTakeBackNotice(), TAKE_BACK_NOTICE_MS)
    return () => window.clearTimeout(timer)
  }, [notice, session])

  const noticeLine = notice !== null && (
    <div data-slot="take-back-notice" className="flex items-center gap-2 text-sm text-muted-foreground">
      <p role="status" aria-live="polite">
        {noticeText(notice, seat)}
      </p>
      <Button size="sm" variant="ghost" onClick={() => session.dismissTakeBackNotice()}>
        OK
      </Button>
    </div>
  )

  if (request !== null) {
    const action = takeBackAction(request.offer.kind, request.offer.name)
    const left = `${secondsLeft(request, now)} s left`
    const asked = me !== null && request.waiting.includes(me)
    return (
      <div data-slot="take-back" className="flex flex-col items-end gap-1 text-sm">
        {noticeLine}
        {asked ? (
          <div
            role="group"
            aria-label="Take-back request"
            className="flex flex-wrap items-center justify-end gap-2"
          >
            <p>
              {seat(request.offer.player)} wants to take back {action}
            </p>
            <span data-slot="take-back-clock" className="text-muted-foreground tabular-nums">
              {left}
            </span>
            <Button size="sm" disabled={!online} onClick={() => session.voteTakeBack(true)}>
              Allow
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={!online}
              onClick={() => session.voteTakeBack(false)}
            >
              Decline
            </Button>
          </div>
        ) : (
          <p data-slot="take-back-waiting" className="text-muted-foreground">
            {me === request.offer.player
              ? `Taking back ${action}. Waiting for the table…`
              : `${seat(request.offer.player)} wants to take back ${action}. Waiting for the table…`}{' '}
            <span className="tabular-nums">{left}</span>
          </p>
        )}
      </div>
    )
  }

  const canAsk = offer !== null && playing && offer.player === me
  if (!canAsk && noticeLine === false) return null
  return (
    <div data-slot="take-back" className="flex flex-col items-end gap-1">
      {noticeLine}
      {canAsk && (
        <Button
          size="sm"
          variant="outline"
          className="max-sm:h-9 max-sm:px-3"
          disabled={!online}
          onClick={() => session.takeBack()}
        >
          Take back {takeBackAction(offer.kind, offer.name)}
        </Button>
      )}
    </div>
  )
}
