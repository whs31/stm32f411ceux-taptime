import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { PUBLIC_API_URL } from "$env/static/public";

export const transport = createGrpcWebTransport({
  baseUrl: PUBLIC_API_URL,
});
