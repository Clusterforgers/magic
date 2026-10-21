import { CircleAlertIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { Label } from '@/components/ui/label'
import { hintId, problemId } from './fieldIds'

interface FieldProps {
  id: string
  label: string
  /** The rule the field is held to, in a sentence, shown before it is typed; null for a field with none. */
  rule: string | null
  /** What is wrong with the field now, or null. */
  problem: string | null
  /** The input. */
  children: ReactNode
}

/**
 * A label, the rule, the input and, under it, the problem. The problem's place is always in the page and announces
 * itself politely when its words change, so a screen reader hears a problem even when focus stays on the field; it
 * takes no room while it is empty. A problem carries an icon, so it never rests on its colour alone.
 */
export function Field({ id, label, rule, problem, children }: FieldProps) {
  return (
    <div className="grid gap-2">
      <Label htmlFor={id}>{label}</Label>
      {rule === null ? null : (
        <p id={hintId(id)} className="text-sm text-muted-foreground">
          {rule}
        </p>
      )}
      {children}
      <div id={problemId(id)} aria-live="polite" className="empty:sr-only">
        {problem === null ? null : (
          <p className="flex items-start gap-2 text-sm text-destructive">
            <CircleAlertIcon aria-hidden="true" className="size-4 shrink-0 translate-y-0.5" />
            <span>{problem}</span>
          </p>
        )}
      </div>
    </div>
  )
}
