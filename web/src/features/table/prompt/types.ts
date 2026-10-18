import type { ComponentType } from 'react'
import type { DecisionKind } from '@/protocol/generated'
import type { DecisionName } from '../turnText'

/** The options a decision kind carries, by the kind's name: `OptionsOf<'Priority'>` is `PriorityOptions`. */
export type OptionsOf<K extends DecisionName> =
  Extract<DecisionKind, Record<K, unknown>> extends Record<K, infer Options> ? Options : never

/** What the host hands the prompt of kind `K`. The rest (answering, selecting) comes from `usePrompt()`. */
export interface PromptProps<K extends DecisionName> {
  options: OptionsOf<K>
}

/** The prompt of one decision kind. */
export type PromptComponent<K extends DecisionName> = ComponentType<PromptProps<K>>
