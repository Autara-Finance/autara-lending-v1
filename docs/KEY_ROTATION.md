# Key rotation assessment — exposed `keys/` material (production fallback branch)

> **Status: PLAN ONLY.** This document is an assessment for a human operator to
> execute. **No on-chain action, key rotation, or broadcast has been performed
> here.** This PR only stops git from *tracking* the committed keypairs and
> moves the Docker/server runtime to env-based secrets. It does **not** un-leak
> anything, and it does **not** change the deployed program/oracle ids.

> **Branch note.** `dia-oracle-fallback` is the branch that **auto-deploys to
> live production** (Railway service "a-l-v1 (dia oracle fallback)"). This change
> deliberately **keeps production on the OLD program** `53def2dc…1cf64192`
> (oracle `eee682c2…ed84b15`). It is purely keys-hygiene/infra: no program-id
> source constant, oracle code, or protocol logic is touched. Because the server
> now derives ids from the decoded key files instead of baked `COPY keys/`,
> Railway **must** be configured with the OLD key material *before* this is
> merged — see "Runtime after this change" below.

## Why untracking is not enough

The repository has been **public**, so every keypair under `keys/` must be
treated as **permanently compromised**. Removing them from the git index (and
not rewriting history, by design) prevents *future* commits of the same files
but does nothing to revoke the already-exposed secret bytes. The only real
remediation is **on-chain rotation / redeployment**, described below.

These keys belong to the **OLD / "stage" testnet deployment** (autara-program
`53def2dc…1cf64192`, autara-oracle `eee682c2…ed84b15`) — i.e. exactly what this
fallback branch runs in production today.

## Exposed keys, roles, and blast radius

| File | Role | On-chain identity | Blast radius if compromised | Rotation urgency |
|------|------|-------------------|-----------------------------|------------------|
| `keys/autara-deployer.key` | Program **upgrade authority** + payer for ELF uploads | upgrade authority of program `53def2dc…` / oracle `eee682c2…` | **CRITICAL** — holder can push a **malicious program upgrade** to the live testnet program, replacing protocol logic for all users/markets. | **Highest** |
| `keys/autara-admin-stage.key` | Protocol **global-config admin** + fee receiver + `create_global_config` signer | global-config admin authority | **CRITICAL/HIGH** — holder controls protocol governance config (admin, fee receiver, fee share). | **Highest** |
| `keys/autara-token-authority.key` | **Mint authority** for the test BTC/ETH/USDC mints | authority `455d9392…` (see `tokens.json`) | **HIGH (testnet value)** — holder can mint unlimited test BTC/ETH/USDC, distorting/draining test markets. | High |
| `keys/autara-cli-signer.key` | Server **operational signer** — market creator, faucet-funded fee payer, Pyth feed pusher | market-owner pubkey (markets PDA-derived from it) | **MEDIUM/HIGH** — holder can create/own markets and push oracle feeds as this identity; faucet-funded so low standing balance. | High |
| `keys/autara-stage.key` | autara-**program** account keypair; its pubkey **equals** the deployed program id (`53def2dc…`) | program account address | **LOW post-deploy** — the program is already deployed; upgrades are gated by the *deployer/upgrade authority*, not this key. Address cannot be "rotated" — it is fixed. | Redeploy (see below) |
| `keys/autara-pyth-stage.key` | autara-**oracle** program account keypair (`eee682c2…`); oracle is position-independent | oracle program account address | **LOW post-deploy** — same reasoning as above. | Redeploy (see below) |
| `keys/token-btc.key` | BTC test **mint account** keypair | mint `36a97410…` | **LOW** — the mint account already exists; minting is gated by the *mint authority* (`autara-token-authority.key`), not the mint account key. Not loaded by the server. | Redeploy mints |
| `keys/token-eth.key` | ETH test **mint account** keypair | mint `7250792…` | **LOW** (as above) | Redeploy mints |
| `keys/token-usdc.key` | USDC test **mint account** keypair | mint `a80fa79…` | **LOW** (as above) | Redeploy mints |

