import { Link } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { buttonVariants } from '@/components/ui/button'
import { bulkReadsApi } from '@/net/bulkReads'
import type { BulkReadsApi } from '@/net/bulkReads'
import { bulkLinkText } from './model'

/** The read page's way to `/read/bulk`, shown only while some wording-only cards wait for this account's read. */
export function BulkLink({ api = bulkReadsApi }: { api?: Pick<BulkReadsApi, 'list'> }) {
  const [count, setCount] = useState(0)
  useEffect(() => {
    let live = true
    api.list().then(
      (reads) => {
        if (live) setCount(reads.cards.length)
      },
      () => undefined,
    )
    return () => {
      live = false
    }
  }, [api])
  if (count === 0) return null
  return (
    <Link to="/read/bulk" className={buttonVariants({ variant: 'outline' })}>
      {bulkLinkText(count)}
    </Link>
  )
}
