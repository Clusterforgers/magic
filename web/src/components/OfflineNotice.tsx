import { WifiOffIcon } from 'lucide-react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { PageShell } from '@/components/PageShell'
import { useOnline } from '@/hooks/useOnline'

/** A line under the header while the browser is offline; polite, since nothing the page does can fix it. */
export function OfflineNotice() {
  const online = useOnline()
  if (online) return null
  return (
    <PageShell className="pb-0 sm:pb-0">
      <Alert role="status" variant="warning">
        <WifiOffIcon aria-hidden="true" />
        <AlertTitle>You are offline</AlertTitle>
        <AlertDescription>Games and lists carry on when the network is back.</AlertDescription>
      </Alert>
    </PageShell>
  )
}
