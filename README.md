# Thrift Pool

This library provides a simple way implement [`bb8`](https://crates.io/crates/bb8)
and/or [`r2d2`](https://crates.io/crates/r2d2) Connection Pools
for any [`TThriftClient`](https://docs.rs/thrift/0.15.0/thrift/trait.TThriftClient.html)

<br>

[Documentation](https://docs.rs/thrift-pool/1.0.2/thrift_pool/)

<br>

## Example

```rust
use thrift::protocol::{TCompactInputProtocol, TCompactOutputProtocol, TInputProtocol, TOutputProtocol};
use thrift::transport::{
    ReadHalf, TFramedReadTransport, TFramedWriteTransport, TTcpChannel, WriteHalf,
};
use thrift_pool::{MakeThriftConnectionFromAddrs, ThriftConnectionManager, ThriftConnection, FromProtocol};
use r2d2::Pool;

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

type Client = MyThriftClient<
    TCompactInputProtocol<TFramedReadTransport<ReadHalf<TTcpChannel>>>,
    TCompactOutputProtocol<TFramedWriteTransport<WriteHalf<TTcpChannel>>>,
>;

let manager = ThriftConnectionManager::new(
                    MakeThriftConnectionFromAddrs::<Client, _>::new("localhost:9090")
                );
let pool = Pool::builder().build(manager)?;
let mut client = pool.get()?;
```


## Other examples

- [hbase-thrift](https://github.com/midnightexigent/hbase-thrift-rs) -- the project from which this
  library was extracted. implements Connection Pools for the client generated from the
  [HBase Thrift Spec](https://github.com/apache/hbase/tree/master/hbase-thrift/src/main/resources/org/apache/hadoop/hbase/thrift)
- [thrift-pool-tutorial](https://github.com/midnightexigent/thrift-pool-tutorial-rs) -- implements
  Connection Pools for the client used in the official
  [thrift tutorial](https://github.com/apache/thrift/tree/master/tutorial)
