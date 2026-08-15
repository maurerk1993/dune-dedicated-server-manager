export class OperationTimeoutError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "OperationTimeoutError";
  }
}

/**
 * Rejects locally when an async operation exceeds its wall-clock budget.
 *
 * Tauri invokes cannot currently be cancelled from JavaScript, so callers
 * must also ignore late results. This helper's job is to guarantee that UI
 * cleanup can run even when the native promise has not settled yet.
 */
export async function withTimeout<T>(
  operation: Promise<T>,
  timeoutMs: number,
  message: string,
): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => reject(new OperationTimeoutError(message)), timeoutMs);
  });

  try {
    return await Promise.race([operation, deadline]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}
