<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Stepper from "$lib/components/ui/stepper";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import {
    CircleAlert,
    UserIcon,
    Calendar1Icon,
    SlidersVerticalIcon,
  } from "@lucide/svelte";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Password from "$lib/components/ui/password";
  import { cn } from "$lib/utils";
  import type { ZxcvbnResult } from "@zxcvbn-ts/core";
  import { createPromiseClient } from "@connectrpc/connect";
  import { AuthService } from "@taptime/proto/taptime/services/auth_connect.js";
  import { RegisterUserRequest } from "@taptime/proto/taptime/services/auth_pb.js";
  import { User, User_Settings } from "@taptime/proto/taptime/user_pb.js";
  import { Weekday } from "@taptime/proto/taptime/weekday_pb.js";
  import { Tz } from "@taptime/proto/taptime/tz_pb.js";
  import { Duration } from "@bufbuild/protobuf";
  import { transport } from "$lib/grpc";
  import { goto } from "$app/navigation";

  const SCORE_NAMING = ["Poor", "Weak", "Average", "Strong", "Secure"];

  const WEEKDAYS: { label: string; value: Weekday }[] = [
    { label: "Mon", value: Weekday.MONDAY },
    { label: "Tue", value: Weekday.TUESDAY },
    { label: "Wed", value: Weekday.WEDNESDAY },
    { label: "Thu", value: Weekday.THURSDAY },
    { label: "Fri", value: Weekday.FRIDAY },
    { label: "Sat", value: Weekday.SATURDAY },
    { label: "Sun", value: Weekday.SUNDAY },
  ];

  const TIMEZONES: string[] = Intl.supportedValuesOf("timeZone");
  const LOCAL_TZ = Intl.DateTimeFormat().resolvedOptions().timeZone;

  const client = createPromiseClient(AuthService, transport);

  let step = $state(1);

  // Step 1: Credentials
  let name = $state("");
  let email = $state("");
  let password = $state("");
  let confirmPassword = $state("");
  let strength = $state<ZxcvbnResult>();

  // Step 2: Schedule
  let timezone = $state(LOCAL_TZ);
  let workHours = $state(8);
  let workMinutes = $state(0);
  let lunchMinutes = $state(60);
  let weekends = $state<Weekday[]>([Weekday.SATURDAY, Weekday.SUNDAY]);
  let remoteDays = $state<Weekday[]>([]);

  // Step 3: Profile
  let organization = $state("");

  let submitting = $state(false);
  let error = $state<string | undefined>();

  const emailValid = $derived(/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email));
  const passwordsMatch = $derived(
    password === confirmPassword && confirmPassword.length > 0,
  );
  const step1Valid = $derived(
    name.trim().length > 0 &&
      emailValid &&
      password.length > 0 &&
      passwordsMatch &&
      (strength?.score ?? 0) >= 2,
  );
  const step2Valid = $derived(
    timezone.length > 0 && (workHours > 0 || workMinutes > 0),
  );
  const nextDisabled = $derived(
    (step === 1 && !step1Valid) || (step === 2 && !step2Valid),
  );

  function toggleDay(days: Weekday[], day: Weekday): Weekday[] {
    return days.includes(day) ? days.filter((d) => d !== day) : [...days, day];
  }

  async function submit() {
    submitting = true;
    error = undefined;
    try {
      const workSecs = BigInt(workHours * 3600 + workMinutes * 60);
      const lunchSecs = BigInt(lunchMinutes * 60);

      const response = await client.registerUser(
        new RegisterUserRequest({
          password,
          user: new User({
            name: name.trim(),
            email: email.trim(),
            organization: organization.trim() || undefined,
            timeZone: new Tz({ timeZone: timezone }),
            settings: new User_Settings({
              requiredWorkHours: new Duration({ seconds: workSecs }),
              lunchBreakDuration: new Duration({ seconds: lunchSecs }),
              weekends,
              remoteDays,
            }),
          }),
        }),
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

<Card.Root class="w-full max-w-xl">
  <Card.Header class="items-center text-center">
    <Card.Title class="text-xl">Create an account</Card.Title>
    <Card.Description>Fill in the details below to get started</Card.Description
    >
  </Card.Header>
  <Card.Content class="flex flex-col gap-6 px-4">
    <Stepper.Root bind:step>
      <Stepper.Nav orientation="horizontal" class="justify-between">
        <Stepper.Item id="account">
          <Stepper.Trigger class="items-center">
            <Stepper.Indicator>
              <UserIcon />
            </Stepper.Indicator>
            <Stepper.Description>Account</Stepper.Description>
          </Stepper.Trigger>
          <Stepper.Separator />
        </Stepper.Item>
        <Stepper.Item id="schedule">
          <Stepper.Trigger class="items-center" disabled={!step1Valid}>
            <Stepper.Indicator>
              <Calendar1Icon />
            </Stepper.Indicator>
            <Stepper.Description>Schedule</Stepper.Description>
          </Stepper.Trigger>
          <Stepper.Separator />
        </Stepper.Item>
        <Stepper.Item id="profile">
          <Stepper.Trigger class="items-center" disabled={!step1Valid || !step2Valid}>
            <Stepper.Indicator>
              <SlidersVerticalIcon />
            </Stepper.Indicator>
            <Stepper.Description>Profile</Stepper.Description>
          </Stepper.Trigger>
        </Stepper.Item>
      </Stepper.Nav>

      <div class="min-h-105">
        <!-- Step 1: Credentials -->
        {#if step === 1}
          <div class="flex flex-col gap-4 pt-2">
            <div class="flex flex-col gap-2">
              <Label for="name">Name</Label>
              <Input
                id="name"
                type="text"
                placeholder="John Doe"
                bind:value={name}
              />
            </div>
            <div class="flex flex-col gap-2">
              <Label for="email">Email</Label>
              <Input
                id="email"
                type="email"
                placeholder="you@example.com"
                bind:value={email}
                aria-invalid={email.length > 0 && !emailValid}
              />
              {#if email.length > 0 && !emailValid}
                <span class="text-destructive text-xs"
                  >Enter a valid email address</span
                >
              {/if}
            </div>
            <div class="flex flex-col gap-2">
              <Label for="password">Password</Label>
              <Password.Root minScore={2}>
                <Password.Input
                  bind:value={password}
                  placeholder="••••••••••••••••"
                >
                  <Password.ToggleVisibility />
                </Password.Input>
                <div class="flex flex-col gap-1">
                  <Password.Strength bind:strength />
                  <span class="text-muted-foreground text-xs">
                    {SCORE_NAMING[strength?.score ?? 0]}
                  </span>
                </div>
              </Password.Root>
            </div>
            <div class="flex flex-col gap-2">
              <Label for="confirm">Confirm password</Label>
              <Password.Root>
                <Password.Input
                  bind:value={confirmPassword}
                  placeholder="••••••••••••••••"
                  aria-invalid={confirmPassword.length > 0 && !passwordsMatch}
                >
                  <Password.ToggleVisibility />
                </Password.Input>
              </Password.Root>
              {#if confirmPassword.length > 0 && !passwordsMatch}
                <span class="text-destructive text-xs"
                  >Passwords do not match</span
                >
              {/if}
            </div>
          </div>

          <!-- Step 2: Schedule -->
        {:else if step === 2}
          <div class="flex flex-col gap-4 pt-2">
            <div class="flex flex-col gap-2">
              <Label for="timezone">Time zone</Label>
              <select
                id="timezone"
                bind:value={timezone}
                class="border-input bg-background ring-offset-background focus-visible:ring-ring flex h-10 w-full rounded-md border px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {#each TIMEZONES as tz}
                  <option value={tz}>{tz}</option>
                {/each}
              </select>
            </div>
            <div class="flex flex-col gap-2">
              <Label>Required work hours per day</Label>
              <div class="flex items-center gap-2">
                <Input
                  type="number"
                  min="0"
                  max="23"
                  bind:value={workHours}
                  class="w-20 text-center"
                />
                <span class="text-muted-foreground text-sm">h</span>
                <Input
                  type="number"
                  min="0"
                  max="59"
                  step="5"
                  bind:value={workMinutes}
                  class="w-20 text-center"
                />
                <span class="text-muted-foreground text-sm">min</span>
              </div>
            </div>
            <div class="flex flex-col gap-2">
              <Label>Lunch break</Label>
              <div class="flex items-center gap-2">
                <Input
                  type="number"
                  min="0"
                  max="240"
                  step="5"
                  bind:value={lunchMinutes}
                  class="w-24 text-center"
                />
                <span class="text-muted-foreground text-sm">minutes</span>
              </div>
            </div>
            <div class="flex flex-col gap-2">
              <Label>Weekend days</Label>
              <div class="flex gap-1.5">
                {#each WEEKDAYS as day}
                  <button
                    type="button"
                    onclick={() => (weekends = toggleDay(weekends, day.value))}
                    class={cn(
                      "h-9 w-10 rounded-md text-xs font-medium transition-colors",
                      weekends.includes(day.value)
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted text-muted-foreground hover:bg-muted/70",
                    )}
                  >
                    {day.label}
                  </button>
                {/each}
              </div>
            </div>
            <div class="flex flex-col gap-2">
              <Label>Remote days</Label>
              <div class="flex gap-1.5">
                {#each WEEKDAYS as day}
                  <button
                    type="button"
                    onclick={() =>
                      (remoteDays = toggleDay(remoteDays, day.value))}
                    class={cn(
                      "h-9 w-10 rounded-md text-xs font-medium transition-colors",
                      remoteDays.includes(day.value)
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted text-muted-foreground hover:bg-muted/70",
                    )}
                  >
                    {day.label}
                  </button>
                {/each}
              </div>
            </div>
          </div>

          <!-- Step 3: Profile -->
        {:else}
          <div class="flex flex-col gap-4 pt-2">
            <div class="flex flex-col gap-2">
              <Label for="org">
                Organization <span class="text-muted-foreground font-normal"
                  >(optional)</span
                >
              </Label>
              <Input
                id="org"
                type="text"
                placeholder="Acme Corp"
                bind:value={organization}
              />
            </div>
            <div class="rounded-md border p-4 flex flex-col gap-1.5 text-sm">
              <p class="font-medium mb-1">Summary</p>
              <p class="text-muted-foreground">
                <span class="text-foreground">{name}</span> · {email}
              </p>
              <p class="text-muted-foreground">{timezone}</p>
              <p class="text-muted-foreground">
                {workHours}h {workMinutes}min work · {lunchMinutes}min lunch
              </p>
              <p class="text-muted-foreground">
                Weekends: {weekends.length > 0
                  ? WEEKDAYS.filter((d) => weekends.includes(d.value))
                      .map((d) => d.label)
                      .join(", ")
                  : "none"}
              </p>
              {#if remoteDays.length > 0}
                <p class="text-muted-foreground">
                  Remote: {WEEKDAYS.filter((d) => remoteDays.includes(d.value))
                    .map((d) => d.label)
                    .join(", ")}
                </p>
              {/if}
            </div>
          </div>
        {/if}
      </div>

      {#if error}
        <Alert.Root variant="destructive">
          <CircleAlert />
          <Alert.Title>Registration failed</Alert.Title>
          <Alert.Description>{error}</Alert.Description>
        </Alert.Root>
      {/if}

      <!-- Navigation -->
      <div class="flex justify-between pt-2">
        <Stepper.Previous>Previous</Stepper.Previous>
        {#if step < 3}
          <Stepper.Next disabled={nextDisabled}>Next</Stepper.Next>
        {:else}
          <Button onclick={submit} disabled={submitting}>
            {submitting ? "Creating account…" : "Create account"}
          </Button>
        {/if}
      </div>
    </Stepper.Root>
  </Card.Content>
  <Card.Footer class="justify-center text-sm text-muted-foreground">
    Already have an account?&nbsp;<a
      href="/login"
      class="underline underline-offset-4">Sign in</a
    >
  </Card.Footer>
</Card.Root>
