"use server";

/**
 * Next.js Server Actions for borrower operations.
 *
 * Same pattern as lender-actions.ts — reads are server-side,
 * mutations require client-side wallet signing via BorrowerService.
 */

import type { BorrowerPositionView } from "../domain/borrower/borrower-types";
import type { BorrowPositionHealth } from "../domain/borrower/borrow-health";

/**
 * Fetch a borrower's position for a specific market.
 */
export async function getBorrowerPosition(
  marketAddress: string,
  authority: string,
): Promise<BorrowerPositionView | null> {
  // Placeholder — will be wired up when ArchProtocolReader is implemented
  return null;
}

/**
 * Fetch all borrower positions for a user.
 */
export async function getUserBorrowerPositions(
  authority: string,
): Promise<BorrowerPositionView[]> {
  // Placeholder — will be wired up when ArchProtocolReader is implemented
  return [];
}

/**
 * Fetch the health of a specific borrow position.
 * Useful for monitoring/alerting without full position view.
 */
export async function getBorrowPositionHealth(
  marketAddress: string,
  authority: string,
): Promise<BorrowPositionHealth | null> {
  // Placeholder — will be wired up when ArchProtocolReader is implemented
  return null;
}
