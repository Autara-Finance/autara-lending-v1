// Domain layer
export * from "./domain";

// Gateway interfaces
export type { IProtocolReader } from "./gateway/interfaces/protocol-reader";
export type { ITransactionBuilder, UnsignedTransaction } from "./gateway/interfaces/transaction-builder";
export type { ITransactionSender, TransactionSignature, TransactionReceipt, AutaraEvent } from "./gateway/interfaces/transaction-sender";

// Services
export type { TransactionLifecycle } from "./services/callbacks";
export { executeTransaction } from "./services/callbacks";
export type { ILenderService } from "./services/lender-service";
export { LenderService } from "./services/lender-service";
export type { IBorrowerService } from "./services/borrower-service";
export { BorrowerService } from "./services/borrower-service";
export type { IMarketService } from "./services/market-service";
export { MarketService } from "./services/market-service";

// State management
export { MarketStore } from "./state/market-store";
export { PositionStore } from "./state/position-store";
export type { OptimisticUpdate, StateAccessor } from "./state/optimistic";
export { applyOptimistic } from "./state/optimistic";

// Composition root
export type { AutaraServices, AutaraServicesConfig } from "./create-services";
export { createAutaraServices } from "./create-services";

// Gateway implementations
export { MockProtocolReader } from "./gateway/mock/mock-protocol-reader";
export { MockTransactionSender } from "./gateway/mock/mock-transaction-sender";
export { createPdaDeriver } from "./gateway/arch/pda";
