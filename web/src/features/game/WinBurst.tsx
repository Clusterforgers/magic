/**
 * The trophy of a won game with its celebration: the badge rises in a gold glow and pieces in the five mana colours
 * burst out of it once (`winBurst.css`). Decoration: the headline says the win in words, and a viewer who asked for
 * reduced motion sees the badge still.
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
