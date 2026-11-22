//! Shared data types for Zeequent: parameters, orders, the order book, and the public clearing receipt.

/// Fixed protocol parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    /// `N`: number of real price ticks.
    pub n_ticks: usize,
    /// `n`: evaluation-domain size, a power of two with `n >= N`.
    pub n: usize,
    /// `k_in`: bit width of the volume resting at a single tick.
    pub k_in: u32,
    /// `k`: bit width of every accumulated quantity, `k = k_in + ceil(log2 N)`.
    pub k: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamError {
    NoTicks,
    ZeroWidth,
    /// `k` must stay small enough for `u64` arithmetic and for `2^(k+1) < q`.
    WidthTooLarge { k: u32 },
}

impl Params {
    pub const MAX_K: u32 = 62;

    pub fn new(n_ticks: usize, k_in: u32) -> Result<Self, ParamError> {
        if n_ticks == 0 {
            return Err(ParamError::NoTicks);
        }
        if k_in == 0 {
            return Err(ParamError::ZeroWidth);
        }
        let n = n_ticks.next_power_of_two().max(2);
        let k = k_in + ceil_log2(n_ticks);
        if k > Self::MAX_K {
            return Err(ParamError::WidthTooLarge { k });
        }
        Ok(Self { n_ticks, n, k_in, k })
    }

    /// Exclusive upper bound on a single tick's volume, `2^k_in`.
    pub fn input_bound(&self) -> u64 {
        1u64 << self.k_in
    }

    /// Exclusive upper bound on every accumulated quantity, `2^k`.
    pub fn bound(&self) -> u64 {
        1u64 << self.k
    }
}

