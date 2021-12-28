# Thrift Pool 

This library provides the `ThriftConnectionManager` type which makes it easy to implement Connection Pools for any [thrift client](https://docs.rs/thrift/latest/thrift/)

This library was extracted from [hbase-thrift](https://crates.io/crates/hbase-thrift)

## Usage

- Implement the `FromIoProtocol`, `IsValid` and the `HasBroken` traits for your `thrift` client
- That's it ! Now you are able to create a `ThriftConnectionManager` with your `thrift`
- `ThriftConnectionManager` can create new instances of the client through [bb8](https://github.com/djc/bb8/) or [r2d2](https://github.com/sfackler/r2d2)

## Examples

- See [hbase-thrift-rs](https://github.com/midnightexigent/hbase-thrift-rs)
- See [thrift-pool-tutorial](https://github.com/midnightexigent/thrift-pool-tutorial) which showcases how to implement a connection pool around the client used in the [official tutorial](https://github.com/apache/thrift/tree/master/tutorial) (`CalculatorSyncClient`) using the capabilities of this library


