import * as validators from "./generated/api-validators.js";

type ResultName = keyof typeof validators;
interface ValidationError {
  instancePath: string;
  message?: string;
  params?: { missingProperty?: string };
}
type Validator = ((value: unknown) => boolean) & {
  errors?: ValidationError[] | null;
};

/** Validate an engine result before exposing its fields to application code. */
export function validateApiResult<T>(name: ResultName, value: unknown): T {
  const validate = validators[name] as Validator;
  if (!validate(value)) {
    const error = validate.errors?.[0];
    const path = `${error?.instancePath ?? ""}${error?.params?.missingProperty ? `/${error.params.missingProperty}` : ""}`;
    throw new Error(
      `Invalid engine ${name} result at ${path || "/"}: ${error?.message ?? "does not match the Rust schema"}`,
    );
  }
  return value as T;
}

/** Shared JSON boundary for both worker and direct engine responses. */
export function parseApiResult<T>(name: ResultName, text: unknown): T {
  if (typeof text !== "string")
    throw new Error(`Engine ${name} result must be JSON text`);
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error(`Engine ${name} result is not valid JSON`);
  }
  return validateApiResult<T>(name, value);
}
