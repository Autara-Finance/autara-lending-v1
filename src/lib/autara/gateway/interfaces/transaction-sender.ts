import type { Atoms } from "../../domain/types/atoms";
import type { PublicKeyStr } from "../../domain/types/common";
import type { IFixedPoint } from "../../domain/types/fixed-point";
import type { OracleRate } from "../../domain/types/oracle";
import type { SupplyVaultSummary } from "../../domain/market/supply-vault";
import type { UnsignedTransaction } from "./transaction-builder";

/**
 * Transaction signature returned after signing and sending.
 */
export interface TransactionSignature {
  readonly signature: string;
}

/**
 * Full receipt after transaction confirmation.
 */
export interface TransactionReceipt {
  readonly signature: string;
  readonly slot: number;
  readonly blockTime: number | null;
  readonly events: ReadonlyArray<AutaraEvent>;
  readonly error: string | null;
}

/**
 * Events emitted by the on-chain program.
 * Mirrors the event structs from autara-lib/src/event.rs.
 */
export type AutaraEvent = SingleMarketTransactionEvent;

export interface SingleMarketTransactionEvent {
  readonly market: PublicKeyStr;
  readonly user: PublicKeyStr;
  readonly position: PublicKeyStr;
  readonly mint: PublicKeyStr;
  readonly amount: Atoms;
  readonly supplyVaultSummary: SupplyVaultSummary;
  readonly collateralVaultAtoms: Atoms;
  readonly supplyOracleRate: OracleRate;
  readonly collateralOracleRate: OracleRate;
}

/**
 * Signs and sends transactions — client-side only.
 *
 * Open/Closed Principle: closed for modification, open for extension
 * via different signing strategies (wallet adapter, Ledger, multisig, etc.).
 *
 * Implementations:
 *  - ArchTransactionSender: signs via wallet adapter, sends to Arch RPC
 *  - MockTransactionSender: deterministic for testing
 */
export interface ITransactionSender {
  /** Sign with the connected wallet and submit to the network */
  signAndSend(tx: UnsignedTransaction): Promise<TransactionSignature>;

  /** Wait for transaction confirmation and parse events */
  confirmTransaction(signature: TransactionSignature): Promise<TransactionReceipt>;
}
