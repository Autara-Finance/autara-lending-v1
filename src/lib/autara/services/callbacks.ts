import type { UnsignedTransaction } from "../gateway/interfaces/transaction-builder";
import type {
  ITransactionSender,
  TransactionSignature,
  TransactionReceipt,
} from "../gateway/interfaces/transaction-sender";
import { AutaraError, AutaraErrorCode } from "../domain/errors";
import type { Result } from "../domain/result";
import { Result as R } from "../domain/result";

/**
 * Lifecycle callbacks for a protocol transaction.
 * Every mutating service method accepts this to give callers full execution control.
 *
 * Flow:
 *   onBuildStart → onBuilt → onSubmit → onConfirm → onSettled
 *                                      ↘ onError  → onSettled
 *
 * All callbacks are optional. onSettled is guaranteed to fire exactly once.
 *
 * Open/Closed Principle: extend behavior (loading states, analytics, toasts)
 * by providing new callbacks — never modify the service code.
 */
export interface TransactionLifecycle<TResult = TransactionReceipt> {
  /** Called before building the transaction. Use for pre-flight UI (loading spinner). */
  onBuildStart?(): void;

  /** Called after the unsigned transaction is built, before wallet signing. */
  onBuilt?(tx: UnsignedTransaction): void;

  /** Called after the transaction is signed and submitted. Signature available but unconfirmed. */
  onSubmit?(signature: TransactionSignature): void;

  /** Called after the transaction is confirmed on-chain. */
  onConfirm?(result: TResult): void;

  /** Called if any phase fails. Error is always a typed AutaraError. */
  onError?(error: AutaraError): void;

  /** Called after the operation completes, regardless of outcome. Guaranteed once per invocation. */
  onSettled?(): void;
}

/**
 * Execute a transaction through all lifecycle phases.
 * This is the single pipeline for build → sign → confirm.
 *
 * Dependency Inversion: depends on ITransactionSender interface, not concrete.
 */
export async function executeTransaction<T = TransactionReceipt>(
  build: () => Promise<UnsignedTransaction>,
  sender: ITransactionSender,
  lifecycle: TransactionLifecycle<T>,
  mapReceipt?: (receipt: TransactionReceipt) => T,
): Promise<Result<T>> {
  try {
    lifecycle.onBuildStart?.();

    const tx = await build();
    lifecycle.onBuilt?.(tx);

    const signature = await sender.signAndSend(tx);
    lifecycle.onSubmit?.(signature);

    const receipt = await sender.confirmTransaction(signature);
    if (receipt.error) {
      const error = new AutaraError(
        AutaraErrorCode.TransactionConfirmFailed,
        receipt.error,
      );
      lifecycle.onError?.(error);
      return R.err(error);
    }

    const result = mapReceipt ? mapReceipt(receipt) : (receipt as unknown as T);
    lifecycle.onConfirm?.(result);
    return R.ok(result);
  } catch (e) {
    const error =
      e instanceof AutaraError
        ? e
        : new AutaraError(AutaraErrorCode.TransactionSendFailed, String(e), e);
    lifecycle.onError?.(error);
    return R.err(error);
  } finally {
    lifecycle.onSettled?.();
  }
}
