import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from '../../names'
import { decidingText, mulliganText, othersDeciding } from '../mulliganText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'

/**
 * Mulligan: keep the opening hand or take a mulligan. The hand stays in its strip, unmarked; the sentence says what a
 * mulligan costs and how many were taken, and a line beneath names the other seats still deciding.
 */
export const MulliganPrompt: PromptComponent<'Mulligan'> = ({ options }) => {
  const { answer } = usePrompt()
  const name = useSeatNamer()
  const others = usePlay(useShallow(othersDeciding))
  return (
    <PromptLayout text={mulliganText(options)}>
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
      {others.length === 0 ? null : (
        <p data-slot="mulligan-waiting" className="basis-full text-xs text-muted-foreground">
          {decidingText(others.map(name))}
        </p>
      )}
    </PromptLayout>
  )
}
