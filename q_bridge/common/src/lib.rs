// This file exports the generated protobuf code for other crates in the workspace to use.

// The `include_proto!` macro will generate the Rust code from the .proto file
// and include it in a module named after the protobuf package.
pub mod gateway {
    tonic::include_proto!("gateway");
}
