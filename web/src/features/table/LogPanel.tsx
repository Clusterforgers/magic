import { memo, useLayoutEffect, useMemo, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { foldLog } from './logFold'
import type { FoldedLine } from './logFold'
import { useSeatNamer } from './names'
import { SeatDot } from './PodParts'

const LogLine = memo(function LogLine({ line }: { line: FoldedLine }) {
  const { text, minor, passed, spoken } = line
  return (
    <li
      className={cn(
        'px-1 py-0.5',
        minor && 'text-muted-foreground',
        passed.length > 0 && 'flex items-center gap-1.5',
      )}
    >
      {passed.length === 0 ? null : (
        <span data-slot="log-seats" className="flex shrink-0 items-center gap-0.5">
          {passed.map((seat) => (
            <SeatDot key={seat} seat={seat} />
          ))}
        </span>
      )}
      <span>{text}</span>
      {spoken === '' ? null : <span className="sr-only">: {spoken}</span>}
    </li>
  )
})

// Within this many pixels of the bottom the log follows new lines; further up it stays where the reader is.
const FOLLOW_SLACK = 24

/**
 * The game log: one sentence for every event worth one and one line for each round of passes (`foldLog`), oldest
 * first, following the newest line unless scrolled up.
 */
export function LogPanel() {
  const log = usePlay((state) => state.log)
  const objects = usePlay((state) => state.objectsById)
  const seat = useSeatNamer()
  const players = usePlay((state) => state.view?.players)
  const seats = usePlay(
    useShallow(
      (state) =>
        state.view?.players.filter((player) => player.lost === null).map((player) => player.seat) ?? [],
    ),
  )
  const lines = useMemo(
    () => foldLog(log, { seat, card: (id) => objects[id]?.name || 'a card' }, { seats, players }),
    [log, objects, seat, seats, players],
  )
  const scroller = useRef<HTMLDivElement>(null)
  const following = useRef(true)
  // Keyed on the newest line, not the count: a full log (`LOG_LIMIT`) keeps its length while lines arrive.
  const newest = lines.at(-1)?.seq
  useLayoutEffect(() => {
    const element = scroller.current
    if (element !== null && following.current) element.scrollTop = element.scrollHeight
  }, [newest])
  return (
    <div
      ref={scroller}
      role="log"
      aria-label="Game log"
      aria-live="off"
      tabIndex={0}
      onScroll={(event) => {
        const { scrollHeight, scrollTop, clientHeight } = event.currentTarget
        following.current = scrollHeight - scrollTop - clientHeight < FOLLOW_SLACK
      }}
      className="h-full overflow-y-auto rounded-md px-1 text-sm outline-primary focus-visible:outline-2"
    >
      {lines.length === 0 ? (
        <p className="p-2 text-muted-foreground">Nothing has happened yet.</p>
      ) : (
        <ol className="grid">
          {lines.map((line) => (
            <LogLine key={line.seq} line={line} />
          ))}
        </ol>
      )}
    </div>
  )
}
