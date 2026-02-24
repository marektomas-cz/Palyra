export type A2uiErrorCode = "invalid_input" | "invalid_patch" | "conflict" | "budget_exceeded";

export class A2uiError extends Error {
  readonly code: A2uiErrorCode;
  readonly context: string | undefined;

  constructor(code: A2uiErrorCode, message: string, context?: string) {
    super(message);
    this.name = "A2uiError";
    this.code = code;
    this.context = context;
  }
}

export function asA2uiError(error: unknown, fallbackCode: A2uiErrorCode): A2uiError {
  if (error instanceof A2uiError) {
    return error;
  }
  if (error instanceof Error) {
    return new A2uiError(fallbackCode, error.message);
  }
  return new A2uiError(fallbackCode, "Unknown A2UI runtime failure.");
}
