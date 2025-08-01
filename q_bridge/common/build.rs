fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set the PROTOC environment variable to the path of the protoc binary.
    // This will compile protoc from source if it's not found on the system.
    std::env::set_var("PROTOC", protobuf_src::protoc());

    tonic_build::configure()
        // Add serde serialization and deserialization to all generated types
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile(&["proto/gateway.proto"], &["proto"])?;
    Ok(())
}