> `tokens.json` is left **tracked**: it holds public program ids / mints /
> authority pubkeys only — **no secret key bytes**. (Confirmed: it stores
> `keyFile` *paths*, not key material.)

## Concrete rotation / remediation steps (human-executed)

Order by urgency. Do these on **testnet** first; verify, then repeat the
pattern if/when a mainnet deploy exists.

1. **Rotate the program upgrade authority (`autara-deployer.key`) — DO FIRST.**
   Transfer the upgrade authority of program `53def2dc…` and oracle
   `eee682c2…` to a **new, never-committed** keypair. Until this is done, anyone
   with the leaked deployer key can upgrade the live program bytecode. After
   transfer, the old deployer key is inert for upgrades.

2. **Rotate the protocol admin (`autara-admin-stage.key`).**
   Use the protocol's admin-change path to move global-config admin + fee
   receiver to a new keypair. (If the on-chain program has no admin-transfer
   instruction, this requires a program upgrade — sequence it after step 1 with
   the *new* upgrade authority.)

3. **Rotate the token mint authority (`autara-token-authority.key`).**
   Reassign mint authority of the BTC/ETH/USDC test mints to a new keypair (or
   set authority to `None`/burn it if no further minting is intended). This
   stops unauthorized minting with the leaked key.

4. **Retire the operational signer (`autara-cli-signer.key`).**
   Generate a new server signer, provide it to the runtime via `SIGNER_KEY_B64`
   (see below), and drain/abandon the old signer's faucet balance. Markets owned
   by the old signer remain PDA-bound to it; recreate under the new signer if
   ownership separation is required.

5. **Redeploy programs + mints to fresh addresses (preferred clean slate).**
   Standing up a fresh deployment to brand-new program/oracle addresses (with
   gitignored keys) and cutting clients over to it is the most thorough
   remediation, since the old program/mint **account addresses themselves cannot
   be rotated** — only their authorities can. Once traffic is migrated, the
   exposed `keys/` material is fully retired. **Note:** this fallback branch is
   intentionally staying on the OLD program for now, so a fresh redeploy is a
   future migration, not part of this change.

## Runtime after this change (no keys in the image)

The server image no longer contains any key bytes and no longer passes
`--program-id`/`--oracle-program-id` flags. `entrypoint.sh` decodes secrets from
base64 env vars at startup into `/app/keys/`, and the server derives the
program/oracle ids from those files at boot
(`autara_stage_program_id()` reads `keys/autara-stage.key`,
`autara_oracle_stage_program_id()` reads `keys/autara-pyth-stage.key`).

To **keep production on the OLD program**, the operator MUST provide, on the
Railway production service, **before merge** (merge auto-deploys):

- `PROGRAM_KEY_B64` — base64 of the **OLD** `keys/autara-stage.key` (pubkey
  `53def2dc…1cf64192`). This is what pins the deployed program id.
- `ORACLE_KEY_B64` — base64 of the **OLD** `keys/autara-pyth-stage.key` (pubkey
  `eee682c2…ed84b15`).
- `SIGNER_KEY_B64` — server signer (decoded to `/app/keys/signer.key`, exported
  as `AUTARA_SIGNER_KEY`).
- `TOKEN_AUTHORITY_KEY_B64` — token mint authority (referenced by `tokens.json`
  as `keys/autara-token-authority.key`).
- `TOKENS_JSON_B64` *(optional)* — overrides the baked-in `tokens.json`.

> **Fail-safe:** if `PROGRAM_KEY_B64`/`ORACLE_KEY_B64` are absent, the server
> panics at boot (`with_secret_key_file(...).unwrap()` on a missing file) and the
> deploy fails fast rather than silently running a different program id.

> **Security note:** ideally these env vars would carry **rotated** key material
> rather than the exposed `keys/` bytes. Because production is deliberately
> staying on the OLD already-deployed program/oracle for now, the OLD key
> material is required here to preserve the current program id. The program and
> oracle account keys are **LOW** blast-radius post-deploy (see table); the
> high-urgency rotations are the deployer/admin/authority keys.
