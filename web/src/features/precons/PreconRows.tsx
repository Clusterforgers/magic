import { PlayIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useMediaQuery } from '@/hooks/useMediaQuery'
import { cn } from '@/lib/utils'
import type { PreconListItem } from '@/protocol/generated'
import { PreconBadge } from './PreconBadge'
import { commandersText, releaseText, statusOf } from './format'

/** The window from which the precons are a table with columns; under it each is a stacked row. */
export const TABLE_FROM = '(min-width: 64rem)'

/** What both layouts need to draw the rows. */
export interface RowsProps {
  rows: readonly PreconListItem[]
  /** Opens the details of a precon; `opener` is the button that asked. */
  onDetails(precon: PreconListItem, opener: HTMLElement): void
  /** Starts a game with a precon; `opener` is the button that asked. */
  onPlay(precon: PreconListItem, opener: HTMLElement): void
  /** The code of the precon whose game is being made, or null; that Play spins. */
  playing: string | null
  /** Some game is being made: every Play is disabled while one is. */
  busy: boolean
}

const statusId = (precon: PreconListItem): string => `precon-status-${precon.code}`
const cardsLabel = (precon: PreconListItem): string => `${precon.card_count} cards`

/** The Details and Play buttons of a row. Play of a deck the game cannot take stays in place, disabled and explained. */
function Actions({ precon, onDetails, onPlay, playing, busy }: RowsProps & { precon: PreconListItem }) {
  const blocked = statusOf(precon) === 'blocked'
  return (
    <div className="flex items-center gap-2">
      <Button
        type="button"
        variant="outline"
        data-details={precon.code}
        aria-label={`Details of ${precon.name}`}
        onClick={(event) => onDetails(precon, event.currentTarget)}
      >
        Details
      </Button>
      <Button
        type="button"
        data-play={precon.code}
        aria-label={`Play ${precon.name}`}
        aria-describedby={blocked ? statusId(precon) : undefined}
        disabled={blocked || busy}
        loading={playing === precon.code}
        // A disabled button that keeps focus does not drop it when another Play starts while it is the one in hand.
        focusableWhenDisabled
        onClick={(event) => onPlay(precon, event.currentTarget)}
      >
        {playing === precon.code ? null : <PlayIcon aria-hidden="true" />}
        Play
      </Button>
    </div>
  )
}

/** The badge and, for a deck the game cannot take, the way to the reasons. */
function Check({ precon }: { precon: PreconListItem }) {
  return (
    <div className="grid justify-items-start gap-1">
      <PreconBadge precon={precon} id={statusId(precon)} />
      {statusOf(precon) === 'blocked' ? (
        <span className="text-xs whitespace-nowrap text-muted-foreground">Details list what stops it.</span>
      ) : null}
    </div>
  )
}

const HEAD =
  'sticky top-0 z-10 border-b bg-surface px-4 py-2 text-left text-xs font-medium text-muted-foreground'
const CELL = 'border-b px-4 py-3 align-middle text-sm'

/** From the L breakpoint up: a real table, its header kept in view while the rows scroll. */
function PreconTable(props: RowsProps) {
  return (
    <table className="w-full border-separate border-spacing-0 text-left">
      <caption className="sr-only">Precons, newest release first</caption>
      <thead>
        <tr>
          <th scope="col" className={HEAD}>
            Deck
          </th>
          <th scope="col" className={HEAD}>
            Set
          </th>
          <th scope="col" className={HEAD}>
            Commanders
          </th>
          <th scope="col" className={cn(HEAD, 'text-right')}>
            Cards
          </th>
          <th scope="col" className={HEAD}>
            Deck check
          </th>
          <th scope="col" className={cn(HEAD, 'text-right')}>
            Action
          </th>
        </tr>
      </thead>
      <tbody className="[&>tr:last-child>*]:border-b-0">
        {props.rows.map((precon) => (
          <tr key={precon.code} data-precon={precon.code} data-status={statusOf(precon)}>
            <th scope="row" className={cn(CELL, 'font-medium break-words')}>
              {precon.name}
              {precon.deck_type === null ? null : (
                <span className="block text-xs font-normal text-muted-foreground">{precon.deck_type}</span>
              )}
            </th>
            <td className={cn(CELL, 'whitespace-nowrap')}>
              <span className="font-medium">{precon.set}</span>
              <span className="block text-xs text-muted-foreground">{releaseText(precon.release_date)}</span>
            </td>
            <td className={cn(CELL, 'break-words text-muted-foreground')}>
              {commandersText(precon.commanders)}
            </td>
            <td className={cn(CELL, 'text-right tabular-nums')}>{precon.card_count}</td>
            <td className={CELL}>
              <Check precon={precon} />
            </td>
            <td className={cn(CELL, 'text-right')}>
              <div className="flex justify-end">
                <Actions {...props} precon={precon} />
              </div>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/** Under the L breakpoint: one stacked row a precon, its buttons under the words. */
function PreconList(props: RowsProps) {
  return (
    <ul aria-label="Precons, newest release first" className="grid">
      {props.rows.map((precon) => (
        <li
          key={precon.code}
          data-precon={precon.code}
          data-status={statusOf(precon)}
          className="grid gap-2 border-b px-4 py-3 last:border-b-0"
        >
          <div className="grid min-w-0 gap-1">
            <p className="text-base font-medium break-words">{precon.name}</p>
            <p className="text-sm break-words text-muted-foreground">
              {[precon.set, releaseText(precon.release_date), precon.deck_type, cardsLabel(precon)]
                .filter((part) => part !== null)
                .join(' · ')}
            </p>
            {precon.commanders.length === 0 ? null : (
              <p className="text-sm break-words text-muted-foreground">{commandersText(precon.commanders)}</p>
            )}
          </div>
          <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
            <Check precon={precon} />
            <Actions {...props} precon={precon} />
          </div>
        </li>
      ))}
    </ul>
  )
}

/** The rows as a table from the L breakpoint up and as stacked rows under it; only the one that applies is in the page. */
export function PreconRows(props: RowsProps) {
  const wide = useMediaQuery(TABLE_FROM, true)
  return wide ? <PreconTable {...props} /> : <PreconList {...props} />
}
