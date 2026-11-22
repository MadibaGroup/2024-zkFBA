# 8. Discussion and conclusion

We prove the clearing tick by certifying three facts about the committed book: which ticks maximize ordinary volume, which of those minimize imbalance, and which qualifying tick takes priority under the announced rationing rule. The same commitments appear at the end of a public history of bounded additions. This lets a trader relate a privately acknowledged order to the calculation that determines the price.

The public output reflects a choice about what participants should be able to check. We disclose the endpoints of the maximum-volume interval to construct the verifier's selectors. At positive imbalance, $P$ and $J$ let traders compute their pre-rounding entitlements. In return these fields expose aggregate information about the book. A venue seeking to disclose only the price would need a different public statement and a separate means for traders to verify entitlements.

Our Rust implementation evaluates the cost of proving both the closing calculation and the update history. Our Noir implementation provides an independent check on the closing calculation. The choice of generator changes how often the price proof encounters a tie. On the one-second calibration sample, joint resampling of central volumes produced a volume plateau in 4.06% of crossing books, compared with 14.60% under independent draws and 3.79% in the replay.

The remaining guarantees concern different parts of the system. A venue must bind the final log digest to an instrument, grid and cutoff and specify how to allocate whole units. The present protocol leaves admission to the exchange. A joint privacy proof would also have to account for the same book commitments appearing in private receipts, the update proof and the closing proof.
