#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Transport(#[from] tonic::transport::Error),

  #[error(transparent)]
  Io(#[from] std::io::Error),

  #[error(transparent)]
  Database(#[from] sqlx::Error),

  #[error(transparent)]
  Migrate(#[from] sqlx::migrate::MigrateError),
}

pub type Result<T> = std::result::Result<T, Error>;
