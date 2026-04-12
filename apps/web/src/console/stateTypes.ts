export type CronScheduleType = "cron" | "every" | "at";

export type LoginForm = {
  adminToken: string;
  principal: string;
  deviceId: string;
  channel: string;
};

export type CronForm = {
  name: string;
  prompt: string;
  scheduleType: CronScheduleType;
  cronExpression: string;
  everyIntervalMs: string;
  atTimestampRfc3339: string;
  enabled: boolean;
  channel: string;
};

export const DEFAULT_LOGIN_DEVICE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

export const DEFAULT_LOGIN_FORM: LoginForm = {
  adminToken: "",
  principal: "admin:local-browser",
  deviceId: DEFAULT_LOGIN_DEVICE_ID,
  channel: "",
};

export const DEFAULT_CRON_FORM: CronForm = {
  name: "",
  prompt: "",
  scheduleType: "every",
  cronExpression: "",
  everyIntervalMs: "60000",
  atTimestampRfc3339: "",
  enabled: true,
  channel: "",
};
