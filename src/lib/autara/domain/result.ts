import { AutaraError, AutaraErrorCode } from "./errors";

/**
 * Discriminated union result type.
 * Service layer methods return Result<T> instead of throwing.
 */
export type Result<T, E = AutaraError> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: E };

export const Result = {
  ok<T>(value: T): Result<T, never> {
    return { ok: true, value };
  },

  err<E>(error: E): Result<never, E> {
    return { ok: false, error };
  },

  map<T, U, E>(result: Result<T, E>, fn: (value: T) => U): Result<U, E> {
    return result.ok ? Result.ok(fn(result.value)) : result;
  },

  flatMap<T, U, E>(result: Result<T, E>, fn: (value: T) => Result<U, E>): Result<U, E> {
    return result.ok ? fn(result.value) : result;
  },

  unwrapOr<T, E>(result: Result<T, E>, fallback: T): T {
    return result.ok ? result.value : fallback;
  },

  async fromPromise<T>(promise: Promise<T>): Promise<Result<T, AutaraError>> {
    try {
      return Result.ok(await promise);
    } catch (e) {
      if (e instanceof AutaraError) return Result.err(e);
      return Result.err(
        new AutaraError(AutaraErrorCode.RpcError, String(e), e),
      );
    }
  },
} as const;
