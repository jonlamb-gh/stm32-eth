use crate::{rx::RxPacket, tx::TxError, Eth};
use core::intrinsics::transmute;
use core::ops::DerefMut;
use smoltcp::phy::{Device, DeviceCapabilities, RxToken, TxToken};
use smoltcp::time::Instant;
use smoltcp::Error;

/// Use this Ethernet driver with [smoltcp](https://github.com/m-labs/smoltcp)
impl<'a, 'rx, 'tx, 'b> Device<'a> for &'b mut Eth<'rx, 'tx> {
    type RxToken = EthRxToken<'a>;
    type TxToken = EthTxToken<'a>;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1500;
        caps.max_burst_size = Some(core::cmp::min(4, 4));
        caps
    }

    fn receive(&mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        let self_ = unsafe {
            // HACK: eliminate lifetimes
            transmute::<&mut Eth<'rx, 'tx>, &mut Eth<'a, 'a>>(*self)
        };
        let eth = self_ as *mut Eth<'a, 'a>;
        match self_.recv_next() {
            Ok(packet) => {
                let rx = EthRxToken { packet };
                let tx = EthTxToken { eth };
                Some((rx, tx))
            }
            Err(_) => None,
        }
    }

    fn transmit(&mut self) -> Option<Self::TxToken> {
        let eth =
            unsafe { transmute::<&mut Eth<'rx, 'tx>, &mut Eth<'a, 'a>>(*self) as *mut Eth<'a, 'a> };
        Some(EthTxToken { eth })
    }
}

pub struct EthRxToken<'a> {
    packet: RxPacket<'a>,
}

impl<'a> RxToken for EthRxToken<'a> {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut [u8]) -> Result<R, Error>,
    {
        let result = f(self.packet.deref_mut());
        self.packet.free();
        result
    }
}

/// Just a reference to [`Eth`](../struct.Eth.html) for sending a
/// packet later with [`consume()`](#method.consume)
pub struct EthTxToken<'a> {
    eth: *mut Eth<'a, 'a>,
}

impl<'a> TxToken for EthTxToken<'a> {
    /// Allocate a [`Buffer`](../struct.Buffer.html), yield with
    /// `f(buffer)`, and send it as an Ethernet packet.
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut [u8]) -> Result<R, Error>,
    {
        let eth = unsafe { &mut *self.eth };
        match eth.send(len, f) {
            Err(TxError::WouldBlock) => Err(Error::Exhausted),
            Ok(r) => r,
        }
    }
}
