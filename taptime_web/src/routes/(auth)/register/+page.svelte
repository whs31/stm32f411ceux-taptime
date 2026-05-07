<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Password from "$lib/components/ui/password";
  import type { ZxcvbnResult } from "@zxcvbn-ts/core";

  const SCORE_NAMING = ["Poor", "Weak", "Average", "Strong", "Secure"];

  let strength = $state<ZxcvbnResult>();
</script>

<Card.Root class="w-full max-w-sm">
  <Card.Header class="items-center text-center">
    <Card.Title class="text-xl">Create an account</Card.Title>
    <Card.Description>Fill in the details below to get started</Card.Description
    >
  </Card.Header>
  <Card.Content class="flex flex-col gap-4">
    <div class="flex flex-col gap-2">
      <Label for="name">Name</Label>
      <Input id="name" type="text" placeholder="John Doe" />
    </div>
    <div class="flex flex-col gap-2">
      <Label for="email">Email</Label>
      <Input id="email" type="email" placeholder="you@example.com" />
    </div>
    <div class="flex flex-col gap-2">
      <Label for="password">Password</Label>
      <Password.Root minScore={2}>
        <Password.Input placeholder="••••••••••••••••">
          <Password.ToggleVisibility />
        </Password.Input>
        <div class="flex flex-col gap-1">
          <Password.Strength bind:strength />
          <span class="text-muted-foreground text-sm">
            {SCORE_NAMING[strength?.score ?? 0]}
          </span>
        </div>
      </Password.Root>
    </div>
    <div class="flex flex-col gap-2">
      <Label for="confirm">Confirm password</Label>
      <Password.Root>
        <Password.Input value="" placeholder="••••••••••••••••">
          <Password.ToggleVisibility />
        </Password.Input>
      </Password.Root>
    </div>
    <Button class="w-full">Create account</Button>
  </Card.Content>
  <Card.Footer class="justify-center text-sm text-muted-foreground">
    Already have an account?&nbsp;<a
      href="/login"
      class="underline underline-offset-4">Sign in</a
    >
  </Card.Footer>
</Card.Root>
