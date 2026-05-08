import { createPromiseClient } from "@connectrpc/connect";
import { AuthService as AuthServiceClient } from "@taptime/proto/taptime/services/auth_connect.js";
import type { User } from "@taptime/proto/taptime/user_pb.js";
import {
  LoginRequest,
  AuthResponse,
  RegisterUserRequest,
} from "@taptime/proto/taptime/services/auth_pb.js";
import { transport } from "$lib/grpc";

export class AuthService {
  private static client = createPromiseClient(AuthServiceClient, transport);

  static async getUserWithToken(jwt: string): Promise<User> {
    return this.client.getUser(
      {},
      { headers: { authorization: `Bearer ${jwt}` } },
    );
  }

  static async getUser(): Promise<User> {
    const jwt = this.getStoredJwt();
    if (!jwt) throw new Error("no jwt");
    return this.getUserWithToken(jwt);
  }

  static async login(email: string, password: string): Promise<User> {
    const response = await this.client.login(
      new LoginRequest({
        email: email.trim(),
        password: password,
      }),
    );
    return this._processResponse(response);
  }

  static async registerUser(user: User, password: string): Promise<User> {
    const response = await this.client.registerUser(
      new RegisterUserRequest({
        user: user,
        password: password,
      }),
    );
    return this._processResponse(response);
  }

  static _processResponse(response: AuthResponse): User {
    if (!response.jwt || response.jwt.length === 0)
      throw new Error("Response has null JWT");
    if (!response.user)
      throw new Error("Response does not contain a valid user");
    this.storeJwt(response.jwt);
    return response.user;
  }

  static getStoredJwt(): string | null {
    if (typeof localStorage === "undefined") return null;
    return localStorage.getItem("jwt");
  }

  static clearStoredJwt(): void {
    if (typeof localStorage !== "undefined") localStorage.removeItem("jwt");
  }

  static storeJwt(jwt: string): void {
    if (typeof localStorage !== "undefined") localStorage.setItem("jwt", jwt);
  }
}
