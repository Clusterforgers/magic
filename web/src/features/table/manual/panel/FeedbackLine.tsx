import { cn } from '@/lib/utils'
import { feedbackWords } from './feedback'
import type { Feedback } from './feedback'

/** The line under a row's button that says what became of the action it sent; nothing before one is sent. */
export function FeedbackLine({ feedback }: { feedback: Feedback | null }) {
  if (feedback === null) return null
  const bad = feedback.state === 'refused' || feedback.state === 'lost' || feedback.state === 'offline'
  return (
    <p
      role="status"
      data-slot="command-feedback"
      data-state={feedback.state}
      className={cn('text-xs', bad ? 'text-destructive' : 'text-muted-foreground')}
    >
      {feedbackWords(feedback)}
    </p>
  )
}
