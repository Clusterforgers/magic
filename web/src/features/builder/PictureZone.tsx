import { useId } from 'react'
import type { FixMarksApi } from '@/net/fixMarks'
import type { DeckZone } from '@/protocol/generated'
import { CardTile } from './CardTile'
import { groupLines } from './deckSummary'
import type { Line } from './deckEdit'
import type { LineActions } from './LineControls'
import { checkOf, lineKey } from './report'
import type { LineCheck, SortKey } from './report'
import { ZONE_TITLE } from './zones'

interface PictureZoneProps {
  zone: DeckZone
  /** The lines to draw, already filtered and ordered by `arrange`. */
  lines: readonly Line[]
  sort: SortKey
  format: string
  room: boolean
  checks: ReadonlyMap<string, LineCheck>
  /** The check's sentence for each line outside the commanders' colour identity, by `lineKey`. */
  outsideByLine: ReadonlyMap<string, string>
  actions: LineActions
  fixMarks?: FixMarksApi
}

/**
 * The lines of one zone as pictures in groups by type, each group with its heading and copies: a grid of cards large
 * enough to read, with the commanders and the rest drawn the same way.
 */
export function PictureZone({
  zone,
  lines,
  sort,
  format,
  room,
  checks,
  outsideByLine,
  actions,
  fixMarks,
}: PictureZoneProps) {
  const id = useId()
  return (
    <div data-slot="picture-zone" className="grid gap-4">
      {groupLines(lines, sort).map((group) => (
        <section
          key={group.kind}
          aria-labelledby={`${id}-${group.kind}`}
          data-group={group.kind}
          className="grid gap-2"
        >
          <h4
            id={`${id}-${group.kind}`}
            className="flex items-baseline justify-between gap-2 text-xs font-medium text-muted-foreground"
          >
            {group.label}
            <span className="tabular-nums">{group.count}</span>
          </h4>
          <ul
            aria-label={`${ZONE_TITLE[zone]} ${group.label.toLowerCase()}`}
            className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-x-2.5 gap-y-3 pt-1.5"
          >
            {group.lines.map((line) => (
              <CardTile
                key={line.card.oracle_id}
                line={line}
                format={format}
                room={room}
                check={checkOf(checks, line)}
                outside={outsideByLine.get(lineKey(line)) ?? null}
                actions={actions}
                fixMarks={fixMarks}
              />
            ))}
          </ul>
        </section>
      ))}
    </div>
  )
}
