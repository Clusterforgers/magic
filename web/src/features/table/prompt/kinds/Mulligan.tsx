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
 * Mulligan: keep the opening hand or take a mulligan. On a phone the hand is a row of cards among the band's choices (`OpeningHand`); the sentence says what a
 * mulligan costs and how many were taken, and a line beneath names the other seats still deciding. Where the table floats
 * the decision (every table off a phone, `useOpeningOverlay`) it stands in the middle of the table over the dimmed boards: the hand in
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
  const buttons = (
    <>
      <Button className="h-10 px-4" onClick={() => answer({ Mulligan: 'Keep' })}>
        Keep
      </Button>
      <Button
        variant="outline"
        className="h-10 px-4"
        disabled={!options.can_mulligan}
        onClick={() => answer({ Mulligan: 'Mulligan' })}
      >
        Mulligan
      </Button>
      {others.length === 0 || floating ? null : (
        <p data-slot="mulligan-waiting" className="basis-full text-xs text-muted-foreground">
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
      className="plaque flex max-h-full max-w-[94%] flex-col items-center gap-3 overflow-y-auto rounded-xl border-2 border-brass p-4 text-center shadow-e3 [&_[data-slot=prompt-actions]]:justify-center [&_[data-slot=prompt-actions]]:bg-transparent"
    >
      <OpeningHand card={openingCardPx(room, cards.length)} />
      <div className="flex max-w-[70ch] flex-col items-center gap-2">
        <PromptLayout text={mulliganText(options)}>{buttons}</PromptLayout>
      </div>
      <OpeningStatus floating />
    </div>,
    host,
  )
}
