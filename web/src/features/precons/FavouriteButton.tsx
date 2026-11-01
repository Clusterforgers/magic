import { StarIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { Favourites } from './useFavourites'

/** The star that marks a precon as a favourite of the account, or takes the mark back. */
export function FavouriteButton({
  code,
  name,
  favourites,
  withLabel = false,
}: {
  code: string
  name: string
  favourites: Favourites
  /** Writes "Favourite" beside the star. */
  withLabel?: boolean
}) {
  const starred = favourites.has(code)
  return (
    <Button
      type="button"
      variant="outline"
      size={withLabel ? 'sm' : 'icon'}
      data-favourite={code}
      aria-pressed={starred}
      aria-label={starred ? `Remove ${name} from your favourites` : `Add ${name} to your favourites`}
      disabled={favourites.isPending(code)}
      focusableWhenDisabled
      onClick={() => favourites.toggle(code)}
    >
      <StarIcon aria-hidden="true" className={cn(starred && 'fill-current text-warning')} />
      {withLabel ? (starred ? 'Favourite' : 'Add to favourites') : null}
    </Button>
  )
}
