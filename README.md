# Thrift Pool

This library provides a way easily to implement Connection Pools for any [`thrift::TThriftClient`],
which can then be used alongside [`bb8`] and/or [`r2d2`]

<br>

## Usage

There are 2 possible use cases

### As a library

If you're implementing a library that provides a (possibly generated) thrift client,
you should implement the [`ThriftConnection`] and [`FromProtocol`] traits
for that client

```rust
# use thrift::protocol::{TInputProtocol, TOutputProtocol};
# use thrift_pool::{FromProtocol, ThriftConnection};
#
// A typical generated client looks like this
struct MyThriftClient<Ip: TInputProtocol, Op: TOutputProtocol> {
    i_prot: Ip,
    o_prot: Op,
}

impl<Ip: TInputProtocol, Op: TOutputProtocol> FromProtocol for MyThriftClient<Ip, Op> {
    type InputProtocol = Ip;

    type OutputProtocol = Op;

    fn from_protocol(
        input_protocol: Self::InputProtocol,
        output_protocol: Self::OutputProtocol,
    ) -> Self {
        MyThriftClient {
            i_prot: input_protocol,
            o_prot: output_protocol,
        }
    }
}

impl<Ip: TInputProtocol, Op: TOutputProtocol> ThriftConnection for MyThriftClient<Ip, Op> {
    type Error = thrift::Error;
    fn is_valid(&mut self) -> Result<(), Self::Error> {
       Ok(())
    }
    fn has_broken(&mut self) -> bool {
       false
   }
}

```

### As an application

If you're implementing an application that uses a (possibly generated) thrift client that
implements [`FromProtocol`] and [`ThriftConnection`] (see previous section), you can use
[`r2d2`] or [`bb8`] (make sure to read their documentations) along with
[`ThriftConnectionManager`] to create Connection Pools for the client

```rust should_panic
# use thrift::protocol::{TInputProtocol, TOutputProtocol};
# use thrift_pool::{FromProtocol, ThriftConnection};
#
# struct MyThriftClient<Ip: TInputProtocol, Op: TOutputProtocol> {
#     i_prot: Ip,
#     o_prot: Op,
# }
#
# impl<Ip: TInputProtocol, Op: TOutputProtocol> FromProtocol for MyThriftClient<Ip, Op> {
#     type InputProtocol = Ip;
#
#     type OutputProtocol = Op;
#
#     fn from_protocol(
#         input_protocol: Self::InputProtocol,
#         output_protocol: Self::OutputProtocol,
#     ) -> Self {
#         MyThriftClient {
#             i_prot: input_protocol,
#             o_prot: output_protocol,
#         }
#     }
# }
#
# impl<Ip: TInputProtocol, Op: TOutputProtocol> ThriftConnection for MyThriftClient<Ip, Op> {
#     type Error = thrift::Error;
#     fn is_valid(&mut self) -> Result<(), Self::Error> {
#        Ok(())
#     }
#     fn has_broken(&mut self) -> bool {
#        false
#    }
# }
# use thrift_pool::{MakeThriftConnectionFromAddrs};
# use thrift::protocol::{TCompactInputProtocol, TCompactOutputProtocol};
# use thrift::transport::{
#    ReadHalf, TFramedReadTransport, TFramedWriteTransport, TTcpChannel, WriteHalf,
# };
#
type Client = MyThriftClient<
    TCompactInputProtocol<TFramedReadTransport<ReadHalf<TTcpChannel>>>,
    TCompactOutputProtocol<TFramedWriteTransport<WriteHalf<TTcpChannel>>>,
>;

# #[tokio::main]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // create a connection manager
  let manager = MakeThriftConnectionFromAddrs::<Client, _>::new("localhost:9090").into_connection_manager();
  
  // we're able to create bb8 and r2d2 Connection Pools
  let bb8 = bb8::Pool::builder().build(manager.clone()).await?;
  let r2d2 = r2d2::Pool::builder().build(manager)?;

  // get a connection
  let conn1 = bb8.get().await?;
  let conn2 = r2d2.get()?;
#  Ok(())
# }
```

<br>

## More examples

- [hbase-thrift](https://github.com/midnightexigent/hbase-thrift-rs) -- the project from which this
library was extracted. implements Connection Pools for the client generated from the
[HBase Thrift Spec](https://github.com/apache/hbase/tree/master/hbase-thrift/src/main/resources/org/apache/hadoop/hbase/thrift)
- [thrift-pool-tutorial](https://github.com/midnightexigent/thrift-pool-tutorial-rs) -- implements
Connection Pools for the client used in the official
[thrift tutorial](https://github.com/apache/thrift/tree/master/tutorial)

<br>

