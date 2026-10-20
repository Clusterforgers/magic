import { InfoIcon } from 'lucide-react'
import { useEffect } from 'react'
import { useStore } from 'zustand'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertAction, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { clearFlash, flashStore } from '@/lib/flash'

/** How long a notice stays before it goes by itself, in milliseconds. */
export const FLASH_MS = 10_000

/** The notice a page left behind (`showFlash`): under the header, announced once, dismissed by a button or in 10 s. */
export function FlashNotice() {
  const { message, id } = useStore(flashStore)
  useEffect(() => {
    if (message === null) return
    const timer = window.setTimeout(clearFlash, FLASH_MS)
    return () => window.clearTimeout(timer)
  }, [message, id])
  if (message === null) return null
  return (
    <PageShell data-slot="flash-notice" className="pb-0 sm:pb-0">
      <Alert>
        <InfoIcon aria-hidden="true" />
        <AlertDescription className="text-foreground">{message}</AlertDescription>
        <AlertAction>
          <Button type="button" variant="ghost" size="xs" onClick={clearFlash}>
            Dismiss
          </Button>
        </AlertAction>
      </Alert>
    </PageShell>
  )
}
