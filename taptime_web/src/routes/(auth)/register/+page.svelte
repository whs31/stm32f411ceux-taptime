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
  import type { ZxcvbnResult } from "@zxcvbn-ts/core";
  import { AuthService } from "$lib/services";
  import { User, User_Settings } from "@taptime/proto/taptime/user_pb.js";
  import { Tz } from "@taptime/proto/taptime/tz_pb.js";
  import { Duration } from "@bufbuild/protobuf";
  import { goto } from "$app/navigation";
  import {
    TimeZoneSelect,
    WorkHoursInput,
    LunchBreakSelect,
    WeekdaySelect,
  } from "$lib/blocks/components";
  import { Weekday } from "@taptime/proto/taptime/weekday_pb.js";

  const SCORE_NAMING = ["Poor", "Weak", "Average", "Strong", "Secure"];
  const LOCAL_TZ = Intl.DateTimeFormat().resolvedOptions().timeZone;

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
  let lunchMinutes = $state(30);
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

  const DAY_LABELS: Record<Weekday, string> = {
    [Weekday.MONDAY]: "Mon",
    [Weekday.TUESDAY]: "Tue",
    [Weekday.WEDNESDAY]: "Wed",
    [Weekday.THURSDAY]: "Thu",
    [Weekday.FRIDAY]: "Fri",
    [Weekday.SATURDAY]: "Sat",
    [Weekday.SUNDAY]: "Sun",
    [Weekday.UNSPECIFIED]: "",
  };

  async function submit() {
    submitting = true;
    error = undefined;
    try {
      const workSecs = BigInt(workHours * 3600 + workMinutes * 60);
      const lunchSecs = BigInt(lunchMinutes * 60);

      await AuthService.registerUser(
        new User({
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
        password,
      );
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
          <Stepper.Trigger class="w-full items-center">
            <Stepper.Indicator>
              <UserIcon />
            </Stepper.Indicator>
            <Stepper.Description>Account</Stepper.Description>
          </Stepper.Trigger>
          <Stepper.Separator class="left-1/2" />
        </Stepper.Item>
        <Stepper.Item id="schedule">
          <Stepper.Trigger class="w-full items-center" disabled={!step1Valid}>
            <Stepper.Indicator>
              <Calendar1Icon />
            </Stepper.Indicator>
            <Stepper.Description>Schedule</Stepper.Description>
          </Stepper.Trigger>
          <Stepper.Separator class="left-1/2" />
        </Stepper.Item>
        <Stepper.Item id="profile" class="flex-1">
          <Stepper.Trigger
            class="w-full items-center"
            disabled={!step1Valid || !step2Valid}
          >
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
                <Password.Input bind:value={password}>
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
              <TimeZoneSelect bind:value={timezone} />
            </div>
            <div class="grid grid-cols-2 gap-4">
              <div class="flex flex-col gap-2">
                <Label>Required work hours per day</Label>
                <WorkHoursInput
                  bind:hours={workHours}
                  bind:minutes={workMinutes}
                />
              </div>
              <div class="flex flex-col gap-2">
                <Label>Lunch break</Label>
                <LunchBreakSelect bind:value={lunchMinutes} />
              </div>
            </div>
            <div class="flex flex-col gap-2">
              <Label>Weekend days</Label>
              <WeekdaySelect bind:value={weekends} />
            </div>
            <div class="flex flex-col gap-2">
              <Label>Remote days</Label>
              <WeekdaySelect bind:value={remoteDays} />
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
                  ? weekends.map((d) => DAY_LABELS[d]).join(", ")
                  : "none"}
              </p>
              {#if remoteDays.length > 0}
                <p class="text-muted-foreground">
                  Remote: {remoteDays.map((d) => DAY_LABELS[d]).join(", ")}
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
