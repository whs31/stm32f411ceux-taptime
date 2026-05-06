const SERDE_DERIVES: &str = "serde::Serialize, serde::Deserialize";
const SERDE_ATTRIBUTES: &str = "#[serde(rename_all = \"camelCase\")]";
const TYPESCRIPT_DERIVES: &str = "ts_rs::TS";
const TYPESCRIPT_ATTRIBUTES: &str = "#[ts(export)]";

fn main() -> Result<(), Box<dyn std::error::Error>> {
  #[cfg(feature = "vendored-protobuf")]
  unsafe {
    std::env::set_var("PROTOC", protobuf_src::protoc());
  }

  let mut b = tonic_prost_build::configure().include_file("_includes.rs");
  let attributes = if cfg!(feature = "serde") {
    if cfg!(feature = "typescript") {
      Some(format!(
        "#[derive({SERDE_DERIVES}, {TYPESCRIPT_DERIVES})] {SERDE_ATTRIBUTES} {TYPESCRIPT_ATTRIBUTES}"
      ))
    } else {
      Some(format!("#[derive({SERDE_DERIVES})] {SERDE_ATTRIBUTES}"))
    }
  } else if cfg!(feature = "typescript") {
    Some(format!(
      "#[derive({TYPESCRIPT_DERIVES})] {TYPESCRIPT_ATTRIBUTES}"
    ))
  } else {
    None
  };
  if let Some(attributes) = attributes {
    b = b
      .message_attribute(".", &attributes)
      .enum_attribute(".", &attributes);
  }
  if cfg!(feature = "legacy") {
    b = b.protoc_arg("--experimental_allow_proto3_optional");
  }
  b = b
    .build_client(cfg!(feature = "client"))
    .build_server(cfg!(feature = "server"))
    .use_arc_self(true);
  if cfg!(feature = "wkt") {
    b = b.compile_well_known_types(true)
  }
  b.compile_protos(
    &[
      "schema/taptime/uuid.proto",
      // #[cfg(feature = "grpc")]
      // "share/proto/quasar/relay/services/license.proto",
    ],
    &["schema"],
  )?;
  Ok(())
}