import { Pip } from './ManaPips'
import { rulesPieces } from './rulesText'

/** A typecycling keyword may break before "cycling" on a narrow card ("Plains-" over "cycling"), as a soft hyphen allows. */
const breakable = (text: string): string => text.replace(/(\p{L})(cycling)/gu, '$1­$2')

/**
 * The play on the foot of a hand card that cannot be cast now but has an ability to use ("Plainscycling {2}"), mana as
 * the card's discs. It lies over the card, so it moves nothing when it comes or goes; where a row of the hand lies over
 * the card's foot (`HandSlot` `data-back`) it sits at the bottom of the head that shows. Decoration: the card's spoken
 * name says the play.
 */
export function AbilityCue({ text }: { text: string }) {
  return (
    <span
      aria-hidden="true"
      data-slot="card-ability"
      className="pointer-events-none absolute inset-x-0.5 bottom-0.5 z-20 flex flex-wrap items-center justify-center gap-x-0.5 rounded-sm bg-background px-0.5 py-px text-center text-xs leading-4 font-semibold text-foreground shadow-sm ring-1 ring-primary/70 group-data-[back]/slot:bottom-[calc(var(--card-h)_-_52px)] group-data-[back]/slot:flex-nowrap"
    >
      {rulesPieces(text).map((piece, index) =>
        piece.kind === 'mana' ? (
          <Pip key={index} pip={piece.pip} sizeClass="size-4 text-xs" />
        ) : (
          // Over a covered foot it keeps one line under the name and cost, the keyword cut short where it must be.
          <span
            key={index}
            className="max-w-full min-w-0 shrink-0 hyphens-manual [overflow-wrap:anywhere] group-data-[back]/slot:shrink group-data-[back]/slot:truncate"
          >
            {breakable(piece.text.trim())}
          </span>
        ),
      )}
    </span>
  )
}
