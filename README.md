# Thrift Pool

This library provides a simple way implement [`bb8`](https://crates.io/crates/bb8)
and/or [`r2d2`](https://crates.io/crates/r2d2) Connection Pools
for any [`TThriftClient`](https://docs.rs/thrift/0.15.0/thrift/trait.TThriftClient.html)

## Documenation

- This library is primarily documented on its 
[docs.rs](https://docs.rs/thrift-pool/latest/thrift_pool/) page

- Here is a quick example 

```rust
use thrift::protocol::{TCompactInputProtocol, TCompactOutputProtocol, TInputProtocol, TOutputProtocol};
use thrift::transport::{
    ReadHalf, TFramedReadTransport, TFramedWriteTransport, TTcpChannel, WriteHalf,
};
use thrift_pool::{MakeThriftConnectionFromAddrs, ThriftConnectionManager, ThriftConnection, FromProtocol};

// A typical generated client
struct MyThriftClient<Ip: TInputProtocol, Op: TOutputProtocol> {
    i_prot: Ip,
    o_prot: Op,
}

// implement this trait so that `MakeThriftConnectionFromAddrs` can create it
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

// implement this trait so that `ThriftConnectionManager` can manage it
impl<Ip: TInputProtocol, Op: TOutputProtocol> ThriftConnection for MyThriftClient<Ip, Op> {
    type Error = thrift::Error;
    fn is_valid(&mut self) -> Result<(), Self::Error> {
       Ok(())
    }
    fn has_broken(&mut self) -> bool {
       false
   }
}

// the actual connection type
type Client = MyThriftClient<
    TCompactInputProtocol<TFramedReadTransport<ReadHalf<TTcpChannel>>>,
    TCompactOutputProtocol<TFramedWriteTransport<WriteHalf<TTcpChannel>>>,
>;

// this works because we implemented FromProtocol for the client
// AND
// because the `Protocol` and `Transport` types used here satisfy the contraints
let manager = ThriftConnectionManager::new(
                    MakeThriftConnectionFromAddrs::<Client, _>::new("localhost:9090")
                );

// this works because we implemented ThriftConnection for the client
let pool = r2d2::Pool::builder().build(manager)?;

// this also works after enabling the `impl-bb8` feature
let pool = bb8::Pool::builder().build(manager).await?;

// the pool can be used just like in r2d2/bb8 documentation
let mut client = pool.get()?;
```


## Examples

- [hbase-thrift](https://github.com/midnightexigent/hbase-thrift-rs):  the project from which this
  library was extracted. implements Connection Pools for the client generated from the
  [HBase Thrift Spec](https://github.com/apache/hbase/tree/master/hbase-thrift/src/main/resources/org/apache/hadoop/hbase/thrift)
- [thrift-pool-tutorial](https://github.com/midnightexigent/thrift-pool-tutorial-rs): implements
  Connection Pools for the client used in the official
  [thrift tutorial](https://github.com/apache/thrift/tree/master/tutorial)
