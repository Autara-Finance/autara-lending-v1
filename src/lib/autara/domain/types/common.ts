/**
 * Branded type for public key strings.
 * Ensures type safety — you can't accidentally pass a random string where a public key is expected.
 */
declare const __publicKeyStr: unique symbol;
export type PublicKeyStr = string & { readonly [__publicKeyStr]: true };

export function publicKeyStr(value: string): PublicKeyStr {
  if (value.length === 0) throw new Error("PublicKeyStr cannot be empty");
  return value as PublicKeyStr;
}

/**
 * Account metadata for transaction instruction building.
 * Mirrors arch_program::instruction::AccountMeta.
 */
export interface AccountMeta {
  readonly pubkey: PublicKeyStr;
  readonly isSigner: boolean;
  readonly isWritable: boolean;
}
