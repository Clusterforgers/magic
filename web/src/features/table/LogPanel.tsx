import { memo, useLayoutEffect, useMemo, useRef } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { describeEvent } from './eventText'
import { useSeatNamer } from './names'

const LogLine = memo(function LogLine({ text, minor }: { text: string; minor: boolean }) {
  return <li className={cn('px-1 py-0.5', minor && 'text-muted-foreground')}>{text}</li>
})

// Within this many pixels of the bottom the log follows new lines; further up it stays where the reader is.
const FOLLOW_SLACK = 24

/** The game log: one sentence for every event, oldest first, following the newest line unless scrolled up. */
export function LogPanel() {
  const log = usePlay((state) => state.log)
  const objects = usePlay((state) => state.objectsById)
  const seat = useSeatNamer()
  const lines = useMemo(
    () =>
      log.map(({ seq, event }) => ({
        seq,
        ...describeEvent(event, { seat, card: (id) => objects[id]?.name ?? 'a card' }),
      })),
    [log, objects, seat],
  )
  const scroller = useRef<HTMLDivElement>(null)
  const following = useRef(true)
  useLayoutEffect(() => {
    const element = scroller.current
    if (element !== null && following.current) element.scrollTop = element.scrollHeight
  }, [lines.length])
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
            <LogLine key={line.seq} text={line.text} minor={line.minor} />
          ))}
        </ol>
      )}
    </div>
  )
}
