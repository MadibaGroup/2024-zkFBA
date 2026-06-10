import numpy as np
import pandas as pd


def generate_fba_dataset(
    n_orders=1000,
    price_min=70,
    price_max=180,
    tick_size=10,
    seed=None,
    plateau_width=2,
):
    """
    Generate synthetic Frequent Batch Auction data.

    Properties:
    - Bid volume decreases with price.
    - Ask volume increases with price.
    - No empty price ticks.
    - Creates an MCV tie region.
    - Creates a small surplus tie region.
    """

    rng = np.random.default_rng(seed)

    prices = np.arange(price_min, price_max + tick_size, tick_size)
    n_ticks = len(prices)

    # --------------------------------------------------
    # 1. Generate bid-side order arrivals

    bid_weights = np.linspace(3.0, 1.0, n_ticks)
    bid_weights += rng.normal(0, 0.10, n_ticks)
    bid_weights = np.clip(bid_weights, 0.01, None)
    bid_weights /= bid_weights.sum()

    bid_orders = rng.multinomial(
        n_orders // 2,
        bid_weights
    )

    # --------------------------------------------------
    # 2. Generate ask-side order arrivals

    ask_weights = np.linspace(1.0, 3.0, n_ticks)
    ask_weights += rng.normal(0, 0.10, n_ticks)
    ask_weights = np.clip(ask_weights, 0.01, None)
    ask_weights /= ask_weights.sum()

    ask_orders = rng.multinomial(
        n_orders // 2,
        ask_weights
    )

    bid_orders += 1
    ask_orders += 1

    # --------------------------------------------------
    # 3. MCV plateau

    center = n_ticks // 2

    if center + plateau_width < n_ticks:

        target = max(
            bid_orders[center:center+plateau_width]
        )

        bid_orders[center:center+plateau_width] = target
        ask_orders[center:center+plateau_width] = target

    # --------------------------------------------------
    # 4. Auction calculations

    bid_depth = np.flip(
        np.cumsum(np.flip(bid_orders))
    )

    ask_depth = np.cumsum(ask_orders)

    executable = np.minimum(
        bid_depth,
        ask_depth
    )

    bid_surplus = np.maximum(
        bid_depth - executable,
        0
    )

    ask_surplus = np.maximum(
        ask_depth - executable,
        0
    )

    delta = np.abs(
        bid_depth - ask_depth
    )

    # --------------------------------------------------
    # 5. Force a tiny surplus tie

    mcv = executable.max()

    plateau_idx = np.where(executable == mcv)[0]

    if len(plateau_idx) >= 2:

        i = plateau_idx[len(plateau_idx)//2]

        delta[i] = delta.min()

        if i + 1 < len(delta):
            delta[i + 1] = delta.min()

   
    # --------------------------------------------------
    # Output table

    df = pd.DataFrame({
        "Price": prices,
        "Bids++": bid_orders,
        "Asks++": ask_orders,
        "Bid Depth": bid_depth,
        "Ask Depth": ask_depth,
        "Min(Bid,Ask)": executable,
        "Bid Surplus": bid_surplus,
        "Ask Surplus": ask_surplus,
        "|Delta|": delta
    })

    return df