/**
 * Optimistic update pattern with automatic rollback on failure.
 *
 * Pairs with TransactionLifecycle callbacks:
 *   onConfirm → commit (optimistic state becomes real)
 *   onError   → rollback (revert to pre-optimistic state)
 *
 * Example usage:
 *   const update = applyOptimistic(
 *     { get: () => positionStore.getLenderPosition(market), set: (v) => positionStore.setLenderPosition(market, v) },
 *     (pos) => ({ ...pos, currentValueAtoms: atoms(BigInt(pos.currentValueAtoms) + BigInt(amount)) }),
 *   );
 *
 *   await lenderService.supply(params, authority, {
 *     onConfirm: () => update.commit(),
 *     onError: () => update.rollback(),
 *   });
 */

export interface OptimisticUpdate<T> {
  /** The state before the optimistic update was applied */
  readonly previous: T;
  /** The optimistic state that was applied */
  readonly optimistic: T;
  /** Confirm the optimistic state as the real state (no-op if already committed) */
  commit(): void;
  /** Revert to the pre-optimistic state */
  rollback(): void;
}

export interface StateAccessor<T> {
  get(): T;
  set(value: T): void;
}

/**
 * Apply an optimistic update to a state accessor.
 *
 * @param store - Get/set accessor for the state being optimistically updated
 * @param transform - Function that produces the optimistic state from the current state
 * @returns An OptimisticUpdate handle with commit() and rollback()
 */
export function applyOptimistic<T>(
  store: StateAccessor<T>,
  transform: (current: T) => T,
): OptimisticUpdate<T> {
  const previous = store.get();
  const optimistic = transform(previous);
  store.set(optimistic);

  let settled = false;

  return {
    previous,
    optimistic,
    commit() {
      if (settled) return;
      settled = true;
      // Optimistic state is already set — nothing to do
    },
    rollback() {
      if (settled) return;
      settled = true;
      store.set(previous);
    },
  };
}
