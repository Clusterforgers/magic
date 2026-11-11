import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { pieces } from '@/lib/glossary'
import { showLexicon } from './lexicon/lexiconStore'
import type { Term } from '@/lib/glossary'

/** A keyword word with its reminder text on hover, focus or press, and a way into the lexicon. */
export function KeywordWord({ term, children }: { term: Term; children: string }) {
  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        delay={250}
        render={<span role="button" tabIndex={0} data-slot="keyword-word" />}
        className="cursor-help underline decoration-dotted underline-offset-2"
      >
        {children}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-64 gap-1 p-2 text-sm">
        <PopoverTitle>{term.title}</PopoverTitle>
        <p data-slot="keyword-reminder">{term.text}</p>
        <button
          type="button"
          data-slot="keyword-lexicon"
          onClick={() => showLexicon(term.title)}
          className="justify-self-start rounded-sm text-xs text-primary underline-offset-2 hover:underline"
        >
          More in the lexicon
        </button>
      </PopoverContent>
    </Popover>
  )
}

/** A text (a card's ability, a log line) with each keyword word in it made into a `KeywordWord`. */
export function KeywordText({ text }: { text: string }) {
  return (
    <span>
      {pieces(text).map((piece, index) =>
        piece.term === undefined ? (
          <span key={index}>{piece.text}</span>
        ) : (
          <KeywordWord key={index} term={piece.term}>
            {piece.text}
          </KeywordWord>
        ),
      )}
    </span>
  )
}
