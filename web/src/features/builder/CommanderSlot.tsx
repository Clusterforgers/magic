import { Trash2Icon, UserRoundSearchIcon } from 'lucide-react'
import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { CardPicture } from '@/features/cards/CardPicture'
import { SupportMark, supportEdge } from '@/features/decks/SupportMark'
import { cn } from '@/lib/utils'
import type { CardHit } from '@/protocol/generated'
import type { Line } from './deckEdit'
import { identityWords } from './identity'
import { IdentityPips } from './IdentityPips'
import { isIssue } from './report'
import type { LineCheck } from './report'

/** What the slot can ask the search to look for. */
export type FindKind = 'commander' | 'partner' | 'background'

interface CommanderSlotProps {
  /** The lines of the command zone, in name order. */
  lines: readonly Line[]
  /** The identity of the commanders together. */
  identity: readonly string[]
  /** How many cards of the deck lie outside that identity, by the deck check. */
  outside: number
  /** The card whose choice is being checked, if one is. */
  judging: string | null
  /** What the deck check says of a line. */
  checkOf: (line: Line) => LineCheck
  onRemove: (card: CardHit) => void
  /** Opens the preview of a card; `opener` is the control focus returns to. */
  onPreview: (card: CardHit, opener: HTMLElement) => void
  /** Sets the search to look for a kind of card and takes the person to it. */
  onFind: (kind: FindKind) => void
}

function Tile({
  line,
  check,
  onRemove,
  onPreview,
}: {
  line: Line
  check: LineCheck
  onRemove: () => void
  onPreview: (opener: HTMLElement) => void
}) {
  const { card, count } = line
  const issue = isIssue(check.support)
  return (
    <li
      data-line={`commander:${card.oracle_id}`}
      data-commander={card.oracle_id}
      data-support={check.support}
      className={cn(
        'flex min-w-0 items-start gap-2 rounded-lg border border-l-4 p-2',
        issue ? supportEdge(check.support) : 'border-l-border',
      )}
    >
      <CardPicture oracleId={card.oracle_id} name={card.name} size="normal" alt="" className="w-20 sm:w-28" />
      <div className="grid min-w-0 flex-1 content-start gap-1">
        <button
          type="button"
          aria-haspopup="dialog"
          title={card.name}
          data-slot="card-name"
          onClick={(event) => onPreview(event.currentTarget)}
          className="min-h-6 min-w-0 truncate rounded-sm text-left text-sm font-semibold underline-offset-4 hover:underline pointer-coarse:min-h-11"
        >
          {card.name}
          {count > 1 ? <span className="font-normal text-muted-foreground"> ×{count}</span> : null}
        </button>
        <span className="truncate text-xs text-muted-foreground">{card.type_line}</span>
        <IdentityPips identity={card.color_identity} />
        {issue ? <SupportMark support={check.support} className="justify-self-start" /> : null}
      </div>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={`Remove ${card.name} from the command zone`}
        onClick={onRemove}
      >
        <Trash2Icon aria-hidden="true" />
      </Button>
    </li>
  )
}

/**
 * The command zone of a Commander deck as a slot: one commander, or two that can be partnered (Partner, Friends
 * forever, a Doctor's companion with a Time Lord Doctor, or a creature that chooses a Background with its Background),
 * each as a picture, name, type and identity. An empty slot says what may go there and starts the search for it; a
 * choice the deck check refuses never gets here (`commanderCheck.ts`). The identity of the commanders and the number
 * of cards outside it sit under the heading.
 */
export function CommanderSlot({
  lines,
  identity,
  outside,
  judging,
  checkOf,
  onRemove,
  onPreview,
  onFind,
}: CommanderSlotProps) {
  const id = useId()
  const first = lines.length === 0
  const total = lines.reduce((sum, line) => sum + line.count, 0)
  const find = (kind: FindKind, label: string) => (
    <Button key={kind} type="button" variant="outline" size="sm" onClick={() => onFind(kind)}>
      <UserRoundSearchIcon aria-hidden="true" />
      {label}
    </Button>
  )
  return (
    <section aria-labelledby={`${id}-commander`} data-slot="commander-slot" className="grid gap-2">
      <h3
        id={`${id}-commander`}
        className="flex items-baseline justify-between gap-2 border-b pb-1 text-sm font-semibold"
      >
        Commander <span className="font-normal text-muted-foreground tabular-nums">{total}</span>
      </h3>
      {first ? null : (
        <p
          data-slot="identity-line"
          className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground"
        >
          <IdentityPips identity={identity} />
          <span>{identityWords(identity)} identity</span>
          <span data-slot="outside" className={cn(outside > 0 && 'font-medium text-destructive')}>
            {outside === 0
              ? 'Every card lies within it.'
              : `${outside} ${outside === 1 ? 'card lies' : 'cards lie'} outside it.`}
          </span>
        </p>
      )}
      <ul aria-label="Commander cards" className="grid gap-2 sm:grid-cols-2">
        {lines.map((line) => (
          <Tile
            key={line.card.oracle_id}
            line={line}
            check={checkOf(line)}
            onRemove={() => onRemove(line.card)}
            onPreview={(opener) => onPreview(line.card, opener)}
          />
        ))}
        {first ? (
          <li
            className="grid min-w-0 content-start gap-1 rounded-lg border border-dashed p-2 sm:col-span-2"
            data-slot="open-seat"
          >
            <p className="text-sm font-medium">No commander yet</p>
            <p className="text-xs text-muted-foreground">
              A legendary creature, or a card that says it can be your commander. Use the menu beside a card
              in the search, or find one.
            </p>
            <div className="flex flex-wrap gap-1">{find('commander', 'Find a commander')}</div>
          </li>
        ) : null}
        {lines.length === 1 && total === 1 ? (
          <li
            className="grid min-w-0 content-start gap-1 rounded-lg border border-dashed p-2"
            data-slot="open-seat"
          >
            <p className="text-sm font-medium">Partner or Background</p>
            <p className="text-xs text-muted-foreground">
              Optional: a second commander with Partner, Friends forever or Doctor’s companion, or a
              Background for a creature that chooses one.
            </p>
            <div className="flex flex-wrap gap-1">
              {find('partner', 'Find a partner')}
              {find('background', 'Find a Background')}
            </div>
          </li>
        ) : null}
      </ul>
      <p role="status" className="sr-only">
        {judging === null ? '' : `Checking ${judging} as a commander…`}
      </p>
    </section>
  )
}
