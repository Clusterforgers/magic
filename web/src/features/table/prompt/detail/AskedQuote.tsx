import { RulesText } from '../../RulesText'
import { usePrompt } from '../PromptContext'
import { askedWords } from './asking'
import { useAsker } from './useAsker'

/** The decisions whose choices already are the asked clause (the modes) or that ask about no clause (a payment). */
const UNQUOTED: ReadonlySet<string> = new Set(['ChooseMode', 'PayMana'])

/**
 * The clause of the asking card's text the decision asks about, quoted in a home that does not draw the card (the dock,
 * a list's window); nothing when the server marked no clause.
 */
export function AskedQuote({ where }: { where: 'line' | 'window' }) {
  const { name } = usePrompt()
  const asker = useAsker()
  const words = asker?.text === undefined || UNQUOTED.has(name) ? null : askedWords(asker.text, asker.clause)
  if (words === null) return null
  return (
    <p
      data-slot="asked-quote"
      data-where={where}
      title={words}
      className="asked-quote line-clamp-2 min-w-0 basis-full whitespace-pre-line"
    >
      <span className="sr-only">{`${asker?.shown ?? 'The card'} asks about: `}</span>
      <RulesText text={words} />
    </p>
  )
}
