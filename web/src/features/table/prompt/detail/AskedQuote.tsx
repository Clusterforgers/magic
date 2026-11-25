import { Quote } from 'lucide-react'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { RulesText } from '../../RulesText'
import { usePrompt } from '../PromptContext'
import { askedWords } from './asking'
import { MarkedRules } from './DecisionDetail'
import { useAsker } from './useAsker'

/** The decisions whose choices already are the asked clause (the modes) or that ask about no clause (a payment). */
const UNQUOTED: ReadonlySet<string> = new Set(['ChooseMode', 'PayMana'])

/**
 * The clause of the asking card's text the decision asks about, quoted in a home that does not draw the card (the dock,
 * a list's window, a phone's bar); nothing when the server marked no clause. A phone's bar has no line for it: there it
 * takes the rest of the question's line, cut to one line, and a press opens the card's words with the clause marked.
 */
export function AskedQuote({ where }: { where: 'line' | 'window' | 'bar' }) {
  const { name } = usePrompt()
  const asker = useAsker()
  const text = asker?.text
  const words = text === undefined || UNQUOTED.has(name) ? null : askedWords(text, asker?.clause)
  if (words === null || text === undefined) return null
  const shown = asker?.shown || 'The card'
  if (where === 'bar') {
    return (
      <Popover>
        <PopoverTrigger
          data-slot="asked-quote"
          data-where="bar"
          title={words}
          aria-label={`${shown} asks about: ${words}. Show its words.`}
          className="asked-quote asked-quote-bar"
        >
          <Quote aria-hidden="true" className="size-3.5 shrink-0" />
          <span data-slot="asked-quote-words" className="min-w-0 truncate">
            {words}
          </span>
        </PopoverTrigger>
        <PopoverContent side="top" align="start" className="gap-1.5 p-3" data-slot="asked-quote-whole">
          <PopoverTitle>{shown}</PopoverTitle>
          <MarkedRules text={text} clause={asker?.clause} />
        </PopoverContent>
      </Popover>
    )
  }
  return (
    <p
      data-slot="asked-quote"
      data-where={where}
      title={words}
      className="asked-quote line-clamp-2 min-w-0 basis-full whitespace-pre-line"
    >
      <span className="sr-only">{`${shown} asks about: `}</span>
      <RulesText text={words} />
    </p>
  )
}
