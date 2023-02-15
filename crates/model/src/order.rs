#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum OrderExecution {
    Full,
    Partial { percentage: f64 },
}
