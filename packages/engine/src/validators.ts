/** Synchronous result validation, for a host that cannot await it. This
 * entry imports every generated validator when it loads, so the main entry
 * never does. */
import { validators } from "./generated/validator-eager.js";
import {
  checkResult,
  parseResultText,
  type ResultName,
  type Validator,
} from "./result-validation.js";

export type { ResultName };

/** Validate an engine result synchronously. */
export function validateApiResultSync<T>(name: ResultName, value: unknown): T {
  const validate = validators[name] as Validator | undefined;
  if (!validate) throw new Error(`Unknown engine result ${String(name)}`);
  return checkResult<T>(name, validate, value);
}

/** Parse and validate engine JSON text synchronously. */
export function parseApiResultSync<T>(name: ResultName, text: unknown): T {
  return validateApiResultSync<T>(name, parseResultText(name, text));
}
