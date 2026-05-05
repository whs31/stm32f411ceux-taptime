fn main() -> Result<(), Box<dyn std::error::Error>> {
  #[cfg(feature = "vendored-protobuf")]
  unsafe {
    std::env::set_var("PROTOC", protobuf_src::protoc());
  }

  let mut b = prost_build::Config::new();
  b.include_file("_includes.rs");
  b.compile_protos(&["schema/taptime/uuid.proto"], &["schema"])?;
  Ok(())
}
