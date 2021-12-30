#![doc = include_str!("../README.md")]

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
};

/// Create self from a [`Read`]
pub trait FromRead: TReadTransport {
    type Read: Read;
    fn from_read(read: Self::Read) -> Self;
}

impl<R: Read> FromRead for TBufferedReadTransport<R> {
    type Read = R;
    fn from_read(read: R) -> Self {
        Self::new(read)
    }
}
impl<R: Read> FromRead for TFramedReadTransport<R> {
    type Read = R;
    fn from_read(read: R) -> Self {
        Self::new(read)
    }
}

/// Create self from a [`Write`]
pub trait FromWrite: TWriteTransport {
    type Write: Write;
    fn from_write(write: Self::Write) -> Self;
}

impl<W: Write> FromWrite for TBufferedWriteTransport<W> {
    type Write = W;
    fn from_write(write: W) -> Self {
        Self::new(write)
    }
}

impl<W: Write> FromWrite for TFramedWriteTransport<W> {
    type Write = W;

    fn from_write(write: Self::Write) -> Self {
        Self::new(write)
    }
}

/// Create self from a [`TReadTransport`]
pub trait FromReadTransport: TInputProtocol {
    type ReadTransport: TReadTransport;
    fn from_read_transport(r_tran: Self::ReadTransport) -> Self;
}

impl<Rt: TReadTransport> FromReadTransport for TBinaryInputProtocol<Rt> {
    type ReadTransport = Rt;

    fn from_read_transport(r_tran: Rt) -> Self {
        Self::new(r_tran, true)
    }
}

impl<Rt: TReadTransport> FromReadTransport for TCompactInputProtocol<Rt> {
    type ReadTransport = Rt;

    fn from_read_transport(r_tran: Rt) -> Self {
        Self::new(r_tran)
    }
}

/// Create self from a [`TWriteTransport`]
pub trait FromWriteTransport: TOutputProtocol {
    type WriteTransport: TWriteTransport;
    fn from_write_transport(w_tran: Self::WriteTransport) -> Self;
}

impl<Wt: TWriteTransport> FromWriteTransport for TBinaryOutputProtocol<Wt> {
    type WriteTransport = Wt;
    fn from_write_transport(w_tran: Wt) -> Self {
        Self::new(w_tran, true)
    }
}

impl<Wt: TWriteTransport> FromWriteTransport for TCompactOutputProtocol<Wt> {
    type WriteTransport = Wt;
    fn from_write_transport(w_tran: Wt) -> Self {
        Self::new(w_tran)
    }
}

/// Create self from a [`TInputProtocol`] and a [`TOutputProtocol`]
pub trait FromProtocol {
    type InputProtocol: TInputProtocol;
    type OutputProtocol: TOutputProtocol;

    fn from_protocol(
        input_protocol: Self::InputProtocol,
        output_protocol: Self::OutputProtocol,
    ) -> Self;
}

/// Checks the validity of the connection
///
/// Used by [`ThriftConnectionManager`] to implement parts of
/// [`bb8::ManageConnection`] and/or [`r2d2::ManageConnection`]
pub trait ThriftConnection {
    /// See [r2d2::ManageConnection::Error] and/or [bb8::ManageConnection::Error]
    type Error;
    /// See [r2d2::ManageConnection::is_valid] and/or [bb8::ManageConnection::is_valid]
    fn is_valid(&mut self) -> Result<(), Self::Error>;
    /// See [r2d2::ManageConnection::has_broken] and/or [bb8::ManageConnection::has_broken]
    fn has_broken(&mut self) -> bool {
        false
    }
}

/// A trait that creates new [`ThriftConnection`]s
///
/// Used by [`ThriftConnectionManager`] to implement
/// [`bb8::ManageConnection::connect`]
/// and/or [`r2d2::ManageConnection::connect`]
pub trait MakeThriftConnection {
    /// The error type returned when a connection creation fails
    type Error;
    /// The connection type the we are trying to create
    type Output;
    /// Attempt to create a new connection
    fn make_thrift_connection(&self) -> Result<Self::Output, Self::Error>;
}

