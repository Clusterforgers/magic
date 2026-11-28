import { useEffect } from 'react'
import { Undo2 } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import { useDockForm } from './dockLayout'
import { KeyHint } from './prompt/KeyHint'
import { isFocusFree } from './prompt/usePrimaryKey'
import { GO_BACK_KEY, GO_BACK_REFUSAL_WORDS, goBackLabel } from './goBackText'
import { TAKE_BACK_NOTICE_MS } from './takeBackText'

/**
 * Going back a step at the table: "Back to Precombat main" while the server offers the seat an earlier step it passed
 * in (Backspace too, while nothing has the keyboard), and a short line saying why a go-back was refused, which goes by
 * itself.
 */
export function GoBackControl() {
  const { session } = useGame()
  const { offer, refused, me, online, playing } = usePlay(
    useShallow((state) => ({
      offer: state.goBackOffer,
      refused: state.goBackRefused,
      me: state.seat,
      online: state.status === 'open',
      playing: state.phase === 'playing',
    })),
  )
  const form = useDockForm()
  const canGo = offer !== null && playing && online && offer.player === me

  useEffect(() => {
    if (refused === null) return
    const timer = window.setTimeout(() => session.dismissGoBackRefusal(), TAKE_BACK_NOTICE_MS)
    return () => window.clearTimeout(timer)
  }, [refused, session])

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

  if (!canGo && refused === null) return null
  return (
    <div data-slot="go-back" className="flex max-w-full flex-col items-end gap-1">
      {refused !== null && (
        <div data-slot="go-back-notice" className="flex items-center gap-2 text-sm text-muted-foreground">
          <p role="status" aria-live="polite">
            {GO_BACK_REFUSAL_WORDS[refused]}
          </p>
          <Button size="sm" variant="ghost" onClick={() => session.dismissGoBackRefusal()}>
            OK
          </Button>
        </div>
      )}
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
