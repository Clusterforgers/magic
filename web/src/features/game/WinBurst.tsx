/**
 * The celebration of a won game (`winBurst.css`): the winner's trophy rises in a gold glow and pieces in the five mana
 * colours burst out of it once; every other seat sees a short banner naming the winner rise in. Decoration: the
 * headline and its sentence say the result in words, nothing here takes a click, and a viewer who asked for reduced
 * motion sees both still.
 */
import { TrophyIcon } from 'lucide-react'
import type { CSSProperties } from 'react'
import './winBurst.css'

const COLOURS = [
  'var(--color-mana-white)',
  'var(--color-mana-blue)',
  'var(--color-mana-black)',
  'var(--color-mana-red)',
  'var(--color-mana-green)',
] as const

/** How many pieces burst out. */
export const WIN_PIECES = 30

/** The flight of piece `n`: an even spread of angles and distances, the same on every render. */
export function pieceFlight(n: number): { dx: number; dy: number; turn: number; delay: number } {
  const angle = (n / WIN_PIECES) * Math.PI * 2 + (n % 3) * 0.17
  const reach = 46 + ((n * 37) % 40)
  return {
    dx: Math.round(Math.cos(angle) * reach),
    // They fall as they fly: more down than up.
    dy: Math.round(Math.sin(angle) * reach * 0.8 + 24),
    turn: ((n * 97) % 360) + 180,
    delay: (n % 6) * 40,
  }
}

/** The trophy badge with its burst. */
export function WinBurst() {
  return (
    <span className="relative flex size-12 shrink-0 items-center justify-center">
      <span aria-hidden="true" className="pointer-events-none absolute inset-0 overflow-visible">
        {Array.from({ length: WIN_PIECES }, (_, n) => {
          const flight = pieceFlight(n)
          return (
            <span
              key={n}
              data-slot="win-piece"
              style={
                {
                  '--dx': `${flight.dx}px`,
                  '--dy': `${flight.dy}px`,
                  '--turn': `${flight.turn}deg`,
                  '--delay': `${flight.delay}ms`,
                  background: COLOURS[n % COLOURS.length],
                } as CSSProperties
              }
            />
          )
        })}
      </span>
      <span
        data-slot="win-badge"
        className="relative flex size-12 items-center justify-center rounded-full bg-turn-you text-on-turn-you ring-2 ring-turn-you-edge"
      >
        <TrophyIcon aria-hidden="true" className="size-6" />
      </span>
    </span>
  )
}

/** The winner's name with a small trophy, for every seat but the winner's. */
export function WinnerBanner({ name }: { name: string }) {
  return (
    <p
      aria-hidden="true"
      data-slot="win-banner"
      className="pointer-events-none flex w-fit shrink-0 items-center gap-2 rounded-full bg-turn-you px-3 py-1 text-sm font-medium text-on-turn-you ring-2 ring-turn-you-edge"
    >
      <TrophyIcon aria-hidden="true" className="size-4 shrink-0" />
      <span className="min-w-0 truncate">{name} won</span>
    </p>
  )
}
