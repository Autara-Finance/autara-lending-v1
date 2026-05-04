"use server";

/**
 * Next.js Server Actions for market reads.
 * All read-only — no mutations, no wallet needed.
 */

import type { MarketView } from "../domain/market/market";

/**
 * Fetch enriched market view by address.
 */
export async function getMarketView(
  marketAddress: string,
): Promise<MarketView | null> {
  // Placeholder — will be wired up when ArchProtocolReader is implemented
  return null;
}

/**
 * Fetch all market views.
 */
export async function getAllMarketViews(): Promise<MarketView[]> {
  // Placeholder — will be wired up when ArchProtocolReader is implemented
  return [];
}
