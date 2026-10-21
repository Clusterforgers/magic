/** The ids of a field's two notes: the rule that is always there and the problem that comes and goes. */
export const hintId = (field: string): string => `${field}-hint`
export const problemId = (field: string): string => `${field}-problem`

/**
 * What a field's input points its description at: the problem first, so it is the first thing a screen reader says,
 * then the rule if the field has one.
 */
export const describedBy = (field: string, hasRule = true): string =>
  hasRule ? `${problemId(field)} ${hintId(field)}` : problemId(field)
