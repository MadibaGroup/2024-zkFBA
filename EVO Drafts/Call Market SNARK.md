# Call Market SNARK

# Unindexed

## Setting up Bid Arrays

Need to decide exactly what method is used to handled bid submission, but here is the rough idea. Each buy submits a commitment to their bid to the auctioneer, which has the direction, price, and quantity. At the end of the bidding process they open their bids for the auctioneer. The auctioneer then compiles a total of four arrays, we will say they are of length $n$. The first array, $\mathsf{Arr_{b1}}$, is an (unsorted) list of all the bidders buy prices. The second array, $\mathsf{Arr_{b2}}$ contains the quantity of each bid at the same index as the corresponding price is at in $\mathsf{Arr_{b1}}$. $\mathsf{Arr_{s1}}$ and $\mathsf{Arr_{s2}}$ are the same buy for the sell bids. To prove to each bidder that their bid has been included, the auctioneer opens $-$ say if the bid is to buy $-$ $\mathsf{Arr_{b1}}$ and $\mathsf{Arr_{b2}}$ at the index corresponding to that bidders bid. If we are worried about the auctioneer opening the same bid for two bidders (this would only work if they bid identically for direction, price, and quantity), we could also make the auctioneer commit to some randomness unique to the bidder (this could be derived from/use the randomness from the bidder's original commitment) in a third array at their index, and open that as well.

## Proving Market Clearing Price

The auctioneer interpolates the four arrays into polynomials (using evaluation points). They claim that the market clearing interval is $[c,d]$. Further, they claim that for some $k \in [c,d]$, the total number of buy bids at $k$ is greater than or equal to the total number of sell bids at $k$, and that at $k+1$, the total number of sell bids is greater than the total number of buy bids. To prove this, they will create 8 boolean vectors. Four of the vectors correspond to the buy bids, indicating whether the bid at each index is included in a certain range (1 means yes, 0 means no), and the other four correspond to the sell bids. The vectors are defined as follows:

- $\mathsf{V}_{s,c}$ indicates all sell bids with price in the range $[0,c]$
- $\mathsf{V}_{s,c-1}$ indicates all sell bids with price in the range $[0, c-1]$
- $\mathsf{V}_{b,d}$ indicates all buy bids with price in the range $[d, \text{max}]$
- $\mathsf{V}_{b,d+1}$ indicates all buy bids with price in the range $[d+1, \text{max}]$
- $\mathsf{V}_{s,k}$ indicates all sell bids with price in the range $[0, k]$
- $\mathsf{V}_{s,k-1}$ indicates all sell bids with price in the range $[0, k+1]$
- $\mathsf{V}_{b,k}$ indicates all sell bids with price in the range $[k, \text{max}]$
- $\mathsf{V}_{b,k-1}$ indicates all sell bids with price in the range $[k+1, \text{max}]$

A Positive-Negative lookup argument (as described in the IZPR paper) is used to prove each of these vectors are correct.

Now we use an addition check on the bid quantity polynomial times the correct boolean vector to prove entries in a polynomial represents commitments to certain sums; we can commit to vector containing all of the sums in as a polynomial (then each sum can be access by evaluating the polynomial at its corresponding index). If we do not care about keeping these sums hidden, we can simply use an addition check proving the value of the sum, without the polynomial commiting to these sums. 

- total sells at $c$  $= t_{s,c} = \mathsf{Poly_{sums}}(\omega^j) = \sum_{i \lt n} \mathsf{Poly_{Accs2}}(\omega^i) \cdot \mathsf{Poly_{V}}_{s,c}(\omega^i)$. Then the addition check uses $\mathsf{{Poly_{sums}}}(\omega^j)$ in the place of $\mathsf{Sum_{Arr}}$.

And similarly for commitments to the following sums:

- $t_{s,c-1} = $ total sells at $c-1$
- $t_{b,d} = $ total buys at $d$
- $t_{b,d+1} = $ total buys at $d+1$
- $t_{b,k} = $ total buys at $k$
- $t_{s,k} = $ total sells at $k$
- $t_{b,k+1} = $ total buys at $k+1$
- $t_{s, k+1}=$ total sells at $k+1$

We then prove the following:

1. Buys greater than (or equal to) sells at $k$, sells greater buys at $k+1$; by:
   - range check to show $t_{b,k} - t_{s,k}$ non negative
   - range check to show $t_{s,k+1} - t_{b,k+1}$ strictly positive

2. There are the same number of trades at $c, k, k+1, d$; by:
   - showing $(t_{s,c} - t_{s,k}) + (t_{s,c} - t_{b,k+1})\rho + (t_{s,c} - t_{b, d})\rho^2 = 0$ for a random evalution point $\rho$, chosen after the accumulators and boolean vectors have been commited to
   - this check could also be be done by checking that $\mathsf{Poly_{sums}}(\omega^j) - \mathsf{Poly_{sums}}(\omega^{j+1}) = 0$, for $j \in [l, l+2]$, where $c, k, k+1, d$ occupy the 4 consecutive entries represented by $[l, l+3]$

3. There are fewer trades at $c-1$ than $c$; by
   - range check to show $t_{s,c} - t_{s,c-1}$ strictly positive

4. There are fewer trades at $d+1$ than $d$; by
   - range check to show $t_{b,d} - t_{b,d+1}$ strictly positive

Then the market clearing price is $p = \frac{c+d}{2}$. The information from this step that is public (not hidden) is $p, c, k, d$ $-$ should any of these instead be kept secret? Since prices increment in "ticks," the interval may not be evenly divisible by 2. In this case, we round up or down based on the parity of the quotient's tick.

## Allocating Trades

We once again use boolean vectors to prove the desired results. This time, we define the following two boolean vectors:

- $\mathsf{V}_{s,p}$ indicates all sell bids with price in the range $[0,p]$
- $\mathsf{V}_{b,p}$ indicates all buy bids with price in the range $[p,\text{max}]$

Similarly to above, we use these vectors to prove commitments to the following sums:

- $t_{s,p} =$ total sells at $p$
- $t_{b,p} = $ total buys at $p$

Whichever is smaller is the number of trades that go through; this is proven by a range check to prove the difference between bids at $p$ in each direction is positive. If the number of sell bids, $s$, and the number of buy bids, $b$, at $p$ are equal, then allocating trades is trivial. Thus we assume they are not equal, and deal with the case $b \lt s$ (the converse follows relatively symmetrically). 

There will be some price $j$, for which all buy bids at that price and greater go through, but, for $j+1$ some number of buy bids (but not all) will go through. We now create two more boolean vectors:

- $\mathsf{V}_{b,j}$ indicates all buy bids with price in the range $[j,\text{max}]$
- $\mathsf{V}_{b,j+1}$ indicates all buy bids with price in the range $[j+1,\text{max}]$

Once again, we use these vectors to prove commitments to the following sums:

- $t_{s,p} =$ total sells at $p$
- $t_{b,p} = $ total buys at $p$

And use two more range proofs to show that the buys at $j$ is less than the total trades that go through, and the buys at $j+1$ is more than the total trades that go through.

We then decide which of the bids at $j+1$ go through the same way as for indexed (see below for an explanation).

## Security Proofs



---

# Indexed

## Setting up Bid Arrays

Need to decide exactly what method is used to handled bid submission, but here is the rough idea. To submit a bid, the bidder submits a (KZG) commitment to a vector which has zeroes everywhere, except at the index of the price they want to bid at, where they put the number of bids they'd like to make. They must also send a Pedersen commitment to either $1$ or $0$, indicating whether they are buying or selling. After the bidding period, bidders will open their commitments to the auctioneer.

The auction starts with an order book for buys and an order book for sells, both of which are the zero polynomial. They create a 2-dimensional array of all the buy bid vectors, and a separate one with all the sell bid vectors. They then create an addition check to prove the polynomial that each of these sum to; in other words, these two polynomials are the interpolations of $\mathsf{Arr_b}$ and $\mathsf{Arr_s}$. To prove to each bidder that their bid was included, the auction opens the correct index for them, and shows the rest of the row (assuming the rows are the bid vectors in the 2D array) is zero.

## Proving Market Clearing Price

ARRAY LEVEL

Let $\mathsf{Arr_b}$ and $\mathsf{Arr_s}$ be indexed arrays (length $n$) of all the buy and sell bids respectively. We create accumulators for each, $\mathsf{Acc_b}$ and $\mathsf{Acc_s}$: 

- $\mathsf{Acc_b}[n-1] = \mathsf{Arr_b}[n-1]$ and $\mathsf{Acc_b}[i] = \mathsf{Acc_b}[i + 1] + \mathsf{Arr_b}[i]$ for $0 \leq i \leq n - 2$
- $\mathsf{Acc_s}[0] = \mathsf{Arr_s}[0]$ and $\mathsf{Acc_s}[i] = \mathsf{Acc_s}[i-1] + \mathsf{Arr_s}[i]$ for $1 \leq i \leq n-1$

These accumulators can be thought of as a sort of discrete integral of the arrays; they represent how many bidders, total, are willing to buy or self at each price. If we think of a piecewise function connecting points for each array, with indices as the $x$ values, and the number of bids as the $y$ value. Where the two functions intersect $-$ which would only occur once (or only a line) since the functions are decreasing and increasing, respectively $-$ would be the marketclearing price (or if they intersect along a line, interval). We claim market clearing interval is $[c, d]$, and define $k$ as in the Unindexed section. To prove this we define four more arrays:

- $\mathsf{Diff_1}[i] = \mathsf{Acc_b}[i] - \mathsf{Acc_s}[i] \space \forall i \lt c$ 
- $\mathsf{Diff_2}[i] = \mathsf{Acc_s}[i] - \mathsf{Acc_b}[i] \space c \leq i \leq k$
- $\mathsf{Diff_3}[i] = \mathsf{Acc_b}[i] - \mathsf{Acc_s}[i] \space k \gt i \leq d$ 
- $\mathsf{Diff_4}[i] = \mathsf{Acc_s}[i] - \mathsf{Acc_b}[i] \space \forall i \gt d$

To prove $[c,d]$ is the market clearing interval we:

1. Use a lookup (or range?) proof to show that all entries of $\mathsf{Diff_1}[i]$ and $\mathsf{Diff_4}[i]$ are strictly positive; this implies that $a$ is somewhere in the interval $[c,d]$.
2. All entries in $\mathsf{Diff_2}[i]$ and $\mathsf{Diff_3}[i]$ are equal; this implies that the whole interval is equivalently market clearing.

Then the market clearing interval is $[c,d]$ and our clearing price is the half way point. In other words, the market clearing price, $p$, is equal to $\frac{c+d}{2}$. Since prices increment in "ticks," the interval may not be evenly divisible by 2. In this case, we round up or down based on the parity of the quotient's tick.

POLYNOMIAL LEVEL

We have $\mathsf{Poly_{Arr_b}}$ and $\mathsf{Poly_{Arr_s}}$, the polynomial interpolations of $\mathsf{Arr_b}$ and $\mathsf{Arr_s}$. We prove the accumulators $\mathsf{Poly_{Acc_b}}$ and $\mathsf{Poly_{Acc_s}}$ are constructed correctly by showing:

1. $\mathsf{Poly}_\mathsf{Vanish1}(X)=(\mathsf{Poly}_\mathsf{Acc_s}(X)-\mathsf{Poly}_\mathsf{Arr_s}(X))\cdot\frac{(X^\kappa-1)}{(X-\omega^{\kappa-1})}=0$,
2. $\mathsf{Poly}_\mathsf{Vanish2}(X)=(\mathsf{Poly}_\mathsf{Acc_s}(X)-\mathsf{Poly}_\mathsf{Arr_s}(X)+\mathsf{Poly}_\mathsf{Acc_s}(\omega\cdot X))\cdot(X-\omega^{\kappa-1})=0$ 

And the accumulators collect in the opposite direction for $\mathsf{Arr_b}$:

1. $\mathsf{Poly}_\mathsf{Vanish3}(X)=(\mathsf{Poly}_\mathsf{Acc}(X)-\mathsf{Poly}_\mathsf{Arr}(X))\cdot\frac{(X^\kappa-1)}{(X-\omega^{0})}=0$,
2. $\mathsf{Poly}_\mathsf{Vanish4}(X)=(\mathsf{Poly}_\mathsf{Acc}(X)-\mathsf{Poly}_\mathsf{Arr}(X)+\mathsf{Poly}_\mathsf{Acc}(\omega^{-1} \cdot X))\cdot(X-\omega^{0})=0$ 

Prove $\mathsf{Poly_{Diffi}}$  constructed correctly by showing:

1. $\mathsf{Poly_{Vanish5}}(X) = [\mathsf{Poly_{Diff1}}(X) - (\mathsf{Poly_{Acc_b}}(X) - \mathsf{Poly_{Acc_s}}(X))](\mathsf{Poly_{z}}(X)) $

Where $\mathsf{Poly_{z}}$ is the polynomials that zero out elements for $i \geq c$, following the zero2 strategy. And we construct $\mathsf{Poly_{Vanish6}}(X)$, $\mathsf{Poly_{Vanish7}}(X)$, $\mathsf{Poly_{Vanish8}}(X)$ similarly for $\mathsf{Poly_{Diff2}}$, $\mathsf{Poly_{Diff3}}$, $\mathsf{Poly_{Diff4}}$.

We then run a lookup range proof showing $\mathsf{Poly_{Diff1}}(\omega^{c -1})$ and $\mathsf{Poly_{Diff4}}(\omega^{d+1})$ are (strictly) positive to show that all entries in the arrays $\mathsf{Diff_1}$ and $\mathsf{Diff_4}$ are positive.

We also check all entries in $\mathsf{Diff_2}$ and $\mathsf{Diff_3}$ are equal by showing that $\mathsf{Poly_{Diff2}}(\omega^c) - \mathsf{Poly_{Diff2}}(\omega^j) = 0$ for $j \in [c, k]$ and $\mathsf{Poly_{Diff2}}(\omega^c) - \mathsf{Poly_{Diff3}}(\omega^j) = 0$ for $j \in [k+1, d]$.

And thus the market clearing price is $p =\frac{c + d}{2}$. Since prices increment in "ticks," the interval may not be evenly divisible by 2. In this case, we round up or down based on the parity of the quotient's tick. In this step of the protocol, the information that is public (not hidden) is $p,c, k, d$ $-$ the same as for the unindexed version.

We batch the proof of the individual vanishing polynomials into a single polynomial by making each $\mathsf{Poly_{Vanishi}}$ into a coffecient for a new polynomial, evaluated at a random point $\rho$, which is chosen after the commitments to the early polynomials are fixed.

## Allocating Trades

- unsure how much of this process/the information here like number of trades in each direction, trade imbalance, who gets to trade (and with whom), etc should be made public

If the number of sell bids, $s$, and the number of buy bids, $b$, at $p$ are equal, then allocating trades is trivial. Thus we assume they are not equal, and deal with the case $b \lt s$ (the converse follows relatively symmetrically).

The values of $b$ and $s$ are simply the opening of their respective accumulator at $p$. If $b$ and $s$ are published publically, we clearly see which is larger, and how many trades go through total (which is equal to the smaller value). If $b$ and $s$ are to be kept hidden, we use a range proof on their difference to prove which is larger, and once again the smaller value is the number of trade that go through (we can open the commit to the smaller value if we want this to be public).

There will be some price $j$, for which all buy bids at that price and greater go through, but, for $j+1$ some number of buy bids (but not all) will go through. We prove the correct $j$ with two range proofs, showing that both $b - \mathsf{Poly_{Acc_b}}(\omega^j)$ and $\mathsf{Poly_{Acc_b}}(\omega^{j+ 1}) - b$ are positive.

In the current scheme, $j$ is not hidden. I am unsure if it should be hidden? If so, I will look into this more, but it seems it will be more complicated both for this step, and the next one (where, at least for the random distribution, buyers who bid at $j+1$ will know $-$ is it then more fair if everyone, not just these bidders gets knowledge of $j$ and $j+1$?).

We then determine which buy bids at $j+1$ go through either pro-rata, or by random-distribution. Pro-rata is more complex to solve (I have devised part of a scheme, but it does not scale well beyond 2 bidders and we would need to prove how many bids each person has at $j+1$), so below we explore random distribution.

Consider a list $[0, m-1]$ where $m$ is the number of buyers at $j + 1$. The auctioneer sends each buyer at $j+1$ an interval of length $l$, where $l$ is the number of bids they have at $j+1$.  

- buyers could be assigned a list of $l$ random indices in $[0, m-1]$, instead of an interval, if assigning intervals leaks knowledge that two winning buy bids are likely to belong to the same bidder

- The auctioneer could also create an array to represent the $m-1$ bids at $j+1$ and commit to it. They would then send each buyer at $m+1$ an opening proving the interval (/points) assigned to them. Once random indices have been chosen for winners, they would publish opening to these indices. This would prevent potentially tampering of the winners, but I am unsure if it is necessary?

A random number, $r_0$, must now be generated. Below we explore different options for this.

- One method would be to use a random beacon; however, I need to look into this more to see if there is a way to ensure the auctioneer uses the first number they generate this way, and cannot keep getting number from the random beacon until it gives them an index they like
- Another method would be the have the auctioneer and each buyer at $i+1$ commit to a random value, then open those values, and use the product of them as the random number. This has the draw back of adding rounds of communication, but it prevents the auctioneer from generating random values until they get one they like. One potential issue that I need to clarify $-$ can anyone choose their value such that it tampers with the result (especially considering the product gets modded by $m$; if tampering is possible, could we fix this by hashing the product before we mod it to get the first index?)

The first random index will be $r_0 \space \mathsf{mod} \space m$. We then generate our second random number as $r_1 = \mathsf{hash}(r_0)$ and the second random index is thus $r_1 \space \mathsf{mod} \space m$. Then the third random number is $r_2 = \mathsf{hash}(r_1)$ and we continue this way until sufficient random indices have been generated to determine all buyers at $i +1$ who get to trade.

## Security Proofs
