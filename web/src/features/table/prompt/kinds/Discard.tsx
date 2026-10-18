import { CountedChoice } from '../CountedChoice'
import type { PromptComponent } from '../types'

/** Discard: at cleanup, choose exactly `count` of the offered cards to discard; the rest of your cards are dimmed. */
export const DiscardPrompt: PromptComponent<'Discard'> = ({ options }) => (
  <CountedChoice
    offered={options.hand}
    count={options.count}
    what="discard"
    intro="Your hand is over the limit. "
    answerWith={(chosen) => ({ Discard: chosen })}
  />
)
