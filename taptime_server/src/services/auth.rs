use std::sync::Arc;

use taptime_schema::{
  User,
  services::{AuthResponse, LoginRequest, RegisterUserRequest, auth_service_server::AuthService},
};
use tonic::{Request, Response, Status};

pub struct AuthServiceImpl {}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
  async fn register_user(
    self: Arc<Self>,
    request: Request<RegisterUserRequest>,
  ) -> Result<Response<AuthResponse>, Status> {
    todo!()
  }

  async fn login(
    self: Arc<Self>,
    request: Request<LoginRequest>,
  ) -> Result<Response<AuthResponse>, Status> {
    todo!()
  }

  async fn get_user(self: Arc<Self>, request: Request<()>) -> Result<Response<User>, Status> {
    todo!()
  }
}
