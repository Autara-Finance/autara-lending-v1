import type { Atoms } from "../../domain/types/atoms";
import type { PublicKeyStr } from "../../domain/types/common";
import type { LeverageParams, DeleverageParams } from "../../domain/borrower/borrower-types";

/**
 * An unsigned transaction payload ready for wallet signing.
 */
export interface UnsignedTransaction {
  readonly message: Uint8Array;
  readonly signatures: Uint8Array[];
  readonly description: string;
}

/**
 * Builds unsigned transaction payloads — Single Responsibility Principle.
 *
 * No wallet dependency. No signing. No sending.
 * Just constructs the instruction bytes matching the on-chain program's expected layout.
 *
 * Each method mirrors a corresponding instruction builder from autara-lib/src/ixs/.
 *
 * Implementations:
 *  - ArchTransactionBuilder: builds real Arch instructions
 *  - MockTransactionBuilder: returns deterministic payloads for testing
 */
export interface ITransactionBuilder {
  // ── Lender instructions (mirrors ixs/supply.rs) ──────────────────────

  createSupplyPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<UnsignedTransaction>;

  supply(
    market: PublicKeyStr,
    authority: PublicKeyStr,
    amount: Atoms,
  ): Promise<UnsignedTransaction>;

  withdrawSupply(
    market: PublicKeyStr,
    authority: PublicKeyStr,
    amount: Atoms | "all",
  ): Promise<UnsignedTransaction>;

  // ── Borrower instructions (mirrors ixs/borrow.rs) ────────────────────

  createBorrowPosition(
    market: PublicKeyStr,
    authority: PublicKeyStr,
  ): Promise<UnsignedTransaction>;

  depositCollateral(
    market: PublicKeyStr,
    authority: PublicKeyStr,
    amount: Atoms,
  ): Promise<UnsignedTransaction>;

  withdrawCollateral(
    market: PublicKeyStr,
    authority: PublicKeyStr,
    amount: Atoms | "all",
  ): Promise<UnsignedTransaction>;

  borrow(
    market: PublicKeyStr,
    authority: PublicKeyStr,
    amount: Atoms,
  ): Promise<UnsignedTransaction>;

  repay(
    market: PublicKeyStr,
    authority: PublicKeyStr,
    amount: Atoms | "all",
  ): Promise<UnsignedTransaction>;

  // ── Compound instructions (mirrors BorrowDepositApl / WithdrawRepayApl) ──

  borrowAndDeposit(
    market: PublicKeyStr,
    authority: PublicKeyStr,
    params: LeverageParams,
  ): Promise<UnsignedTransaction>;

  withdrawAndRepay(
    market: PublicKeyStr,
    authority: PublicKeyStr,
    params: DeleverageParams,
  ): Promise<UnsignedTransaction>;

  // ── Liquidation ───────────────────────────────────────────────────────

  liquidate(
    market: PublicKeyStr,
    liquidator: PublicKeyStr,
    positionOwner: PublicKeyStr,
    maxRepayAtoms: Atoms,
  ): Promise<UnsignedTransaction>;
}
