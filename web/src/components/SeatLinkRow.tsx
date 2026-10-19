import type { ReactNode } from 'react'
import { CopyButton } from '@/components/CopyButton'

interface SeatLinkRowProps {
  /** Who the link is for, such as "Seat 2: Bo". */
  title: string
  /** A quieter second line of the heading, such as the deck. */
  note?: ReactNode
  /** The link as written in the page: `/game/<id>#seat=<n>&token=<token>`. */
  path: string
  /** The same link with its origin, which is what is shown and copied. */
  url: string
  /** The accessible name of the copy button. */
  copyLabel: string
}

// The one element that may print a seat token, so the `data-share-link` mark lets tests tell it from a leak.
/** One seat's share link with its copy button. */
export function SeatLinkRow({ title, note, path, url, copyLabel }: SeatLinkRowProps) {
  return (
    <li className="surface grid gap-2 p-4">
      <div className="flex items-center justify-between gap-2">
        <p className="flex min-w-0 flex-wrap items-baseline gap-x-2 text-sm">
          <span className="font-medium">{title}</span>
          {note === undefined ? null : <span className="text-xs text-muted-foreground">{note}</span>}
        </p>
        <CopyButton text={url} label={copyLabel}>
          Copy
        </CopyButton>
      </div>
      <a
        data-share-link=""
        href={path}
        className="rounded-md bg-muted px-2 py-2 font-mono text-xs break-all underline-offset-2 hover:underline"
      >
        {url}
      </a>
    </li>
  )
}
