fn main() -> Result<(), Box<dyn std::error::Error>> {
  #[cfg(feature = "vendored-protobuf")]
  unsafe {
    std::env::set_var("PROTOC", protobuf_src::protoc());
  }

  buffa_build::Config::new()
    .include_file("_includes.rs")
    .files(&["schema/taptime/uuid.proto"])
    .includes(&["schema"])
    .compile()?;
  Ok(())
}
