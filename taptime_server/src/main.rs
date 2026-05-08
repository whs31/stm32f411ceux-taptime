mod args;
mod error;
mod jwt;
pub mod interceptors;
pub mod services;

use std::path::Path;

pub use self::{
  args::Args,
  error::{Error, Result},
};

fn init_tracing(
  level: tracing::Level,
  log_dir: &Path,
) -> tracing_appender::non_blocking::WorkerGuard {
  use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

  let file_appender = tracing_appender::rolling::daily(log_dir, "taptime_server.log");
  let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

  tracing_subscriber::registry()
    .with(EnvFilter::new(level.to_string()))
    .with(fmt::layer())
    .with(fmt::layer().with_writer(file_writer).with_ansi(false))
    .init();

  guard
}

#[tokio::main]
async fn main() -> Result<()> {
  let directories = directories::ProjectDirs::from("com", "whs31", "TapTime")
    .expect("failed to get project directories");

  let args = args::parse();
  let _guard = init_tracing(args.log_level, &directories.data_dir().join("logs"));

  let pool = sqlx::PgPool::connect(&args.database_url).await?;
  sqlx::migrate!("./migrations").run(&pool).await?;

  let auth_service = services::AuthServiceImpl::new(pool.clone(), args.jwt_secret.clone());
  let store_service = services::StoreServiceImpl::new(pool);

  let auth_svc =
    taptime_schema::services::auth_service_server::AuthServiceServer::new(auth_service);
  let store_svc =
    taptime_schema::services::store_service_server::StoreServiceServer::with_interceptor(
      store_service,
      interceptors::AuthInterceptor::new(args.jwt_secret),
    );

  let cors = tower_http::cors::CorsLayer::new()
    .allow_origin(tower_http::cors::Any)
    .allow_headers([
      http::header::AUTHORIZATION,
      http::header::CONTENT_TYPE,
      http::header::HeaderName::from_static("x-grpc-web"),
      http::header::HeaderName::from_static("x-user-agent"),
      http::header::HeaderName::from_static("grpc-timeout"),
    ])
    .allow_methods(tower_http::cors::Any);

  tracing::info!("server listening on {}", args.address);
  tonic::transport::Server::builder()
    .accept_http1(true)
    .layer(cors)
    .layer(tonic_web::GrpcWebLayer::new())
    .add_service(auth_svc)
    .add_service(store_svc)
    .serve(args.address)
    .await?;

  Ok(())
}
