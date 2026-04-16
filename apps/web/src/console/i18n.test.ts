import { describe, expect, it } from "vite-plus/test";

import {
  describeConsoleLocale,
  nextConsoleLocale,
  readStoredConsoleLocale,
  translateConsoleMessage,
} from "./i18n";

describe("console i18n", () => {
  it("cycles locales through english, czech, and pseudo-localization", () => {
    expect(nextConsoleLocale("en")).toBe("cs");
    expect(nextConsoleLocale("cs")).toBe("qps-ploc");
    expect(nextConsoleLocale("qps-ploc")).toBe("en");
  });

  it("returns Czech translations for shell labels", () => {
    expect(translateConsoleMessage("cs", "shell.signOut")).toBe("Odhlásit");
    expect(describeConsoleLocale("cs")).toBe("Čeština");
  });

  it("keeps pseudo localization visible", () => {
    expect(translateConsoleMessage("qps-ploc", "shell.signOut")).toContain("[~ ");
  });

  it("reads stored czech locale safely", () => {
    window.localStorage.setItem("palyra.console.locale", "cs");
    expect(readStoredConsoleLocale()).toBe("cs");
  });
});
