import { CountedChoice } from '../CountedChoice'
import type { PromptComponent } from '../types'

/** BottomCards: choose exactly `count` cards of your hand to put on the bottom of your library. */
export const BottomCardsPrompt: PromptComponent<'BottomCards'> = ({ options }) => (
  <CountedChoice
    offered={options.hand}
    count={options.count}
    what="put on the bottom of your library"
    answerWith={(chosen) => ({ BottomCards: chosen })}
  />
)
