#[cfg(feature = "enable-bb8")]
pub use bb8;

#[cfg(feature = "enable-r2d2")]
pub use r2d2;

use std::{
    io::{Read, Write},
    marker::PhantomData,
    net::ToSocketAddrs,
};
use thrift::{
    protocol::{TBinaryInputProtocol, TBinaryOutputProtocol, TInputProtocol, TOutputProtocol},
    transport::{
        ReadHalf, TBufferedReadTransport, TBufferedWriteTransport, TIoChannel, TReadTransport,
        TTcpChannel, TWriteTransport, WriteHalf,
    },
};

pub trait MakeWriteTransport {
    type Channel: Write;
    type Output: TWriteTransport;
    fn make_write_transport(&self, channel: Self::Channel) -> Self::Output;
}
pub trait MakeReadTransport {
    type Channel: Read;
    type Output: TReadTransport;
    fn make_read_transport(&self, channel: Self::Channel) -> Self::Output;
}

pub struct MakeBufferedTransport<T>(PhantomData<T>);

impl<T> MakeBufferedTransport<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Default for MakeBufferedTransport<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Read> MakeReadTransport for MakeBufferedTransport<T> {
    type Channel = T;

    type Output = TBufferedReadTransport<T>;

    fn make_read_transport(&self, channel: Self::Channel) -> Self::Output {
        TBufferedReadTransport::new(channel)
    }
}

impl<T: Write> MakeWriteTransport for MakeBufferedTransport<T> {
    type Channel = T;
    type Output = TBufferedWriteTransport<T>;

    fn make_write_transport(&self, channel: Self::Channel) -> Self::Output {
        TBufferedWriteTransport::new(channel)
    }
}

pub trait MakeInputProtocol {
    type Transport: TReadTransport;
    type Output: TInputProtocol;
    fn make_input_protocol(&self, transport: Self::Transport) -> Self::Output;
}
pub trait MakeOutputProtocol {
    type Transport: TWriteTransport;
    type Output: TOutputProtocol;
    fn make_output_protocol(&self, transport: Self::Transport) -> Self::Output;
}

pub struct MakeBinaryProtocol<T> {
    strict: bool,
    _phantom: PhantomData<T>,
}

impl<T> MakeBinaryProtocol<T> {
    pub fn new(strict: bool) -> Self {
        Self {
            strict,
            _phantom: PhantomData,
        }
    }
}
impl<T> Default for MakeBinaryProtocol<T> {
    fn default() -> Self {
        Self::new(true)
    }
}

impl<T: TWriteTransport> MakeOutputProtocol for MakeBinaryProtocol<T> {
    type Transport = T;

    type Output = TBinaryOutputProtocol<T>;

    fn make_output_protocol(&self, transport: Self::Transport) -> Self::Output {
        TBinaryOutputProtocol::new(transport, self.strict)
    }
}
impl<T: TReadTransport> MakeInputProtocol for MakeBinaryProtocol<T> {
    type Transport = T;

    type Output = TBinaryInputProtocol<T>;

    fn make_input_protocol(&self, transport: Self::Transport) -> Self::Output {
        TBinaryInputProtocol::new(transport, self.strict)
    }
}

pub trait FromIoProtocol {
    type InputProtocol: TInputProtocol;
    type OutputProtocol: TOutputProtocol;
    fn from_io_protocol(
        input_protocol: Self::InputProtocol,
        output_protocol: Self::OutputProtocol,
    ) -> Self;
}

pub trait IsValid {
    fn is_valid(&mut self) -> Result<(), thrift::Error>;
}

pub trait HasBroken {
    fn has_broken(&mut self) -> bool;
}

pub struct ThriftConnectionManager<T, S: ToSocketAddrs, MIP, MOP, MRT, MWT> {
    addr: S,
    mk_i_prt: MIP,
    mk_o_prt: MOP,
    mk_r_tpt: MRT,
    mk_w_tpt: MWT,
    _t: PhantomData<T>,
}

