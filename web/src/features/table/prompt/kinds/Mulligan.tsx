import { anchor } from '@/features/learn/anchors'
import { useLayoutEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { createPortal } from 'react-dom'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from '../../names'
import { OpeningStatus } from '../../OpeningStatus'
import { useOpeningOverlay, useOverlayRoom } from '../choiceOverlay'
import { decidingText, mulliganText, othersDeciding } from '../mulliganText'
import { usePrompt } from '../PromptContext'
import { OpeningHand, openingHandIds } from '../OpeningHand'
import { openingCardPx } from '../openingCard'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'

/**
 * The height in px the floating panel takes beside its row of cards (padding, sentence, answers, status), in steps of
 * 8 px; 0 until it is drawn. The cards get only the rest of the room, so the panel never scrolls its answers over the
 * sentence.
 */
function usePanelRest(panel: RefObject<HTMLElement | null>, host: HTMLElement | null): number {
  const [rest, setRest] = useState(0)
  useLayoutEffect(() => {
    const element = panel.current
    if (element === null || host === null) return undefined
    const measure = (): void => {
      const row = element.querySelector<HTMLElement>('[data-slot="opening-hand"]')
      const style = getComputedStyle(element)
      const borders = (parseFloat(style.borderTopWidth) || 0) + (parseFloat(style.borderBottomWidth) || 0)
      const next = Math.ceil((element.scrollHeight + borders - (row?.offsetHeight ?? 0)) / 8) * 8
      setRest((before) => (before === next ? before : next))
    }
    measure()
    if (typeof ResizeObserver !== 'function') return undefined
    const observer = new ResizeObserver(measure)
    observer.observe(element)
    for (const child of element.children) observer.observe(child)
    return () => observer.disconnect()
  }, [panel, host])
  return rest
}

/**
 * Mulligan: keep the opening hand or take a mulligan. On a phone the hand is a row of cards among the band's choices (`OpeningHand`); the sentence says what a
 * mulligan costs and how many were taken, and a short line left of the answers names the other seats still deciding, in
 * place of the strip's list of every seat (`promptBand.css`). Where the table floats
 * the decision (every table off a phone, `useOpeningOverlay`) it stands centred over the whole table (`openingPlace`), the opponents dimmed: the hand in
 * one row of large cards, then the sentence and the buttons, then every seat's progress.
 */
export const MulliganPrompt: PromptComponent<'Mulligan'> = ({ options }) => {
  const { answer, name: decision } = usePrompt()
  const name = useSeatNamer()
  const others = usePlay(useShallow(othersDeciding))
  const host = useOpeningOverlay()
  const room = useOverlayRoom()
  const cards = usePlay(useShallow(openingHandIds))
  const floating = host !== null
  const panel = useRef<HTMLDivElement>(null)
  const rest = usePanelRest(panel, host)
  const buttons = (
    <>
      <Button className="h-10 px-4" {...anchor('opening.keep')} onClick={() => answer({ Mulligan: 'Keep' })}>
        Keep
      </Button>
      <Button
        variant="outline"
        className="h-10 px-4"
        disabled={!options.can_mulligan}
        {...anchor('opening.mulligan')}
        onClick={() => answer({ Mulligan: 'Mulligan' })}
      >
        Mulligan
      </Button>
      {others.length === 0 || floating ? null : (
        // Beside the answers on their row, cut with an ellipsis where it is long: whole in its tooltip.
        <p
          data-slot="mulligan-waiting"
          title={decidingText(others.map(name))}
          className="min-w-0 truncate text-sm text-muted-foreground"
        >
          {decidingText(others.map(name))}
        </p>
      )}
    </>
  )
  if (host === null) {
    return (
      <PromptLayout text={mulliganText(options)} choices={<OpeningHand />}>
        {buttons}
      </PromptLayout>
    )
  }
  return createPortal(
    <div
      data-slot="choice-overlay"
      data-decision={decision}
      data-opening=""
      ref={panel}
      className="plaque flex max-h-full max-w-full flex-col items-center gap-3 overflow-y-auto rounded-xl border-2 border-brass p-4 text-center shadow-e3 [&_[data-slot=prompt-actions]]:justify-center [&_[data-slot=prompt-actions]]:bg-transparent"
    >
      <OpeningHand card={openingCardPx(room, cards.length, rest)} />
      {/*
        The sentence beside the answers, on a width of its own: the cards get the height a line of answers would take,
        and their size never changes how the sentence wraps (`usePanelRest`).
      */}
      <div className="grid w-[80ch] max-w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-x-4 gap-y-1 text-left [&>[data-slot=prompt-title]]:col-span-2 [&>[data-slot=prompt-title]]:text-center">
        <PromptLayout text={mulliganText(options)}>{buttons}</PromptLayout>
      </div>
      <OpeningStatus floating />
    </div>,
    host,
  )
}
