
| **Kimia's Notation** | **Zeequent Notation** | **Description**                                                 |
| -------------------- | --------------------- | --------------------------------------------------------------- |
| $Min(X)$             | **TradeVol**          | The actual volume of trades that can clear at price $X$.        |
| $V_{max}$            | **MCV**               | Market Clearing Volume (the scalar global maximum of $Min(X)$). |
| $Acc_A(X)$           | **AsksDepth**         | Cumulative supply from lowest to highest price.                 |
| $Acc_B(X)$           | **BidsDepth**         | Cumulative demand from highest to lowest price.                 |
| $P_K(X)$             | **Ask Surplus**       | $Acc_A(X) - Min(X)$. Excess supply at price $X$.                |
| $P_L(X)$             | **Bid Surplus**       | $Acc_B(X) - Min(X)$. Excess demand at price $X$.                |
| $P_{Surplus}(X)$     | **Delta**             | $BidSurplus + AskSurplus$                                       |
