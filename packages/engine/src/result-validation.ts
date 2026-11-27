import {
  loadFamily,
  resultFamilies,
  type ResultFamily,
  type ResultName,
} from "./generated/validator-index.js";

export type { ResultName };
interface ValidationError {
  instancePath: string;
  message?: string;
  params?: { missingProperty?: string };
}
/** One generated validator, as ajv's standalone code exports it. */
export type Validator = ((value: unknown) => boolean) & {
  errors?: ValidationError[] | null;
};

const families = new Map<ResultFamily, Promise<Record<string, Validator>>>();

function family(name: ResultFamily): Promise<Record<string, Validator>> {
  let loaded = families.get(name);
  if (!loaded) {
    loaded = loadFamily[name]().then(
      (module) => module as Record<string, Validator>,
    );
    // A failed import (a dropped connection, say) is retried on the next call.
    loaded.catch(() => families.delete(name));
    families.set(name, loaded);
  }
  return loaded;
}

async function validatorFor(name: ResultName): Promise<Validator> {
  const familyName: ResultFamily | undefined = resultFamilies[name];
  if (!familyName) throw new Error(`Unknown engine result ${String(name)}`);
  const validate = (await family(familyName))[name];
  if (!validate) throw new Error(`Unknown engine result ${String(name)}`);
  return validate;
}

/** Load the validators for these results now, so that their first
 * validation does not wait for a module to download. */
export async function preloadValidators(
  names: readonly ResultName[],
): Promise<void> {
  await Promise.all(
    [...new Set(names.map((name) => resultFamilies[name]))].map(family),
  );
}

/** Throw the first schema error, or return the value typed. */
export function checkResult<T>(
  name: ResultName,
  validate: Validator,
  value: unknown,
): T {
  if (!validate(value)) {
    const error = validate.errors?.[0];
    const path = `${error?.instancePath ?? ""}${error?.params?.missingProperty ? `/${error.params.missingProperty}` : ""}`;
    throw new Error(
      `Invalid engine ${name} result at ${path || "/"}: ${error?.message ?? "does not match the Rust schema"}`,
    );
  }
  return value as T;
}

/** Parse JSON text, with the error messages both entries share. */
export function parseResultText(name: ResultName, text: unknown): unknown {
  if (typeof text !== "string")
    throw new Error(`Engine ${name} result must be JSON text`);
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(`Engine ${name} result is not valid JSON`);
  }
}

/** Validate an engine result before exposing its fields to application code.
 * The validator for `name` is imported on first use. */
export async function validateApiResult<T>(
  name: ResultName,
  value: unknown,
): Promise<T> {
  return checkResult<T>(name, await validatorFor(name), value);
}

/** Shared JSON boundary for both worker and direct engine responses. */
export async function parseApiResult<T>(
  name: ResultName,
  text: unknown,
): Promise<T> {
  const value = parseResultText(name, text);
  return checkResult<T>(name, await validatorFor(name), value);
}
