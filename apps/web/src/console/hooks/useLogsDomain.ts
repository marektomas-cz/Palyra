import { useEffect, useMemo, useState } from "react";

import type { ConsoleApiClient, LogListEnvelope, LogRecord } from "../../consoleApi";
import { toErrorMessage } from "../shared";

type LogWindowKey = "15m" | "1h" | "24h" | "7d";

type UseLogsDomainArgs = {
  api: ConsoleApiClient;
  setError: (message: string | null) => void;
  setNotice: (message: string | null) => void;
};

const DEFAULT_LIMIT = 120;
const FOLLOW_POLL_MS = 5_000;

export function useLogsDomain({ api, setError, setNotice }: UseLogsDomainArgs) {
  const [busy, setBusy] = useState(false);
  const [records, setRecords] = useState<LogRecord[]>([]);
  const [availableSources, setAvailableSources] = useState<string[]>([]);
  const [windowKey, setWindowKey] = useState<LogWindowKey>("1h");
  const [source, setSource] = useState("");
  const [severity, setSeverity] = useState("");
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
  const [selectedCursor, setSelectedCursor] = useState("");
  const [page, setPage] = useState<LogListEnvelope["page"] | null>(null);
  const [newestCursor, setNewestCursor] = useState("");

  const selectedRecord = useMemo(
    () => records.find((record) => record.cursor === selectedCursor) ?? null,
    [records, selectedCursor],
  );

  useEffect(() => {
    void refreshLogs();
  }, [windowKey, source, severity, query]);

  useEffect(() => {
    if (!follow || newestCursor.length === 0) {
      return undefined;
    }
    const timer = window.setInterval(() => {
      void refreshFollowSlice();
    }, FOLLOW_POLL_MS);
    return () => window.clearInterval(timer);
  }, [follow, newestCursor, windowKey, source, severity, query]);

  function buildParams(now = Date.now()): URLSearchParams {
    const params = new URLSearchParams();
    params.set("limit", DEFAULT_LIMIT.toString());
    params.set("end_at_unix_ms", now.toString());
    params.set("start_at_unix_ms", Math.max(0, now - logWindowMs(windowKey)).toString());
    if (source.trim().length > 0) {
      params.set("source", source.trim());
    }
    if (severity.trim().length > 0) {
      params.set("severity", severity.trim());
    }
    if (query.trim().length > 0) {
      params.set("contains", query.trim());
    }
    return params;
  }

  async function refreshLogs(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const response = await api.listLogs(buildParams());
      applyEnvelope(response, false);
    } catch (error) {
      setError(toErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function refreshFollowSlice(): Promise<void> {
    try {
      const params = buildParams();
      params.set("direction", "after");
      params.set("cursor", newestCursor);
      const response = await api.listLogs(params);
      applyEnvelope(response, true);
    } catch (error) {
      setError(toErrorMessage(error));
    }
  }

  function applyEnvelope(response: LogListEnvelope, appendNewer: boolean): void {
    setAvailableSources(response.available_sources);
    setPage(response.page);
    setNewestCursor(response.newest_cursor ?? "");
    setRecords((previous) => {
      if (!appendNewer || response.records.length === 0) {
        return response.records;
      }
      const merged = [...response.records, ...previous];
      const seen = new Set<string>();
      return merged.filter((record) => {
        if (seen.has(record.cursor)) {
          return false;
        }
        seen.add(record.cursor);
        return true;
      });
    });
    setSelectedCursor((previous) => {
      if (previous.length > 0 && response.records.some((record) => record.cursor === previous)) {
        return previous;
      }
      if (appendNewer && previous.length > 0) {
        return previous;
      }
      return response.records[0]?.cursor ?? "";
    });
  }

  function exportLogs(format: "csv" | "json"): void {
    const params = buildParams();
    params.set("format", format);
    window.open(
      api.resolvePath(`/console/v1/logs/export?${params.toString()}`),
      "_blank",
      "noopener",
    );
    setNotice(`Logs export started (${format.toUpperCase()}).`);
  }

  return {
    busy,
    records,
    availableSources,
    windowKey,
    setWindowKey,
    source,
    setSource,
    severity,
    setSeverity,
    query,
    setQuery,
    follow,
    setFollow,
    selectedCursor,
    setSelectedCursor,
    selectedRecord,
    page,
    refreshLogs,
    exportLogs,
  };
}

function logWindowMs(windowKey: LogWindowKey): number {
  switch (windowKey) {
    case "15m":
      return 15 * 60 * 1000;
    case "1h":
      return 60 * 60 * 1000;
    case "24h":
      return 24 * 60 * 60 * 1000;
    case "7d":
      return 7 * 24 * 60 * 60 * 1000;
  }
}
