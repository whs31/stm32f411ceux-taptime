<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Password from "$lib/components/ui/password";
  import { CircleAlert } from "@lucide/svelte";
  import { createPromiseClient } from "@connectrpc/connect";
  import { AuthService } from "@taptime/proto/taptime/services/auth_connect.js";
  import { LoginRequest } from "@taptime/proto/taptime/services/auth_pb.js";
  import { transport } from "$lib/grpc";
  import { goto } from "$app/navigation";

  const client = createPromiseClient(AuthService, transport);

  let email = $state("");
  let password = $state("");
  let submitting = $state(false);
  let error = $state<string | undefined>();

  async function login() {
    submitting = true;
    error = undefined;
    try {
      const response = await client.login(
        new LoginRequest({ email: email.trim(), password }),
      );
      localStorage.setItem("jwt", response.jwt);
      await goto("/");
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      submitting = false;
    }
  }
</script>

<Card.Root class="w-full max-w-sm">
  <Card.Header class="items-center text-center">
    <Card.Title class="text-xl">Sign in to TapTime</Card.Title>
    <Card.Description>Enter your credentials to continue</Card.Description>
  </Card.Header>
  <Card.Content class="flex flex-col gap-4">
    {#if error}
      <Alert.Root variant="destructive">
        <CircleAlert />
        <Alert.Title>Sign in failed</Alert.Title>
        <Alert.Description
          >Either your email or password is incorrect.</Alert.Description
        >
      </Alert.Root>
    {/if}
    <div class="flex flex-col gap-2">
      <Label for="email">Email</Label>
      <Input
        id="email"
        type="email"
        placeholder="you@example.com"
        bind:value={email}
        onkeydown={(e) => e.key === "Enter" && login()}
      />
    </div>
    <div class="flex flex-col gap-2">
      <Label for="password">Password</Label>
      <Password.Root>
        <Password.Input
          bind:value={password}
          placeholder="••••••••••••••••"
          onkeydown={(e) => e.key === "Enter" && login()}
        >
          <Password.ToggleVisibility />
        </Password.Input>
      </Password.Root>
    </div>
    <Button class="w-full" onclick={login} disabled={submitting}>
      {submitting ? "Signing in…" : "Sign in"}
    </Button>
  </Card.Content>
  <Card.Footer class="justify-center text-sm text-muted-foreground">
    Don't have an account?&nbsp;<a
      href="/register"
      class="underline underline-offset-4">Register</a
    >
  </Card.Footer>
</Card.Root>
