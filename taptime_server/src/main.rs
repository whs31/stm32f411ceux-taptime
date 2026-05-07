mod args;
mod error;
pub mod interceptors;
pub mod services;

pub use self::{
  args::Args,
  error::{Error, Result},
};

#[tokio::main]
async fn main() -> Result<()> {
  let args = args::parse();

  let auth_service = services::AuthServiceImpl {};
  let svc = taptime_schema::services::auth_service_server::AuthServiceServer::new(auth_service);

  tonic::transport::Server::builder()
    .accept_http1(true)
    .layer(tonic_web::GrpcWebLayer::new())
    .add_service(svc)
    .serve(args.address)
    .await?;

  Ok(())
}
