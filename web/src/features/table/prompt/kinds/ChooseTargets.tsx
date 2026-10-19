import { Button } from '@/components/ui/button'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'

/**
 * Placeholder until the target prompt: it can only take the cast back (nothing is paid yet), so a spell chosen by
 * mistake never leaves the table stuck. Replace this file's body with the real prompt, keeping the export.
 */
export const ChooseTargetsPrompt: PromptComponent<'ChooseTargets'> = ({ options }) => {
  const { answer } = usePrompt()
  return (
    <PromptLayout text="Choosing targets is not supported in the UI yet">
      {options.can_cancel ? (
        <Button
          variant="outline"
          onClick={() => {
            answer({ ChooseTargets: 'Cancel' })
          }}
        >
          Cancel the spell
        </Button>
      ) : null}
    </PromptLayout>
  )
}
