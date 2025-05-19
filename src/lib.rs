//! This library provides a simple way implement [`bb8`] and/or [`r2d2`] Connection Pools
//! for any [`TThriftClient`](thrift::TThriftClient)
//!
//! <br>
//!
//! # Usage
//!
//! There are 2 possible use cases
//!
//! ## As a library
//!
//! If you're implementing a library that provides a (possibly generated) thrift client,
//! you should implement the [`ThriftConnection`] and [`FromProtocol`] traits
//! for that client
//!
//! ```
//! # use thrift::protocol::{TInputProtocol, TOutputProtocol};
//! # use thrift_pool::{FromProtocol, ThriftConnection};
//! #
//! // A typical generated client looks like this
//! struct MyThriftClient<Ip: TInputProtocol, Op: TOutputProtocol> {
//!     i_prot: Ip,
//!     o_prot: Op,
//! }
//!
//! impl<Ip: TInputProtocol, Op: TOutputProtocol> FromProtocol for MyThriftClient<Ip, Op> {
//!     type InputProtocol = Ip;
//!
//!     type OutputProtocol = Op;
//!
//!     fn from_protocol(
//!         input_protocol: Self::InputProtocol,
//!         output_protocol: Self::OutputProtocol,
//!     ) -> Self {
//!         MyThriftClient {
//!             i_prot: input_protocol,
//!             o_prot: output_protocol,
//!         }
//!     }
//! }
//!
//! impl<Ip: TInputProtocol, Op: TOutputProtocol> ThriftConnection for MyThriftClient<Ip, Op> {
//!     type Error = thrift::Error;
//!     fn is_valid(&mut self) -> Result<(), Self::Error> {
//!         Ok(())
//!     }
//!     fn has_broken(&mut self) -> bool {
//!         false
//!     }
//! }
//! ```
//!
//! ## As an application
//!
//! If you're implementing an application that uses a (possibly generated) thrift client that
//! implements [`FromProtocol`] and [`ThriftConnection`] (see previous section), you can use
//! [`r2d2`] or [`bb8`] (make sure to read their documentations) along with
//! [`ThriftConnectionManager`] to create Connection Pools for the client
//!
//! ```
//! # use thrift::protocol::{TInputProtocol, TOutputProtocol};
//! # use thrift_pool::{FromProtocol, ThriftConnection};
//! #
//! # struct MyThriftClient<Ip: TInputProtocol, Op: TOutputProtocol> {
//! #     i_prot: Ip,
//! #     o_prot: Op,
//! # }
//! #
//! # impl<Ip: TInputProtocol, Op: TOutputProtocol> FromProtocol for MyThriftClient<Ip, Op> {
//! #     type InputProtocol = Ip;
//! #
//! #     type OutputProtocol = Op;
//! #
//! #     fn from_protocol(
//! #         input_protocol: Self::InputProtocol,
//! #         output_protocol: Self::OutputProtocol,
//! #     ) -> Self {
//! #         MyThriftClient {
//! #             i_prot: input_protocol,
//! #             o_prot: output_protocol,
//! #         }
//! #     }
//! # }
//! #
//! # impl<Ip: TInputProtocol, Op: TOutputProtocol> ThriftConnection for MyThriftClient<Ip, Op> {
//! #     type Error = thrift::Error;
//! #     fn is_valid(&mut self) -> Result<(), Self::Error> {
//! #        Ok(())
//! #     }
//! #     fn has_broken(&mut self) -> bool {
//! #        false
//! #    }
//! # }
//! # use thrift_pool::{MakeThriftConnectionFromAddrs};
//! # use thrift::protocol::{TCompactInputProtocol, TCompactOutputProtocol};
//! # use thrift::transport::{
//! #    ReadHalf, TFramedReadTransport, TFramedWriteTransport, TTcpChannel, WriteHalf,
//! # };
//! #
//! type Client = MyThriftClient<
//!     TCompactInputProtocol<TFramedReadTransport<ReadHalf<TTcpChannel>>>,
//!     TCompactOutputProtocol<TFramedWriteTransport<WriteHalf<TTcpChannel>>>,
//! >;
//! # use tokio::net::TcpListener;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let listener = TcpListener::bind("127.0.0.1:9090").await?;
//! # tokio::spawn(async move {
//! #      loop {
//! #         listener.accept().await.unwrap();
//! #      }
//! #  });
//!   // create a connection manager
//!   let manager = MakeThriftConnectionFromAddrs::<Client, _>::new("localhost:9090")
//!                 .into_connection_manager();
//!   
//!   // we're able to create bb8 and r2d2 Connection Pools
//!   let bb8 = bb8::Pool::builder().build(manager.clone()).await?;
//!   let r2d2 = r2d2::Pool::builder().build(manager)?;
//!
//!   // get a connection
//!   let conn1 = bb8.get().await?;
//!   let conn2 = r2d2.get()?;
//! #  Ok(())
//! # }
//! ```
//!
//! <br>
//!
//! # Examples
//!
//! - [hbase-thrift](https://github.com/midnightexigent/hbase-thrift-rs) -- the project from which this
//! library was extracted. implements Connection Pools for the client generated from the
//! [`HBase` Thrift Spec](https://github.com/apache/hbase/tree/master/hbase-thrift/src/main/resources/org/apache/hadoop/hbase/thrift)
//! - [thrift-pool-tutorial](https://github.com/midnightexigent/thrift-pool-tutorial-rs) -- implements
//! Connection Pools for the client used in the official
//! [thrift tutorial](https://github.com/apache/thrift/tree/master/tutorial)

