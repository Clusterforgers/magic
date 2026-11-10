import type { CoinSide } from '@/protocol/generated'
import { cn } from '@/lib/utils'

/** One face of the coin, its letter on a round plate. */
function Face({ letter, back }: { letter: string; back?: boolean }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'coin-face absolute inset-0 grid place-items-center rounded-full border-4 border-primary bg-card text-4xl font-bold text-foreground shadow-md',
        back === true && 'coin-back',
      )}
    >
      {letter}
    </span>
  )
}

/**
 * A coin that turns over in the air and lands on `side` (`data-landed`): two faces, H and T, back to back. The turn is
 * a CSS 3D flip (`moment.css`, only where motion is welcome) that ends with the side up; a `still` coin shows the side
 * and nothing else. The side is said in words once it has `landed` (`useCoinLanded`), so a screen reader and a test
 * read it.
 */
export function CoinFace({ side, still, landed }: { side: CoinSide; still: boolean; landed: boolean }) {
  return (
    <span className="grid place-items-center gap-2">
      <span
        data-slot="coin"
        data-side={side}
        data-still={still || undefined}
        data-landed={landed || undefined}
        className="coin relative size-28"
      >
        <span className="coin-body absolute inset-0">
          <Face letter="H" />
          <Face letter="T" back />
        </span>
      </span>
      <span data-slot="coin-side" aria-live="polite" className="text-lg font-semibold">
        {landed ? side : ' '}
      </span>
    </span>
  )
}
