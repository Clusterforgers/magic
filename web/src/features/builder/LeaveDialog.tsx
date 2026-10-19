import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'

/** The part of the router's blocker the dialog needs: blocked with a way on and a way back, or idle. */
export type Resolver = { status: 'blocked'; proceed: () => void; reset: () => void } | { status: 'idle' }

/**
 * "Leave without saving?", asked when the router stops a navigation away from a deck with unsaved changes. The safe
 * choice, Keep editing, is first and takes the focus; Escape keeps editing too.
 */
export function LeaveDialog({ blocker, name }: { blocker: Resolver; name: string }) {
  return (
    <AlertDialog
      open={blocker.status === 'blocked'}
      onOpenChange={(open) => {
        if (!open && blocker.status === 'blocked') blocker.reset()
      }}
    >
      <AlertDialogContent>
        <AlertDialogTitle>Leave without saving?</AlertDialogTitle>
        <AlertDialogDescription>
          {`“${name.trim() === '' ? 'This deck' : name.trim()}” has changes that are not saved. If you leave now they are lost.`}
        </AlertDialogDescription>
        <AlertDialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => blocker.status === 'blocked' && blocker.reset()}
          >
            Keep editing
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={() => blocker.status === 'blocked' && blocker.proceed()}
          >
            Discard changes
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
