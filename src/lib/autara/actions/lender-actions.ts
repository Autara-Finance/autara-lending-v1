"use server";

/**
 * Next.js Server Actions for lender operations.
 *
 * Server Actions run on the server but can be called from Client Components.
 * Read operations (getPosition) execute fully server-side.
 * Write operations return serializable results across the RSC boundary.
 *
 * NOTE: Transaction signing must happen client-side (wallet is in the browser).
 * These actions handle the read/validation portion. The actual sign+send
 * flow is orchestrated by the client-side LenderService with lifecycle callbacks.
 */

import type { LenderPositionView } from "../domain/lender/lender-types";

/**
 * Fetch a lender's position for a specific market.
 * Safe to call from Server Components.
 *
 * @param readerFactory - Factory that creates an IProtocolReader (avoids serializing the reader)
 * @param marketAddress - Market public key
 * @param authority - User's public key
 */
export async function getLenderPosition(
  marketAddress: string,
  authority: string,
): Promise<LenderPositionView | null> {
  // In a real implementation, this would instantiate a server-side reader
  // using environment variables for RPC URL and program ID.
  //
  // const reader = createServerProtocolReader();
  // const marketService = new MarketService(reader);
  // const lenderService = new LenderService(reader, null!, null!);
  // const result = await lenderService.getPosition(marketAddress as PublicKeyStr, authority as PublicKeyStr);
  // return result.ok ? result.value : null;

  // Placeholder — will be wired up when ArchProtocolReader is implemented
  return null;
}

/**
 * Fetch all lender positions for a user.
 */
export async function getUserLenderPositions(
  authority: string,
): Promise<LenderPositionView[]> {
  // Placeholder — will be wired up when ArchProtocolReader is implemented
  return [];
}
