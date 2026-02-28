The difference between buying and selling price is too much to pay for the convenience of continuous markets.

Charles Branch, Foster & Braithwaite, 1878



Paper Pitch

* Motivate FBA against LOBs and AMMs in terms of HFT, front-running, uncertainity (slippage -> sandwhich)
* FBA require greater trust, less visibility than LOBs or AMMs



Order Types

* LOC

* MOC -> encode in LOC book as limit up and limit down

* IOC

  * Different algorithms but sensible is: ignore for MCP but use for MCI tie-breaker

  

Pro-Rata Fulfilment

* Match LOCs from best price improvement to worst (none)
* Once at MCP, determine if there is zero imbalance (done) or an imbalance (proceed)
  * Prove side of imbalance (bid or ask)
  * Show surplus amount: delta(p)
  * Show accumulated volume at one tick before p -> this is how many are allocated with priority
  * MCV - % -> how many left to allocate -> numerator of ratio
  * Show number of LOCs at p plus MOCs -> this is how many need to be split up -> denominator of ratio

Constraints

* BidsVol and AskVol with range check
* BidsDepth and AskDepth with acculumator
* MCV = min(BidsDepth, AskDepth)
  * (BidDepth-MCV) with range check (proves min is not too high)
  * (AskDepth-MCV) with range check (proves min is not too high)
  * (BidDepth-MCV)(AskDepth-MCV) = 0 (proves min is not too low)
* Delta = (BidDepth-MCV)+(AskDepth-MCV)
* MCVMax is global max on MCV
  * Selector (MCP-W) and membership check that selector*MCV is {0,MCVMax}
* Bookends of MCP-W are cliffs
  * Select bookends (cliff values) with two hot bit selector S
  * Witness is slack vector with range check Slack
    * Slack is one less than cliff size at selected bits and MCVMax everywhere else
  * MCVMax - S*MCV + S + Slack = 0 where +S ensures strictly larger than 0...
* MCIMax is global min on Delta