use std::{
    io::{self, Read, Write},
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
    type Read: io::Read;
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
    type Write: io::Write;
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

impl<RT: TReadTransport> FromReadTransport for TBinaryInputProtocol<RT> {
    type ReadTransport = RT;

    fn from_read_transport(r_tran: RT) -> Self {
        Self::new(r_tran, true)
    }
}

impl<RT: TReadTransport> FromReadTransport for TCompactInputProtocol<RT> {
    type ReadTransport = RT;

    fn from_read_transport(r_tran: RT) -> Self {
        Self::new(r_tran)
    }
}

/// Create self from a [`TWriteTransport`]
pub trait FromWriteTransport: TOutputProtocol {
    type WriteTransport: TWriteTransport;
    fn from_write_transport(w_tran: Self::WriteTransport) -> Self;
}

impl<WT: TWriteTransport> FromWriteTransport for TBinaryOutputProtocol<WT> {
    type WriteTransport = WT;
    fn from_write_transport(w_tran: WT) -> Self {
        Self::new(w_tran, true)
    }
}

impl<WT: TWriteTransport> FromWriteTransport for TCompactOutputProtocol<WT> {
    type WriteTransport = WT;
    fn from_write_transport(w_tran: WT) -> Self {
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
    /// See [`r2d2::ManageConnection::Error`] and/or [`bb8::ManageConnection::Error`]
    type Error;

    /// See [`r2d2::ManageConnection::is_valid`] and/or [`bb8::ManageConnection::is_valid`]
    ///
    /// # Errors
    ///
    /// Should return `Err` if the connection is invalid
    fn is_valid(&mut self) -> Result<(), Self::Error>;

    /// See [`r2d2::ManageConnection::has_broken`] and/or [`bb8::ManageConnection::has_broken`]
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
    ///
    /// # Errors
    ///
    /// Should return `Err` if (for any reason)
    /// unable to create a new connection
    fn make_thrift_connection(&self) -> Result<Self::Output, Self::Error>;
}

/// A [`MakeThriftConnection`] that attempts to create new connections
/// from a [`ToSocketAddrs`] and a [`FromProtocol`]
///
/// The connection is created in accordance with the
/// [thrift rust tutorial](https://github.com/apache/thrift/tree/master/tutorial):
///
/// * Open a [`TTcpChannel`] and split it
/// * Use the created `[ReadHalf]` and `[WriteHalf]` to create [`TReadTransport`] and [`TWriteTransport`]
/// * Use those to create [`TInputProtocol`] and [`TOutputProtocol`]
/// * Create a new client with `i_prot` and `o_prot` -- It needs to implement [`FromProtocol`]
///
/// For that to happen, `T` needs to be able
/// to create the `Read`/`Write` `Transport`s
/// and `Input`/`Output` `Protocol`s from
/// the `ReadHalf` and `WriteHalf` of the `TTcpChannel`.
/// Those contraints should be fairly easily satisfied
/// by implementing the relevant traits in the library
///
/// ```
/// use thrift_pool::{FromProtocol, MakeThriftConnectionFromAddrs};
///
/// use thrift::{
///     protocol::{
///         TCompactInputProtocol, TCompactOutputProtocol, TInputProtocol, TOutputProtocol,
///     },
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
    RT: FromRead<Read = ReadHalf<TTcpChannel>>,
    IP: FromReadTransport<ReadTransport = RT>,
    WT: FromWrite<Write = WriteHalf<TTcpChannel>>,
    OP: FromWriteTransport<WriteTransport = WT>,
    T: FromProtocol<InputProtocol = IP, OutputProtocol = OP>,
> MakeThriftConnectionFromAddrs<T, S>
{
    pub fn into_connection_manager(self) -> ThriftConnectionManager<Self> {
        ThriftConnectionManager::new(self)
    }
}

impl<
    S: ToSocketAddrs + Clone,
    RT: FromRead<Read = ReadHalf<TTcpChannel>>,
    IP: FromReadTransport<ReadTransport = RT>,
    WT: FromWrite<Write = WriteHalf<TTcpChannel>>,
    OP: FromWriteTransport<WriteTransport = WT>,
    T: FromProtocol<InputProtocol = IP, OutputProtocol = OP>,
> MakeThriftConnection for MakeThriftConnectionFromAddrs<T, S>
{
    type Error = thrift::Error;

    type Output = T;

    fn make_thrift_connection(&self) -> Result<Self::Output, Self::Error> {
        let mut channel = TTcpChannel::new();
        channel.open(self.addrs.clone())?;
        let (read, write) = channel.split()?;

        let read_transport = RT::from_read(read);
        let input_protocol = IP::from_read_transport(read_transport);

        let write_transport = WT::from_write(write);
        let output_protocol = OP::from_write_transport(write_transport);

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

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
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
