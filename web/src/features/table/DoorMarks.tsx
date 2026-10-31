/** The door badge of a Room permanent: one mark per door with its name and a lock, open once it is unlocked. */
import { Lock, LockOpen } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { Door, Doors } from './rooms'

// An unlocked door is a filled mark, a locked one the dashed outline of an empty slot (never colour alone: the lock says it too).
const OPEN = 'bg-foreground text-background'
const SHUT = 'border border-dashed border-foreground bg-background text-foreground'

// The foot stack's marks are 16 px lines, the text face's sit in its art slot.
const BASE = {
  foot: 'flex h-fit max-w-full items-center gap-0.5 rounded-sm px-1 leading-4 font-semibold whitespace-nowrap shadow-sm',
  art: 'inline-flex h-fit max-w-full items-center gap-0.5 rounded-sm px-1 leading-snug font-medium',
}

interface DoorMarksProps {
  doors: Doors
  /** A compact face shows both locks in one mark; the names stay for a screen reader. */
  compact: boolean
  /** The text size class of the face. */
  text: string
  /** Where the marks sit: the foot stack of a picture or the art slot of a text face. */
  spot: 'foot' | 'art'
}

/** The lock of a door: open once it is unlocked. */
const LockOf = ({ door }: { door: Door }) =>
  door.unlocked ? (
    <LockOpen aria-hidden="true" className="size-3 shrink-0" />
  ) : (
    <Lock aria-hidden="true" className="size-3 shrink-0" />
  )

/**
 * One mark for each door, in the order of the card, with the state in words after the name for a screen reader. A
 * compact face has no room for two lines: its two locks share one mark.
 */
export function DoorMarks({ doors, compact, text, spot }: DoorMarksProps) {
  if (compact) {
    return (
      <span
        data-slot="card-doors"
        className={cn(BASE[spot], 'border border-foreground bg-background text-foreground', text)}
      >
        {doors.map((door, at) => (
          <span
            key={at}
            data-slot="card-door"
            data-door={at}
            data-unlocked={door.unlocked}
            className="inline-flex items-center"
          >
            <LockOf door={door} />
            <span className="sr-only">{`${door.name}, ${door.unlocked ? 'unlocked' : 'locked'}`}</span>
          </span>
        ))}
      </span>
    )
  }
  return doors.map((door, at) => (
    <span
      key={at}
      data-slot="card-door"
      data-door={at}
      data-unlocked={door.unlocked}
      className={cn(BASE[spot], door.unlocked ? OPEN : SHUT, text)}
    >
      <LockOf door={door} />
      <span className="truncate">{door.name}</span>
      <span className="sr-only">{door.unlocked ? ', unlocked' : ', locked'}</span>
    </span>
  ))
}
