import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import { bringLabel } from '../companion'
import { playerOf } from '../selectors'
import { usePrompt } from './PromptContext'

/** The special action that puts the viewer's companion into their hand for {3}; drawn only while the server offers it. */
export function BringCompanion({ offered }: { offered: boolean }) {
  const { answer } = usePrompt()
  const name = usePlay((state) =>
    state.seat === null ? undefined : playerOf(state, state.seat)?.companion?.name,
  )
  if (!offered) return null
  return (
    <Button
      variant="outline"
      size="sm"
      data-slot="bring-companion"
      onClick={() => answer({ Priority: 'BringCompanion' })}
    >
      {bringLabel(name)}
    </Button>
  )
}
