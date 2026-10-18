import { Alert, AlertAction, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'

/** The latest thing the server or the session wants the player to know, with a way to dismiss it. */
export function ServerNotice({ message, onDismiss }: { message: string; onDismiss(): void }) {
  return (
    <Alert>
      <AlertDescription className="text-foreground">{message}</AlertDescription>
      <AlertAction>
        <Button type="button" variant="ghost" size="xs" onClick={onDismiss}>
          Dismiss
        </Button>
      </AlertAction>
    </Alert>
  )
}
