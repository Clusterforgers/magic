import { DicesIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Field } from '@/features/auth/Field'
import { describedBy } from '@/features/auth/fieldIds'
import { PASSWORD_RULE } from '@/features/auth/rules'
import { generatePassword } from './password'
import type { PasswordControl } from './usePasswordControl'

interface PasswordFieldProps {
  id: string
  control: PasswordControl
  problem: string | null
  /** True while the form is sent: the input is read-only and the generator is off. */
  pending: boolean
  /** The text after every edit, so a complaint about the password can be taken back as it is fixed. */
  onEdit: (value: string) => void
  /** After "Make a password" has filled the field. */
  onGenerated: () => void
  label?: string
}

/**
 * The password a form hands to a friend: the rule, the input, "Make a password" (a random one, shown in plain text so it
 * can be copied) and a switch for showing the text. It is a new password, never one the browser should fill or save: the
 * admin's browser would otherwise offer to keep the friend's login under the admin's own entry.
 */
export function PasswordField({
  id,
  control,
  problem,
  pending,
  onEdit,
  onGenerated,
  label = 'Password',
}: PasswordFieldProps) {
  const { ref, revealed, setRevealed } = control

  function make(): void {
    if (ref.current === null) return
    ref.current.value = generatePassword()
    setRevealed(true)
    onGenerated()
    ref.current.focus()
  }

  return (
    <Field id={id} label={label} rule={PASSWORD_RULE} problem={problem}>
      <Input
        ref={ref}
        id={id}
        name={id}
        type={revealed ? 'text' : 'password'}
        autoComplete="new-password"
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        // The attributes the common password managers read to leave a field alone.
        data-1p-ignore
        data-lpignore="true"
        data-bwignore="true"
        readOnly={pending}
        aria-invalid={problem !== null}
        aria-describedby={describedBy(id)}
        className={revealed ? 'font-mono' : undefined}
        onChange={(event) => onEdit(event.target.value)}
      />
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="outline" size="sm" disabled={pending} onClick={make}>
          <DicesIcon aria-hidden="true" />
          Make a password
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-pressed={revealed}
          onClick={() => setRevealed(!revealed)}
        >
          {revealed ? 'Hide the password' : 'Show the password'}
        </Button>
      </div>
    </Field>
  )
}
