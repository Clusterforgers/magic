import { cn } from '@/lib/utils'
import { DISC } from './discs'
import { identityWords } from './identity'

/**
 * A colour identity as small discs with the letter in each, so colour is never the only sign; the group is named in
 * words ("Colour identity: White and Blue", or "Colorless" for none).
 */
export function IdentityPips({ identity, className }: { identity: readonly string[]; className?: string }) {
  const letters = identity.length === 0 ? ['C'] : identity
  return (
    <span
      role="img"
      aria-label={`Colour identity: ${identityWords(identity)}`}
      data-slot="identity"
      className={cn('inline-flex items-center gap-1', className)}
    >
      {letters.map((letter) => (
        <span
          key={letter}
          aria-hidden="true"
          className={cn(
            'flex size-5 items-center justify-center rounded-full text-xs leading-none font-semibold ring-1 ring-foreground/25',
            DISC[letter],
          )}
        >
          {letter}
        </span>
      ))}
    </span>
  )
}
