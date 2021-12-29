use std::{
    io::{Read, Write},
    marker::PhantomData,
    net::ToSocketAddrs,
};

use thrift::{
    protocol::{
        TBinaryInputProtocol, TBinaryOutputProtocol, TCompactInputProtocol, TCompactOutputProtocol,
        TInputProtocol, TOutputProtocol,
    },
    transport::{
        ReadHalf, TBufferedReadTransport, TBufferedWriteTransport, TFramedReadTransport,
        TFramedWriteTransport, TIoChannel, TReadTransport, TTcpChannel, TWriteTransport, WriteHalf,
    },
    TThriftClient,
};

pub trait ReadTransportExt: TReadTransport {
    type Read: Read;
    fn from_read(read: Self::Read) -> Self;
}
impl<R: Read> ReadTransportExt for TBufferedReadTransport<R> {
    type Read = R;
    fn from_read(read: R) -> Self {
        Self::new(read)
    }
}
impl<R: Read> ReadTransportExt for TFramedReadTransport<R> {
    type Read = R;
    fn from_read(read: R) -> Self {
        Self::new(read)
    }
}
pub trait WriteTransportExt: TWriteTransport {
    type Write: Write;
    fn from_write(write: Self::Write) -> Self;
}
impl<W: Write> WriteTransportExt for TBufferedWriteTransport<W> {
    type Write = W;
    fn from_write(write: W) -> Self {
        Self::new(write)
    }
}
impl<W: Write> WriteTransportExt for TFramedWriteTransport<W> {
    type Write = W;

    fn from_write(write: Self::Write) -> Self {
        Self::new(write)
    }
}
pub trait InputProtocolExt: TInputProtocol {
    type ReadTransport: TReadTransport;
    fn from_read_transport(r_tran: Self::ReadTransport) -> Self;
}
impl<Rt: TReadTransport> InputProtocolExt for TBinaryInputProtocol<Rt> {
    type ReadTransport = Rt;

    fn from_read_transport(r_tran: Rt) -> Self {
        Self::new(r_tran, true)
    }
}
impl<Rt: TReadTransport> InputProtocolExt for TCompactInputProtocol<Rt> {
    type ReadTransport = Rt;

    fn from_read_transport(r_tran: Rt) -> Self {
        Self::new(r_tran)
    }
}
pub trait OutputProtocolExt: TOutputProtocol {
    type WriteTransport: TWriteTransport;
    fn from_write_transport(w_tran: Self::WriteTransport) -> Self;
}
impl<Wt: TWriteTransport> OutputProtocolExt for TBinaryOutputProtocol<Wt> {
    type WriteTransport = Wt;
    fn from_write_transport(w_tran: Wt) -> Self {
        Self::new(w_tran, true)
    }
}
impl<Wt: TWriteTransport> OutputProtocolExt for TCompactOutputProtocol<Wt> {
    type WriteTransport = Wt;
    fn from_write_transport(w_tran: Wt) -> Self {
        Self::new(w_tran)
    }
}

pub trait ThriftClientExt: TThriftClient {
    type InputProtocol: TInputProtocol;
    type OutputProtocol: TOutputProtocol;

    fn from_protocol(
        input_protocol: Self::InputProtocol,
        output_protocol: Self::OutputProtocol,
    ) -> Self;
}

pub trait ThriftConnection {
    type Error;
    fn is_valid(&mut self) -> Result<(), Self::Error>;
    fn has_broken(&mut self) -> bool;
}

pub trait MakeThriftConnection {
    type Error;
    type Output;
    fn make_thrift_connection(&self) -> Result<Self::Output, Self::Error>;
}

pub struct MakeThriftConnectionFromAddrs<T, S> {
    addrs: S,
    _t: PhantomData<T>,
}

impl<T, S> MakeThriftConnectionFromAddrs<T, S> {
    pub fn new(addrs: S) -> Self {
        Self {
            addrs,
            _t: PhantomData,
        }
    }
}

impl<
        S: ToSocketAddrs + Clone,
        Rt: ReadTransportExt<Read = ReadHalf<TTcpChannel>>,
        Ip: InputProtocolExt<ReadTransport = Rt>,
        Wt: WriteTransportExt<Write = WriteHalf<TTcpChannel>>,
        Op: OutputProtocolExt<WriteTransport = Wt>,
        T: ThriftClientExt<InputProtocol = Ip, OutputProtocol = Op>,
    > MakeThriftConnectionFromAddrs<T, S>
{
    pub fn into_connection_manager(self) -> ThriftConnectionManager<Self> {
        ThriftConnectionManager::new(self)
    }
}

impl<
        S: ToSocketAddrs + Clone,
        Rt: ReadTransportExt<Read = ReadHalf<TTcpChannel>>,
        Ip: InputProtocolExt<ReadTransport = Rt>,
        Wt: WriteTransportExt<Write = WriteHalf<TTcpChannel>>,
        Op: OutputProtocolExt<WriteTransport = Wt>,
        T: ThriftClientExt<InputProtocol = Ip, OutputProtocol = Op>,
    > MakeThriftConnection for MakeThriftConnectionFromAddrs<T, S>
{
    type Error = thrift::Error;

    type Output = T;

    fn make_thrift_connection(&self) -> Result<Self::Output, Self::Error> {
        let mut channel = TTcpChannel::new();
        channel.open(self.addrs.clone())?;
        let (read, write) = channel.split()?;

        let read_transport = Rt::from_read(read);
        let input_protocol = Ip::from_read_transport(read_transport);

        let write_transport = Wt::from_write(write);
        let output_protocol = Op::from_write_transport(write_transport);

        Ok(T::from_protocol(input_protocol, output_protocol))
    }
}

pub struct ThriftConnectionManager<T>(T);
impl<T> ThriftConnectionManager<T> {
    pub fn new(make_thrift_connection: T) -> Self {
        Self(make_thrift_connection)
    }
}

#[cfg(feature = "impl-bb8")]
#[async_trait::async_trait]
impl<
        E: Send + std::fmt::Debug + 'static,
        C: ThriftConnection<Error = E> + Send + 'static,
        T: MakeThriftConnection<Output = C, Error = E> + Send + Sync + 'static,
    > bb8::ManageConnection for ThriftConnectionManager<T>
{
    type Connection = C;

    type Error = E;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.0.make_thrift_connection()
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.has_broken()
    }

    async fn is_valid(
        &self,
        conn: &mut bb8::PooledConnection<'_, Self>,
    ) -> Result<(), Self::Error> {
        conn.is_valid()
    }
}

#[cfg(feature = "impl-r2d2")]
impl<
        E: std::error::Error + 'static,
        C: ThriftConnection<Error = E> + Send + 'static,
        T: MakeThriftConnection<Output = C, Error = E> + Send + Sync + 'static,
    > r2d2::ManageConnection for ThriftConnectionManager<T>
{
    type Connection = C;

    type Error = E;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.0.make_thrift_connection()
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.has_broken()
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.is_valid()
    }
}