impl<T, S: ToSocketAddrs, MIP, MOP, MRT, MWT> ThriftConnectionManager<T, S, MIP, MOP, MRT, MWT> {
    pub fn new(addr: S, mk_i_prt: MIP, mk_o_prt: MOP, mk_r_tpt: MRT, mk_w_tpt: MWT) -> Self {
        Self {
            addr,
            mk_i_prt,
            mk_o_prt,
            mk_r_tpt,
            mk_w_tpt,
            _t: PhantomData,
        }
    }
}

impl<
        S: ToSocketAddrs + Clone,
        MRT: MakeReadTransport<Channel = ReadHalf<TTcpChannel>>,
        MIP: MakeInputProtocol<Transport = MRT::Output>,
        MWT: MakeWriteTransport<Channel = WriteHalf<TTcpChannel>>,
        MOP: MakeOutputProtocol<Transport = MWT::Output>,
        T: FromIoProtocol<InputProtocol = MIP::Output, OutputProtocol = MOP::Output>,
    > ThriftConnectionManager<T, S, MIP, MOP, MRT, MWT>
{
    pub fn create_connection(&self) -> Result<T, thrift::Error> {
        let mut channel = TTcpChannel::new();
        channel.open(self.addr.clone())?;

        let (read, write) = channel.split()?;

        let read_transport = self.mk_r_tpt.make_read_transport(read);
        let input_protocol = self.mk_i_prt.make_input_protocol(read_transport);

        let write_transport = self.mk_w_tpt.make_write_transport(write);
        let output_protocol = self.mk_o_prt.make_output_protocol(write_transport);

        Ok(T::from_io_protocol(input_protocol, output_protocol))
    }
}

#[cfg(feature = "enable-bb8")]
#[async_trait::async_trait]
impl<
        S: ToSocketAddrs + Clone + Send + Sync + 'static,
        MRT: MakeReadTransport<Channel = ReadHalf<TTcpChannel>> + Send + Sync + 'static,
        MIP: MakeInputProtocol<Transport = MRT::Output> + Send + Sync + 'static,
        MWT: MakeWriteTransport<Channel = WriteHalf<TTcpChannel>> + Send + Sync + 'static,
        MOP: MakeOutputProtocol<Transport = MWT::Output> + Send + Sync + 'static,
        T: FromIoProtocol<InputProtocol = MIP::Output, OutputProtocol = MOP::Output>
            + HasBroken<S>
            + IsValid<S>
            + Send
            + Sync
            + 'static,
    > bb8::ManageConnection for ThriftConnectionManager<T, S, MIP, MOP, MRT, MWT>
{
    type Connection = T;

    type Error = thrift::Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.create_connection()
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.has_broken(&self.addr)
    }

    async fn is_valid(
        &self,
        conn: &mut bb8::PooledConnection<'_, Self>,
    ) -> Result<(), Self::Error> {
        conn.is_valid(&self.addr)
    }
}

#[cfg(feature = "enable-r2d2")]
impl<
        S: ToSocketAddrs + Clone + Send + Sync + 'static,
        MRT: MakeReadTransport<Channel = ReadHalf<TTcpChannel>> + Send + Sync + 'static,
        MIP: MakeInputProtocol<Transport = MRT::Output> + Send + Sync + 'static,
        MWT: MakeWriteTransport<Channel = WriteHalf<TTcpChannel>> + Send + Sync + 'static,
        MOP: MakeOutputProtocol<Transport = MWT::Output> + Send + Sync + 'static,
        T: FromIoProtocol<InputProtocol = MIP::Output, OutputProtocol = MOP::Output>
            + HasBroken
            + IsValid
            + Send
            + Sync
            + 'static,
    > r2d2::ManageConnection for ThriftConnectionManager<T, S, MIP, MOP, MRT, MWT>
{
    type Connection = T;

    type Error = thrift::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.create_connection()
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.has_broken()
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.is_valid()
    }
}