/// A [`MakeThriftConnection`] that attempts to create new connections
/// from a [`ToSocketAddrs`] and a [`FromProtocol`]
///
/// The connection is accordance with the
/// [thrift rust tutorial](https://github.com/apache/thrift/tree/master/tutorial):
///
/// * Open a [`TTcpChannel`]
/// * Split it
/// * Create `TReadTransport` and `TWriteTransport`
/// * Create `TInputProtocol` and `TOutputProtocol`
/// * Create a client with `i_prot` and `o_prot`
///
/// For that to happen, `T` needs to be able
/// to create the `Read`/`Write` `Transport`s
/// and `Input`/`Output` `Protocol`s from
/// the `ReadHalf` and `WriteHalf` of the `TTcpChannel`.
/// Those contraints should be fairly easily satisfied
/// by implementing the relevant traits in the library
///
/// ```
///
/// use thrift_pool::{MakeThriftConnectionFromAddrs, FromProtocol};
///
/// use thrift::{
///     protocol::{TCompactInputProtocol, TCompactOutputProtocol, TInputProtocol, TOutputProtocol},
///     transport::{
///         ReadHalf, TFramedReadTransport, TFramedWriteTransport, TIoChannel, TReadTransport,
///         TTcpChannel, TWriteTransport, WriteHalf,
///     },
/// };
///
/// // A typical generated client looks like this
/// struct MyThriftClient<Ip: TInputProtocol, Op: TOutputProtocol> {
///     i_prot: Ip,
///     o_prot: Op,
/// }
/// impl<Ip: TInputProtocol, Op: TOutputProtocol> FromProtocol for MyThriftClient<Ip, Op> {
///     type InputProtocol = Ip;
///
///     type OutputProtocol = Op;
///
///     fn from_protocol(
///         input_protocol: Self::InputProtocol,
///         output_protocol: Self::OutputProtocol,
///     ) -> Self {
///         MyThriftClient {
///             i_prot: input_protocol,
///             o_prot: output_protocol,
///         }
///     }
/// }
/// type Client = MyThriftClient<
///     TCompactInputProtocol<TFramedReadTransport<ReadHalf<TTcpChannel>>>,
///     TCompactOutputProtocol<TFramedWriteTransport<WriteHalf<TTcpChannel>>>,
/// >;
///
/// // The Protocols/Transports used in this client implement the necessary traits so we can do this
/// let manager =
///     MakeThriftConnectionFromAddrs::<Client, _>::new("localhost:9090").into_connection_manager();
///
/// ```

pub struct MakeThriftConnectionFromAddrs<T, S> {
    addrs: S,
    conn: PhantomData<T>,
}

impl<T, S: std::fmt::Debug> std::fmt::Debug for MakeThriftConnectionFromAddrs<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MakeThriftConnectionFromAddrs")
            .field("addrs", &self.addrs)
            .field("conn", &self.conn)
            .finish()
    }
}
impl<T, S: Clone> Clone for MakeThriftConnectionFromAddrs<T, S> {
    fn clone(&self) -> Self {
        Self {
            addrs: self.addrs.clone(),
            conn: PhantomData,
        }
    }
}

impl<T, S> MakeThriftConnectionFromAddrs<T, S> {
    pub fn new(addrs: S) -> Self {
        Self {
            addrs,
            conn: PhantomData,
        }
    }
}

impl<
        S: ToSocketAddrs + Clone,
        Rt: FromRead<Read = ReadHalf<TTcpChannel>>,
        Ip: FromReadTransport<ReadTransport = Rt>,
        Wt: FromWrite<Write = WriteHalf<TTcpChannel>>,
        Op: FromWriteTransport<WriteTransport = Wt>,
        T: FromProtocol<InputProtocol = Ip, OutputProtocol = Op>,
    > MakeThriftConnectionFromAddrs<T, S>
{
    pub fn into_connection_manager(self) -> ThriftConnectionManager<Self> {
        ThriftConnectionManager::new(self)
    }
}

impl<
        S: ToSocketAddrs + Clone,
        Rt: FromRead<Read = ReadHalf<TTcpChannel>>,
        Ip: FromReadTransport<ReadTransport = Rt>,
        Wt: FromWrite<Write = WriteHalf<TTcpChannel>>,
        Op: FromWriteTransport<WriteTransport = Wt>,
        T: FromProtocol<InputProtocol = Ip, OutputProtocol = Op>,
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

/// An implementor of [`bb8::ManageConnection`] and/or [`r2d2::ManageConnection`].
/// `T` should a [`MakeThriftConnection`] and `T::Output` should be a [`ThriftConnection`]
pub struct ThriftConnectionManager<T>(T);

impl<T: Clone> Clone for ThriftConnectionManager<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for ThriftConnectionManager<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ThriftConnectionManager")
            .field(&self.0)
            .finish()
    }
}

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
