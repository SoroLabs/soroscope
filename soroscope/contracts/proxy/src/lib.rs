#![no_std]

mod proxy;
mod proxy_logic;
mod proxy_logic_v2;

#[cfg(test)]
mod test;

pub use crate::proxy::{Proxy, ProxyClient};
pub use crate::proxy_logic::{ProxyLogicV1, ProxyLogicV1Client};
pub use crate::proxy_logic_v2::{ProxyLogicV2, ProxyLogicV2Client};