/// Smallest `e` with `2^e >= x` (and 0 for `x <= 1`).
pub fn ceil_log2(x: usize) -> u32 {
    if x <= 1 {
        0
    } else {
        usize::BITS - (x - 1).leading_zeros()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderKind {
    /// Limit order at a price tick.
    Limit { tick: usize },
    /// Market order.
    Market,
    /// Imbalance-only order with a limit tick.
    Io { tick: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Order {
    pub side: OrderSide,
    pub kind: OrderKind,
    pub qty: u64,
}

/// The four committed input columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Bids,
    Asks,
    IoBids,
    IoAsks,
}

impl Column {
    pub fn from_index(index: usize) -> Option<Self> {
        [Column::Bids, Column::Asks, Column::IoBids, Column::IoAsks]
            .get(index)
            .copied()
    }

    /// Position in any `[_; 4]` array of per-column book data.
    pub fn index(self) -> usize {
        match self {
            Column::Bids => 0,
            Column::Asks => 1,
            Column::IoBids => 2,
            Column::IoAsks => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookError {
    TickOutOfRange { tick: usize, n_ticks: usize },
    Overflow,
}

/// Tick-indexed order book: four histograms of length `N`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    pub bids: Vec<u64>,
    pub asks: Vec<u64>,
    pub io_bids: Vec<u64>,
    pub io_asks: Vec<u64>,
}

impl Book {
    pub fn new(n_ticks: usize) -> Self {
        Self {
            bids: vec![0; n_ticks],
            asks: vec![0; n_ticks],
            io_bids: vec![0; n_ticks],
            io_asks: vec![0; n_ticks],
        }
    }

    pub fn n_ticks(&self) -> usize {
        self.bids.len()
    }

    /// The column and tick an order is recorded at.
    pub fn slot(&self, order: &Order) -> Result<(Column, usize), BookError> {
        let n_ticks = self.n_ticks();
        let in_range = |tick: usize| {
            if tick < n_ticks {
                Ok(tick)
            } else {
                Err(BookError::TickOutOfRange { tick, n_ticks })
            }
        };
        Ok(match (order.side, order.kind) {
            (OrderSide::Buy, OrderKind::Limit { tick }) => (Column::Bids, in_range(tick)?),
            (OrderSide::Sell, OrderKind::Limit { tick }) => (Column::Asks, in_range(tick)?),
            (OrderSide::Buy, OrderKind::Market) => (Column::Bids, n_ticks - 1),
            (OrderSide::Sell, OrderKind::Market) => (Column::Asks, 0),
            (OrderSide::Buy, OrderKind::Io { tick }) => (Column::IoBids, in_range(tick)?),
            (OrderSide::Sell, OrderKind::Io { tick }) => (Column::IoAsks, in_range(tick)?),
        })
    }

    pub fn column(&self, column: Column) -> &[u64] {
        match column {
            Column::Bids => &self.bids,
            Column::Asks => &self.asks,
            Column::IoBids => &self.io_bids,
            Column::IoAsks => &self.io_asks,
        }
    }

    fn column_mut(&mut self, column: Column) -> &mut Vec<u64> {
        match column {
            Column::Bids => &mut self.bids,
            Column::Asks => &mut self.asks,
            Column::IoBids => &mut self.io_bids,
            Column::IoAsks => &mut self.io_asks,
        }
    }

    /// Adds an order's volume to its slot and returns that slot.
    pub fn add(&mut self, order: &Order) -> Result<(Column, usize), BookError> {
        let (column, tick) = self.slot(order)?;
        let cell = &mut self.column_mut(column)[tick];
        *cell = cell.checked_add(order.qty).ok_or(BookError::Overflow)?;
        Ok((column, tick))
    }
}

/// Which side has unfilled volume at the clearing tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// `s = 0`: cumulative demand exceeds cumulative supply (buy imbalance).
    DemandLong,
    /// `s = 1`: cumulative supply exceeds cumulative demand.
    SupplyLong,
}

impl Side {
    /// The public bit `s`.
    pub fn bit(self) -> u64 {
        match self {
            Side::DemandLong => 0,
            Side::SupplyLong => 1,
        }
    }
}

/// The public clearing receipt of a batch that crosses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    /// `V_max`: maximum executable volume.
    pub v_max: u64,
    /// `c`: first tick of the plateau.
    pub c: usize,
    /// `d`: last tick of the plateau.
    pub d: usize,
    /// `p*`: clearing tick.
    pub p_star: usize,
    /// `V_minDelta`: imbalance at `p*`, the minimum over the plateau.
    pub v_min_delta: u64,
    /// `s`: imbalance side.
    pub side: Side,
    /// `P`: long-side volume resting at exactly `p*`.
    pub p_at_price: u64,
    /// `J`: short-side IO volume eligible at `p*`.
    pub j_io: u64,
}

impl Receipt {
    /// `I = min(J, V_minDelta)`: IO volume admitted.
    pub fn admitted_io(&self) -> u64 {
        self.j_io.min(self.v_min_delta)
    }

    /// `R = V_minDelta - I`: imbalance left unfilled on the long side.
    pub fn residual(&self) -> u64 {
        self.v_min_delta - self.admitted_io()
    }

    /// `V_exec = V_max + I`: total traded volume.
    pub fn v_exec(&self) -> u64 {
        self.v_max + self.admitted_io()
    }

    /// `nu = [V_minDelta > 0]`.
    pub fn has_imbalance(&self) -> bool {
        self.v_min_delta > 0
    }
}

/// Result of clearing one batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// `V_max = 0`: no price crosses.
    NoCross,
    Cross(Receipt),
}

impl Outcome {
    /// The outcome as `key=value` lines.
    pub fn to_text(&self) -> String {
        match self {
            Outcome::NoCross => "outcome=no-cross\n".to_string(),
            Outcome::Cross(r) => format!(
                "outcome=cross\nv_max={}\nc={}\nd={}\np_star={}\nv_min_delta={}\nside={}\np_at_price={}\nj_io={}\n",
                r.v_max,
                r.c,
                r.d,
                r.p_star,
                r.v_min_delta,
                match r.side {
                    Side::DemandLong => "demand-long",
                    Side::SupplyLong => "supply-long",
                },
                r.p_at_price,
                r.j_io
            ),
        }
    }

    pub fn from_text(text: &str) -> Option<Self> {
        let field = |name: &str| {
            text.lines()
                .filter_map(|line| line.split_once('='))
                .find(|(key, _)| key.trim() == name)
                .map(|(_, value)| value.trim())
        };
        match field("outcome")? {
            "no-cross" => Some(Outcome::NoCross),
            "cross" => Some(Outcome::Cross(Receipt {
                v_max: field("v_max")?.parse().ok()?,
                c: field("c")?.parse().ok()?,
                d: field("d")?.parse().ok()?,
                p_star: field("p_star")?.parse().ok()?,
                v_min_delta: field("v_min_delta")?.parse().ok()?,
                side: match field("side")? {
                    "demand-long" => Side::DemandLong,
                    "supply-long" => Side::SupplyLong,
                    _ => return None,
                },
                p_at_price: field("p_at_price")?.parse().ok()?,
                j_io: field("j_io")?.parse().ok()?,
            })),
            _ => None,
        }
    }
}
