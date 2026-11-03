import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { pieces } from '@/lib/glossary'
import type { Term } from '@/lib/glossary'

/** A keyword word with its reminder text on hover, focus or press. */
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
