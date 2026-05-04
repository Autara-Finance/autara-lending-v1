import type { UnsignedTransaction } from "../interfaces/transaction-builder";
import type {
  ITransactionSender,
  TransactionSignature,
  TransactionReceipt,
} from "../interfaces/transaction-sender";

/**
 * Deterministic mock transaction sender for testing.
 * Liskov Substitution: interchangeable with ArchTransactionSender.
 *
 * Configure behavior via constructor options:
 *  - shouldFail: simulate transaction failures
 *  - confirmDelayMs: simulate network latency
 */
export class MockTransactionSender implements ITransactionSender {
  private txCount = 0;
  private readonly shouldFail: boolean;
  private readonly confirmDelayMs: number;
  private readonly failureError: string;

  constructor(options: {
    shouldFail?: boolean;
    confirmDelayMs?: number;
    failureError?: string;
  } = {}) {
    this.shouldFail = options.shouldFail ?? false;
    this.confirmDelayMs = options.confirmDelayMs ?? 0;
    this.failureError = options.failureError ?? "Mock transaction failed";
  }

  async signAndSend(_tx: UnsignedTransaction): Promise<TransactionSignature> {
    this.txCount++;
    return { signature: `mock_sig_${this.txCount}` };
  }

  async confirmTransaction(
    signature: TransactionSignature,
  ): Promise<TransactionReceipt> {
    if (this.confirmDelayMs > 0) {
      await new Promise((resolve) => setTimeout(resolve, this.confirmDelayMs));
    }

    return {
      signature: signature.signature,
      slot: 100_000 + this.txCount,
      blockTime: Math.floor(Date.now() / 1000),
      events: [],
      error: this.shouldFail ? this.failureError : null,
    };
  }

  /** Number of transactions sent through this mock */
  get transactionCount(): number {
    return this.txCount;
  }
}
