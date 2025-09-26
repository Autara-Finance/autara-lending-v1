### Autara Market

Autara markets enable lenders and borrowers to interact in a permissionless manner.
Lenders can provide liquidity to the market by depositing assets, while borrowers can take out loans by providing collateral.
Each market is isolated from others. This isolation is crucial for maintaining the integrity and stability of the overall system.
Risk parameters and bad debt events are managed by a curator, who is incentivized to maintain a healthy market.

#### Market Components

- **Supply token**: Represents the asset that lenders deposit into the market and borrowers receive when they take out a loan. Supply tokens earn interest over time, which is paid by borrowers.
- **Collateral token**: Represents the asset that borrowers provide as collateral when taking out a loan. Collateral token are idle and do not earn interest, to guarantee liquidity in case of liquidation and ensure the safety of the market.
- **Curator**: An address that has the authority to manage risk parameters of the market see section _Market Risk Parameters_.
- **Interest Rate Model**: A contract that defines how interest rates are calculated based on the utilization of the market. See section _Interest Rate Model_.

Market are composed by a collateral vault and a supply vault.
The collateral vault holds the collateral token deposited by borrowers, while the supply vault holds the supply token deposited by lenders and borrowed by borrowers and keeps track of the interest accrued on the supply token.

##### Market Risk Parameters

- **Max utilization rate**: The maximum percentage of the market's total supply that can be borrowed.
- **Max supply** : The maximum amount of the supply token that can be deposited into the market.
- **Max Loan to Value (LTV)**: The maximum percentage of the collateral value that can be borrowed.
- **Liquidation Threshold (Unhealthy  LTV)**: The percentage of LTV at which a borrower's position becomes eligible for liquidation.
- **Liquidation Bonus**: The percentage bonus that liquidators receive on collateral when they liquidate a position.
- **Oracle**: Price feed to estimate the value of the collateral and the borrowed asset.
- **Oracle Validation Configuration**: Configuration to validate the price feed from the oracle. This can include parameters such as acceptable price confidence and staleness.

##### Interest Rate Model

The interest rate model is a crucial component of the market, as it determines how interest rates are calculated based on the utilization of the market. The interest rate model is defined at creation of the market and cannot be changed afterwards.

Currently Autara supports the following interest rate models:

- **Fixed Interest Rate Model**: A simple model where the borrow interest rate is fixed and does not change based on market utilization (lending rate is still based on utilization)
- **Polyline Interest Rate Model**: A more complex model where the interest rate is defined by a series of linear segments based on market utilization. This allows for more flexibility in setting interest rates and can help to better manage market dynamics. Curator can define multiple segments with at most 8 points.
- **Morpho Adaptive Interest Rate Model**: An advanced model that adjusts interest rates based on market conditions and utilization. This model is designed to optimize interest rates for both lenders and borrowers, ensuring rate converge to an optimal level over time.

### Supply Position

When a user wants to lend assets to the market, they can create a supply position by depositing the supply token into the market's supply vault. In return, they receive a share of the supply vault, which represents their ownership of the deposited assets and the interest accrued on them.
Those shares are not transferable and can only be redeemed by the original owner.

### Borrow Position

When a user wants to borrow assets from the market, they can create a borrow position by providing collateral token to the market's collateral vault and borrowing the supply token from the market's supply vault. The amount that can be borrowed is determined by the value of the collateral provided and the market's max LTV.
Borrow positions accrue interest over time, which is paid by the borrower to the supply vault. If the value of the collateral falls below the liquidation threshold, the position becomes eligible for liquidation.
Borrow Position are not transferable.

### Liquidation

If a borrower's position becomes eligible for liquidation, any user can liquidate the position by repaying a portion of the borrowed amount and receiving a portion of the collateral in return. The amount that can be liquidated is determined by the market's liquidation threshold and liquidation bonus. Liquidation amounts are tailored to be the minimum between the amount needed to bring the position back to a safe state (ie. liquidation threshold * 90%).

Liquidations are permissionless.

Example:
If a borrower has borrowed $100 worth of assets and has $150 worth of collateral, and the market's liquidation threshold is 80%, the position becomes eligible for liquidation if the value of the collateral falls below $125 (i.e., $100 / 0.8). If a liquidator repays $50 of the borrowed amount, they will receive $55 worth of collateral (i.e., $50 * (1 + 0.1 liquidation bonus)).


### Flash Loans

Autara does not support flash loans. However, some lending operation like Liquidation, BorrowAndDeposit and RepayAndWithdraw can be called using a callback function, allowing users to perform complex operations in a single atomic transaction, like leveraging. For these operations, the protocol will first transfer the tokens it owes to the user, then execute the user’s callback function, and finally collect the tokens owed to the protocol from the user.

For example, a user can call the `liquidate` function, which will transfer the collateral liquidated to the user, then execute the user's callback function like a swap to convert the collateral to the borrowed asset, and finally collect the borrowed asset from the user to repay the loan.

### Undercollateralized Positions

If a borrower's position becomes undercollateralized and is not liquidated, the curator is asked to take action.
He must socialize the loss by impacting all suppliers in the market. The loss is distributed proportionally to the amount supplied by each supplier. The curator will withdraw the collateral and is trusted to swap it back to supply asset and then donat it to the supply vault to minimize the loss for suppliers.

To avoid those situations, the curator should set conservative parameters for the market and make sure that liquidations bonus are attractive enough for liquidators to act especially in volatile market conditions. He should also monitor the available liquidity on DEX to ensure that liquidators can easily perform atomic liquidations.

Lenders should monitor the health of the market and can withdraw their funds if they believe that the market is becoming too risky but can also run a liquidation bot to liquidate undercollateralized positions and earn liquidation bonuses.
