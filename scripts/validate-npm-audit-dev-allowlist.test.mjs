#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(
  new URL("./validate-npm-audit-dev-allowlist.mjs", import.meta.url),
);

function dateWithOffsetDays(days) {
  const date = new Date();
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

function writeJsonFile(baseDir, fileName, payload) {
  const filePath = path.join(baseDir, fileName);
  fs.writeFileSync(filePath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  return filePath;
}

function emptyAuditReport() {
  return { vulnerabilities: {} };
}

function fullAuditWithAdvisory(id) {
  return {
    vulnerabilities: {
      eslint: {
        via: [
          {
            severity: "high",
            title: "fixture advisory",
            url: `https://github.com/advisories/${id}`,
          },
        ],
      },
    },
  };
}

function createTempFixtureDir(testName) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `${testName}-`));
}

function runValidator({ full, runtime, allowlist, summary }) {
  return spawnSync(
    process.execPath,
    [
      scriptPath,
      "--full",
      full,
      "--runtime",
      runtime,
      "--allowlist",
      allowlist,
      "--summary",
      summary,
      "--threshold",
      "high",
    ],
    { encoding: "utf8" },
  );
}

test("fails when allowlist contains stale expired entry", (t) => {
  const tmpDir = createTempFixtureDir("allowlist-expired-stale");
  t.after(() => fs.rmSync(tmpDir, { recursive: true, force: true }));

  const fullPath = writeJsonFile(tmpDir, "full.json", emptyAuditReport());
  const runtimePath = writeJsonFile(tmpDir, "runtime.json", emptyAuditReport());
  const allowlistPath = writeJsonFile(tmpDir, "allowlist.json", {
    version: 1,
    entries: [
      {
        id: "GHSA-stale-expired-0001",
        expires_on: dateWithOffsetDays(-2),
        owner: "@marektomas-cz",
        reason: "fixture",
      },
    ],
  });
  const summaryPath = path.join(tmpDir, "summary.json");

  const result = runValidator({
    full: fullPath,
    runtime: runtimePath,
    allowlist: allowlistPath,
    summary: summaryPath,
  });

  assert.equal(result.status, 1);
  assert.match(result.stdout, /Expired allowlist entry GHSA-stale-expired-0001/);
  assert.match(result.stderr, /expired_allowlist=1/);

  const summary = JSON.parse(fs.readFileSync(summaryPath, "utf8"));
  assert.equal(summary.counts.dev_only_tracked, 0);
  assert.equal(summary.counts.expired, 1);
  assert.equal(summary.counts.expired_dev_only, 0);
  assert.equal(summary.counts.stale_allowlist, 1);
});

test("passes when stale allowlist entry is not expired", (t) => {
  const tmpDir = createTempFixtureDir("allowlist-stale-not-expired");
  t.after(() => fs.rmSync(tmpDir, { recursive: true, force: true }));

  const fullPath = writeJsonFile(tmpDir, "full.json", emptyAuditReport());
  const runtimePath = writeJsonFile(tmpDir, "runtime.json", emptyAuditReport());
  const allowlistPath = writeJsonFile(tmpDir, "allowlist.json", {
    version: 1,
    entries: [
      {
        id: "GHSA-stale-valid-0001",
        expires_on: dateWithOffsetDays(14),
        owner: "@marektomas-cz",
        reason: "fixture",
      },
    ],
  });
  const summaryPath = path.join(tmpDir, "summary.json");

  const result = runValidator({
    full: fullPath,
    runtime: runtimePath,
    allowlist: allowlistPath,
    summary: summaryPath,
  });

  assert.equal(result.status, 0);
  assert.equal(result.stderr, "");

  const summary = JSON.parse(fs.readFileSync(summaryPath, "utf8"));
  assert.equal(summary.counts.dev_only_tracked, 0);
  assert.equal(summary.counts.expired, 0);
  assert.equal(summary.counts.expired_dev_only, 0);
  assert.equal(summary.counts.stale_allowlist, 1);
});

test("fails when active dev advisory uses an expired allowlist entry", (t) => {
  const tmpDir = createTempFixtureDir("allowlist-active-expired");
  t.after(() => fs.rmSync(tmpDir, { recursive: true, force: true }));

  const advisoryId = "GHSA-active-expired-0001";
  const fullPath = writeJsonFile(tmpDir, "full.json", fullAuditWithAdvisory(advisoryId));
  const runtimePath = writeJsonFile(tmpDir, "runtime.json", emptyAuditReport());
  const allowlistPath = writeJsonFile(tmpDir, "allowlist.json", {
    version: 1,
    entries: [
      {
        id: advisoryId,
        expires_on: dateWithOffsetDays(-2),
        owner: "@marektomas-cz",
        reason: "fixture",
      },
    ],
  });
  const summaryPath = path.join(tmpDir, "summary.json");

  const result = runValidator({
    full: fullPath,
    runtime: runtimePath,
    allowlist: allowlistPath,
    summary: summaryPath,
  });

  assert.equal(result.status, 1);
  assert.match(result.stdout, /Expired allowlist entry GHSA-active-expired-0001/);
  assert.match(result.stderr, /unallowlisted=0, expired_allowlist=1/);

  const summary = JSON.parse(fs.readFileSync(summaryPath, "utf8"));
  assert.equal(summary.counts.dev_only_tracked, 1);
  assert.equal(summary.counts.unallowlisted, 0);
  assert.equal(summary.counts.expired, 1);
  assert.equal(summary.counts.expired_dev_only, 1);
});
