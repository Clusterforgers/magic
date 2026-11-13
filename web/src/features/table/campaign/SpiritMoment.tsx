/**
 * The moment a hero falls: a full-screen card over the table, "You are a Restless Spirit", with the few lines that say
 * what a Spirit does and a button to go on. It shows only when a batch turns the viewer's own seat into a Spirit,
 * never for a snapshot, and nothing waits on it: the game goes on underneath.
 */
import { GhostIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useGame } from '@/features/game/gameContext'
import { Button } from '@/components/ui/button'
import type { PlayerView } from '@/protocol/generated'
import { batchBetween } from '../fx/batch'
import { SPIRIT_LINES } from './haunt'

/** Whether the viewer's seat became a Spirit between the two seat lists. */
export function fellBetween(
  before: readonly PlayerView[],
  after: readonly PlayerView[],
  seat: number | null,
): boolean {
  if (seat === null) return false
  const was = before.find((player) => player.seat === seat)
  const now = after.find((player) => player.seat === seat)
  return was?.spirit !== true && now?.spirit === true
}

export function SpiritMoment() {
  const { session } = useGame()
  const [open, setOpen] = useState(false)
  const button = useRef<HTMLButtonElement>(null)
  useEffect(
    () =>
      session.store.subscribe((next, prev) => {
        const step = batchBetween(prev, next)
        if (step === null) return
        if (fellBetween(step.before.view.players, step.after.view.players, next.seat)) setOpen(true)
      }),
    [session],
  )
  useEffect(() => {
    if (!open) return undefined
    button.current?.focus()
    const close = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') setOpen(false)
    }
    window.addEventListener('keydown', close)
    return () => window.removeEventListener('keydown', close)
  }, [open])
  if (!open) return null
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="spirit-moment-title"
      data-slot="spirit-moment"
      className="fixed inset-0 z-50 grid place-items-center bg-background/80 p-4 motion-safe:animate-in motion-safe:fade-in"
    >
      <div className="grid max-w-md gap-4 rounded-xl border-2 border-dashed border-spirit bg-raised p-6 text-center shadow-e3">
        <GhostIcon aria-hidden="true" className="mx-auto size-12 text-spirit" />
        <h2 id="spirit-moment-title" className="text-2xl font-semibold text-spirit">
          You are a Restless Spirit
        </h2>
        <ul className="grid gap-2 text-sm">
          {SPIRIT_LINES.map((text) => (
            <li key={text}>{text}</li>
          ))}
        </ul>
        <Button ref={button} size="lg" onClick={() => setOpen(false)}>
          Haunt the fight
        </Button>
      </div>
    </div>
  )
}
