import { describe, expect, it } from "vite-plus/test";

import {
  describeDesktopLocale,
  formatDesktopDateTime,
  nextDesktopLocale,
  translateDesktopMessage,
} from "./i18n";

describe("desktop i18n", () => {
  it("cycles locales through english, czech, and pseudo-localization", () => {
    expect(nextDesktopLocale("en")).toBe("cs");
    expect(nextDesktopLocale("cs")).toBe("qps-ploc");
    expect(nextDesktopLocale("qps-ploc")).toBe("en");
  });

  it("returns Czech translations for desktop shell labels", () => {
    expect(translateDesktopMessage("cs", "desktop.header.refresh")).toBe("Obnovit");
    expect(describeDesktopLocale("cs")).toBe("Čeština");
  });

  it("formats dates with the selected locale instead of forcing english", () => {
    expect(formatDesktopDateTime("cs", 1_710_000_000_000)).toMatch(/\d/);
  });
});
