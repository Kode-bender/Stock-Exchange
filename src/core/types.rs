use std::borrow::Cow;

pub type Price = i64;
pub type Quantity = u32;
pub type OrderId = u64;
pub type SequenceNumber = u64;
pub type Timestamp = u64;
pub type ParticipantId = u32;
pub type MatchId = u64;

pub const PRICE_SCALE: Price = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Side {
    Buy,
    Sell,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = match self {
            Side::Buy => "BUY",
            Self::Sell => "SELL",
        };

        write!(f, "{out}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OrderType {
    Limit,
    Market,
    Ioc,
    Fok,
    Gtc,
    Stop,
    StopLimit,
    Iceberg,
    PostOnly,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = match self {
            OrderType::Fok => "FOK",
            OrderType::Gtc => "GTC",
            OrderType::Iceberg => "ICEBERG",
            OrderType::Ioc => "IOC",
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
            OrderType::PostOnly => "POSTONLY",
            OrderType::Stop => "STOP",
            OrderType::StopLimit => "STOPLIMIT",
        };

        write!(f, "{out}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OrderStatus {
    New,
    Accepted,
    PartiallyFilled,
    Filled,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(pub [u8; 8]);

impl Symbol {
    pub fn new() -> Self {
        Self([0; 8])
    }

    pub fn view(&self) -> Cow<'_, str> {
        let Symbol(symbol) = self;
        let length = symbol.iter().position(|&b| b == 0).unwrap_or(symbol.len());
        String::from_utf8_lossy(&symbol[..length])
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        let mut symbol = [0u8; 8];
        let bytes = s.as_bytes();
        let len = bytes.len().min(8);
        symbol[..len].copy_from_slice(&bytes[..len]);
        Symbol(symbol)
    }
}
