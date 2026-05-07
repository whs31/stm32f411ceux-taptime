<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Password from "$lib/components/ui/password";
</script>

<Card.Root class="w-full max-w-sm">
  <Card.Header class="items-center text-center">
    <Card.Title class="text-xl">Sign in to TapTime</Card.Title>
    <Card.Description>Enter your credentials to continue</Card.Description>
  </Card.Header>
  <Card.Content class="flex flex-col gap-4">
    <div class="flex flex-col gap-2">
      <Label for="email">Email</Label>
      <Input id="email" type="email" placeholder="you@example.com" />
    </div>
    <div class="flex flex-col gap-2">
      <Label for="password">Password</Label>
      <Password.Root>
        <Password.Input value="" placeholder="••••••••••••••••">
          <Password.ToggleVisibility />
        </Password.Input>
      </Password.Root>
    </div>
    <Button class="w-full">Sign in</Button>
  </Card.Content>
  <Card.Footer class="justify-center text-sm text-muted-foreground">
    Don't have an account?&nbsp;<a
      href="/register"
      class="underline underline-offset-4">Register</a
    >
  </Card.Footer>
</Card.Root>
