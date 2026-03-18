import type { Dispatch, FormEvent, SetStateAction } from "react";

import { Alert, Button, Card, CardContent, Disclosure } from "@heroui/react";

import { AppForm, TextInputField } from "../ui";
import { DEFAULT_LOGIN_FORM, type LoginForm } from "../../stateTypes";

type ConsoleAuthScreenProps = {
  error: string | null;
  loginBusy: boolean;
  loginForm: LoginForm;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void | Promise<void>;
  setLoginForm: Dispatch<SetStateAction<LoginForm>>;
};

export function ConsoleAuthScreen({
  error,
  loginBusy,
  loginForm,
  onSubmit,
  setLoginForm,
}: ConsoleAuthScreenProps) {
  return (
    <div className="console-root console-root--auth flex min-h-screen items-center justify-center">
      <Card className="workspace-card w-full max-w-2xl" variant="secondary">
        <CardContent className="grid gap-6 px-6 py-7 sm:px-7">
          <div className="grid gap-2">
            <p className="console-label">Palyra console</p>
            <h1 className="text-2xl font-semibold tracking-tight">Operator Dashboard</h1>
            <p className="console-copy">
              Desktop handoff remains the shortest path, but direct browser sign-in still uses the
              same admin token, cookie session, and CSRF guardrails.
            </p>
          </div>

          <AppForm className="space-y-5" onSubmit={(event) => void onSubmit(event)}>
            <TextInputField
              autoComplete="off"
              disabled={loginBusy}
              label="Admin token"
              required
              type="password"
              value={loginForm.adminToken}
              onChange={(value) => setLoginForm((previous) => ({ ...previous, adminToken: value }))}
            />

            <Disclosure>
              <Disclosure.Trigger className="flex w-full items-center justify-between rounded-lg border border-border bg-surface px-4 py-3 text-left text-sm">
                <Disclosure.Heading className="font-medium">
                  Advanced session identity
                </Disclosure.Heading>
                <Disclosure.Indicator />
              </Disclosure.Trigger>
              <Disclosure.Content>
                <Disclosure.Body>
                  <div className="mt-4 grid gap-4 md:grid-cols-2">
                    <TextInputField
                      disabled={loginBusy}
                      label="Operator principal"
                      required
                      value={loginForm.principal}
                      onChange={(value) =>
                        setLoginForm((previous) => ({ ...previous, principal: value }))
                      }
                    />
                    <TextInputField
                      disabled={loginBusy}
                      label="Device label"
                      required
                      value={loginForm.deviceId}
                      onChange={(value) =>
                        setLoginForm((previous) => ({ ...previous, deviceId: value }))
                      }
                    />
                    <div className="md:col-span-2">
                      <TextInputField
                        disabled={loginBusy}
                        label="Channel label"
                        placeholder="Optional"
                        value={loginForm.channel}
                        onChange={(value) =>
                          setLoginForm((previous) => ({ ...previous, channel: value }))
                        }
                      />
                    </div>
                  </div>
                </Disclosure.Body>
              </Disclosure.Content>
            </Disclosure>

            <Alert status="default">
              <Alert.Content className="flex flex-wrap items-center justify-between gap-3">
                <div className="grid gap-1">
                  <Alert.Title>Browser sign-in path</Alert.Title>
                  <Alert.Description>
                    Manual browser sign-in still keeps the existing session cookie and CSRF
                    guardrails in place. Open from desktop for the shortest local path on a single
                    machine.
                  </Alert.Description>
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  onPress={() => setLoginForm(DEFAULT_LOGIN_FORM)}
                  isDisabled={loginBusy}
                >
                  Restore defaults
                </Button>
              </Alert.Content>
            </Alert>

            <div className="flex flex-wrap items-center justify-end gap-3 pt-1">
              <Button type="submit" variant="primary" isDisabled={loginBusy}>
                {loginBusy ? "Signing in..." : "Sign in"}
              </Button>
            </div>
          </AppForm>

          {error !== null ? (
            <Alert status="danger">
              <Alert.Content>
                <Alert.Title>Sign-in failed</Alert.Title>
                <Alert.Description>{error}</Alert.Description>
              </Alert.Content>
            </Alert>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}
