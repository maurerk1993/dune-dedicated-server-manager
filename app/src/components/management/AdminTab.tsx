import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import {
  AlertDialog,
  Badge,
  Box,
  Button,
  Checkbox,
  Flex,
  Select,
  Table,
  Text,
  TextArea,
  TextField,
} from "@radix-ui/themes";

import { managementApi } from "../../services/management";
import type {
  Category,
  CommandSpec,
  FieldSpec,
  HistoryDto,
  ItemCatalogChangeDto,
  ItemCatalogCheckDto,
  ItemCatalogDiffDto,
  ItemCatalogStatusDto,
  ItemDto,
  PlayerSpecializationDto,
  PublishResultDto,
  SpecializationTrackDto,
} from "../../types/management";
import { formatTime } from "../../utils/formatting";
import Combobox from "./Combobox";

export type AdminTabPrefill = {
  commandId: string;
  values: Record<string, unknown>;
} | null;

export type AdminTabProps = {
  tunnelId: string;
  prefill?: AdminTabPrefill;
  onPrefillConsumed?: () => void;
};

const CATEGORY_LABEL: Record<Category, string> = {
  items: "Inventory",
  player: "Player ops",
  progression: "Progression",
  movement: "Teleport & spawn",
  broadcast: "Broadcast",
  journey: "Story journey",
  exec: "Server scripts",
};

const CATEGORY_ORDER: Category[] = [
  "broadcast",
  "items",
  "player",
  "progression",
  "movement",
  "journey",
  "exec",
];

const CLIENT_DEFAULTS: Record<string, unknown> = {
  Quantity: 1,
  Durability: 1.0,
  WaterAmount: 1_000_000,
  Experience: 1000,
  Level: 1,
  SkillPoints: 0,
  BroadcastType: "Generic",
  BroadcastDuration: 30,
  Persistent: 1.0,
};

function applyDefaults(spec: CommandSpec): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const field of spec.fields) {
    if (CLIENT_DEFAULTS[field.key] !== undefined) {
      out[field.key] = CLIENT_DEFAULTS[field.key];
    } else if (field.default !== undefined && field.default !== null) {
      out[field.key] = field.default;
    }
  }
  return out;
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value.trim() : value == null ? "" : String(value).trim();
}

function numberValue(value: unknown, fallback: number): number {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
}

function clampInt(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, Math.round(value)));
}

function apiErrorMessage(err: unknown): string {
  const raw = String(err);
  const bodyStart = raw.indexOf("{");
  if (bodyStart >= 0) {
    try {
      const obj = JSON.parse(raw.slice(bodyStart));
      if (obj && typeof obj.error === "string") return obj.error;
    } catch {
      // fall through to raw
    }
  }
  return raw;
}

export default function AdminTab({ tunnelId, prefill, onPrefillConsumed }: AdminTabProps) {
  const [commands, setCommands] = useState<CommandSpec[]>([]);
  const [selected, setSelected] = useState<CommandSpec | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [history, setHistory] = useState<HistoryDto[]>([]);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<PublishResultDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [selectedPanel, setSelectedPanel] = useState<"commands" | "item-catalog">("commands");
  const appliedRef = useRef<{ selectedId: string; prefillFp: string | null } | null>(null);
  // Templates available for the currently-picked vehicle (SpawnVehicleAt).
  // Populated whenever values.ClassName changes so TemplateName renders as a
  // proper combobox of valid options instead of a free-text field.
  const [vehicleTemplates, setVehicleTemplates] = useState<string[]>([]);
  const [selectedItem, setSelectedItem] = useState<ItemDto | null>(null);

  const refreshHistory = useCallback(async () => {
    try {
      const list = await managementApi.history(tunnelId, 30);
      setHistory(list);
    } catch (err) {
      setError(String(err));
    }
  }, [tunnelId]);

  useEffect(() => {
    managementApi
      .listCommands(tunnelId)
      .then(setCommands)
      .catch((err) => setError(String(err)));
    void refreshHistory();
  }, [tunnelId, refreshHistory]);

  useEffect(() => {
    // Reset values + apply prefill exactly once per (selected, prefill) pair.
    // The earlier two-effect version raced; the single-effect version still
    // clobbered prefill on the next render after onPrefillConsumed cleared it
    // because the [prefill] dep change re-ran the defaults reset. Track what
    // we've already applied so post-consumption re-renders are a no-op.
    if (!selected) {
      appliedRef.current = null;
      return;
    }
    const prefillFp =
      prefill && prefill.commandId === selected.id ? JSON.stringify(prefill) : null;
    const current = appliedRef.current;

    if (!current || current.selectedId !== selected.id) {
      // Brand new command pick (sidebar click or first prefill into a new command).
      if (prefillFp) {
        setValues({ ...applyDefaults(selected), ...(prefill?.values ?? {}) });
        onPrefillConsumed?.();
      } else {
        setValues(applyDefaults(selected));
      }
      setResult(null);
      appliedRef.current = { selectedId: selected.id, prefillFp };
      return;
    }

    // Same command. Only act if a NEW prefill arrived for it.
    if (prefillFp && prefillFp !== current.prefillFp) {
      setValues((prev) => ({ ...prev, ...(prefill?.values ?? {}) }));
      setResult(null);
      onPrefillConsumed?.();
      appliedRef.current = { selectedId: selected.id, prefillFp };
    }
    // Otherwise the prefill was cleared after we consumed it — leave values alone.
  }, [selected, prefill, onPrefillConsumed]);

  useEffect(() => {
    // If a prefill arrives for a command different from what's currently
    // selected, switch the sidebar to that command. The effect above will
    // then notice prefill.commandId === selected.id and apply the values.
    if (!prefill || commands.length === 0) return;
    if (selected?.id === prefill.commandId) return;
    const target = commands.find((c) => c.id === prefill.commandId);
    if (!target) return;
    setSelected(target);
    setSelectedPanel("commands");
  }, [prefill, commands, selected?.id]);

  useEffect(() => {
    // For SpawnVehicleAt, look up the templates of the picked vehicle so the
    // TemplateName field can render its real options.
    const cls =
      selected?.id === "SpawnVehicleAt" && typeof values.ClassName === "string"
        ? (values.ClassName as string).trim()
        : "";
    if (!cls) {
      setVehicleTemplates([]);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const matches = await managementApi.searchVehicles(tunnelId, cls, 10);
        const hit = matches.find((v) => v.id === cls || v.actor_class === cls);
        const templates = hit?.templates ?? [];
        if (cancelled) return;
        setVehicleTemplates(templates);
        // If the current TemplateName isn't valid for this vehicle, auto-pick
        // the first available one. Keeps the form submittable without the user
        // having to know that TreadWheel doesn't carry a T0.
        if (templates.length > 0) {
          setValues((prev) => {
            const current = typeof prev.TemplateName === "string" ? prev.TemplateName : "";
            if (current && templates.includes(current)) return prev;
            return { ...prev, TemplateName: templates[0] };
          });
        }
      } catch {
        if (!cancelled) setVehicleTemplates([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selected?.id, values.ClassName, tunnelId]);

  useEffect(() => {
    const itemId =
      selected?.id === "AddItemToInventory" && typeof values.ItemName === "string"
        ? values.ItemName.trim()
        : "";
    if (!itemId) {
      setSelectedItem(null);
      setValues((prev) => (prev.Quality ? { ...prev, Quality: 0 } : prev));
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const matches = await managementApi.searchItems(tunnelId, itemId, 10);
        const hit = matches.find((item) => item.id === itemId) ?? null;
        if (cancelled) return;
        setSelectedItem(hit);
        if (!hit?.gradeable) {
          setValues((prev) => (prev.Quality ? { ...prev, Quality: 0 } : prev));
        }
      } catch {
        if (!cancelled) setSelectedItem(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selected?.id, values.ItemName, tunnelId]);

  const grouped = useMemo(() => groupByCategory(commands), [commands]);

  const doPublish = useCallback(async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      let out: PublishResultDto;
      if (selected.id === "AddItemToInventory" && numberValue(values.Quality, 0) > 0) {
        const playerId = stringValue(values.PlayerId);
        const itemId = stringValue(values.ItemName);
        const quantity = numberValue(values.Quantity, 1);
        const quality = numberValue(values.Quality, 0);
        if (!playerId || playerId === "*") {
          throw new Error("Graded item grants require one specific offline player.");
        }
        if (!itemId) {
          throw new Error("Pick an item before granting a Grade.");
        }
        const matches = await managementApi.searchItems(tunnelId, itemId, 10);
        const item = matches.find((row) => row.id === itemId);
        if (!item?.gradeable) {
          throw new Error("The selected item does not support Grades.");
        }
        out = await managementApi.grantQualityItem(tunnelId, playerId, itemId, quantity, quality);
      } else {
        out = await managementApi.publish(tunnelId, selected.id, values);
      }
      setResult(out);
      await refreshHistory();
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setBusy(false);
    }
  }, [selected, tunnelId, values, refreshHistory]);

  const publish = useCallback(() => {
    if (!selected) return;
    if (selected.destructive) {
      setConfirmOpen(true);
    } else {
      void doPublish();
    }
  }, [selected, doPublish]);

  return (
    <Flex mt="3" gap="3" align="stretch" wrap="wrap">
      <Box style={{ flex: "0 0 240px", minWidth: 0 }}>
        <Text size="2" weight="medium">
          Commands
        </Text>
        <Box mt="2">
          <Button
            size="1"
            variant={selectedPanel === "item-catalog" ? "solid" : "surface"}
            onClick={() => setSelectedPanel("item-catalog")}
            style={{ justifyContent: "flex-start", width: "100%" }}
          >
            Item Catalog
          </Button>
        </Box>
        {CATEGORY_ORDER.map((cat) => {
          const specs = grouped[cat];
          if (!specs || specs.length === 0) return null;
          return (
            <Box key={cat} mt="2">
              <Text size="1" color="gray" style={{ textTransform: "uppercase", letterSpacing: 0.5 }}>
                {CATEGORY_LABEL[cat] ?? cat}
              </Text>
              <Flex direction="column" gap="1" mt="1">
                {specs.map((spec) => (
                  <Button
                    key={spec.id}
                    size="1"
                    variant={selectedPanel === "commands" && selected?.id === spec.id ? "solid" : "surface"}
                    color={spec.destructive ? "red" : undefined}
                    onClick={() => {
                      setSelected(spec);
                      setSelectedPanel("commands");
                    }}
                    style={{ justifyContent: "flex-start" }}
                  >
                    {spec.label}
                  </Button>
                ))}
              </Flex>
            </Box>
          );
        })}
      </Box>
      <Box style={{ flex: "1 1 400px", minWidth: 0 }}>
        {selectedPanel === "item-catalog" ? (
          <ItemCatalogPanel tunnelId={tunnelId} />
        ) : selected ? (
          <Box>
            <Flex justify="between" align="baseline" wrap="wrap" gap="2">
              <Text size="3" weight="medium">
                {selected.label}
              </Text>
              {selected.destructive ? <Badge color="red">destructive</Badge> : null}
            </Flex>
            <Text size="1" color="gray">
              {selected.describe}
            </Text>
            {selected.id === "SpecializationLevelXp" ? (
              <SpecializationLevelPanel tunnelId={tunnelId} onHistoryRefresh={refreshHistory} />
            ) : (
              <>
                <Flex direction="column" gap="3" mt="3">
                  {visibleFields(selected, values).map((field) => (
                    <FieldInput
                      key={field.key}
                      field={field}
                      value={values[field.key]}
                      onChange={(v) => setValues((prev) => ({ ...prev, [field.key]: v }))}
                      tunnelId={tunnelId}
                      vehicleTemplates={vehicleTemplates}
                    />
                  ))}
                  {selected.id === "AddItemToInventory" ? (
                    <ItemQualityControl
                      item={selectedItem}
                      value={numberValue(values.Quality, 0)}
                      onChange={(v) => setValues((prev) => ({ ...prev, Quality: v }))}
                    />
                  ) : null}
                </Flex>
                {selected.id === "SpawnVehicleAt" ? (
                  <UsePlayerPositionButton
                    tunnelId={tunnelId}
                    playerId={values.PlayerId as string | undefined}
                    onLocation={(loc) =>
                      setValues((prev) => ({ ...prev, X: loc.x, Y: loc.y, Z: loc.z }))
                    }
                  />
                ) : null}
                <Flex mt="3" gap="2" align="center">
                  <Button onClick={publish} disabled={busy} color={selected.destructive ? "red" : undefined}>
                    {busy ? "Publishing..." : selected.destructive ? "Publish (destructive)" : "Publish"}
                  </Button>
                  {result ? (
                    <Badge color={result.ok ? "green" : "red"}>{result.ok ? "ok" : "failed"}</Badge>
                  ) : null}
                </Flex>
                {result && !result.ok && result.error ? (
                  <Text size="1" color="red" mt="2">
                    {result.error}
                  </Text>
                ) : null}
                {result?.output ? (
                  <Box
                    mt="2"
                    className="mono"
                    style={{ fontSize: 11, padding: 6, background: "var(--color-panel-translucent)", whiteSpace: "pre-wrap" }}
                  >
                    {result.output}
                  </Box>
                ) : null}
                {error ? (
                  <Text size="1" color="red" mt="2">
                    {error}
                  </Text>
                ) : null}
              </>
            )}
          </Box>
        ) : (
          <Text color="gray">Select a command on the left.</Text>
        )}
      </Box>
      <Box style={{ flex: "1 1 320px", minWidth: 0 }}>
        <Text size="2" weight="medium">
          Recent publishes
        </Text>
        <Table.Root variant="surface" size="1" mt="1">
          <Table.Header>
            <Table.Row>
              <Table.ColumnHeaderCell>Cmd</Table.ColumnHeaderCell>
              <Table.ColumnHeaderCell>OK</Table.ColumnHeaderCell>
              <Table.ColumnHeaderCell>When</Table.ColumnHeaderCell>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {history.map((h) => (
              <Table.Row key={h.id}>
                <Table.Cell className="mono" style={{ fontSize: 11 }}>
                  {h.command}
                </Table.Cell>
                <Table.Cell>
                  <Badge color={h.ok ? "green" : "red"}>{h.ok ? "ok" : "fail"}</Badge>
                </Table.Cell>
                <Table.Cell className="mono" style={{ fontSize: 11 }}>
                  {formatTime(h.createdAt)}
                </Table.Cell>
              </Table.Row>
            ))}
          </Table.Body>
        </Table.Root>
      </Box>

      <AlertDialog.Root open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialog.Content maxWidth="460px">
          <AlertDialog.Title>Run {selected?.label}?</AlertDialog.Title>
          <AlertDialog.Description size="2">
            This command is destructive and cannot be undone. {selected?.describe}
          </AlertDialog.Description>
          <Flex gap="2" mt="4" justify="end">
            <AlertDialog.Cancel>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </AlertDialog.Cancel>
            <Button
              color="red"
              onClick={() => {
                setConfirmOpen(false);
                void doPublish();
              }}
            >
              Run it
            </Button>
          </Flex>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </Flex>
  );
}

function groupByCategory(specs: CommandSpec[]): Record<string, CommandSpec[]> {
  const out: Record<string, CommandSpec[]> = {};
  for (const spec of specs) {
    if (!out[spec.category]) out[spec.category] = [];
    out[spec.category].push(spec);
  }
  return out;
}

function compareText(a: string | undefined | null, b: string | undefined | null): number {
  return (a || "").localeCompare(b || "", undefined, { sensitivity: "base", numeric: true });
}

function comparePlayers(a: any, b: any): number {
  const aOnline = String(a.online || "").toLowerCase() === "online";
  const bOnline = String(b.online || "").toLowerCase() === "online";
  if (aOnline !== bOnline) return aOnline ? -1 : 1;
  return compareText(a.name || a.flsId, b.name || b.flsId);
}

function sortCommandOptions(kind: ComboboxKind, options: any[]): any[] {
  const rows = [...options];
  if (kind === "players") return rows.sort(comparePlayers);
  if (kind === "vehicles") return rows.sort((a, b) => compareText(a.id, b.id));
  return rows.sort((a, b) => compareText(a.name || a.id, b.name || b.id));
}

function UsePlayerPositionButton({
  tunnelId,
  playerId,
  onLocation,
}: {
  tunnelId: string;
  playerId: string | undefined;
  onLocation: (loc: { x: number; y: number; z: number }) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const enabled = !!playerId && !busy;

  const click = useCallback(async () => {
    if (!playerId) return;
    setBusy(true);
    setError(null);
    try {
      const loc = await managementApi.playerLocation(tunnelId, playerId);
      onLocation(loc);
    } catch (err) {
      // Backend wraps proxy errors as `GET /path -> STATUS: {"error":"…"}`.
      // Pull out the inner `error` field for a readable message; fall back to raw.
      const raw = String(err);
      let nice = raw;
      const bodyStart = raw.indexOf("{");
      if (bodyStart >= 0) {
        try {
          const obj = JSON.parse(raw.slice(bodyStart));
          if (obj && typeof obj.error === "string") nice = obj.error;
        } catch {
          // leave nice as raw
        }
      }
      setError(nice);
    } finally {
      setBusy(false);
    }
  }, [tunnelId, playerId, onLocation]);

  return (
    <Box mt="2">
      <Button size="1" variant="soft" disabled={!enabled} onClick={click}>
        {busy ? "Fetching…" : "Use player's current position"}
      </Button>
      {!playerId ? (
        <Text size="1" color="gray" ml="2">
          (pick a player first)
        </Text>
      ) : null}
      {error ? (
        <Text size="1" color="red" as="div" mt="1">
          {error}
        </Text>
      ) : null}
    </Box>
  );
}

function ItemQualityControl({
  item,
  value,
  onChange,
}: {
  item: ItemDto | null;
  value: number;
  onChange: (value: number) => void;
}) {
  if (!item?.gradeable) return null;
  const safeValue = clampInt(value, 0, 5);
  return (
    <Box>
      <Flex justify="between" align="baseline" gap="2">
        <Text size="2" weight="medium">
          Item Grade
        </Text>
        <Badge color="amber">supports Grades</Badge>
      </Flex>
      <Box mt="1">
        <Select.Root value={String(safeValue)} onValueChange={(next) => onChange(Number(next))}>
          <Select.Trigger />
          <Select.Content>
            <Select.Item value="0">No Grade</Select.Item>
            <Select.Item value="1">Grade 1</Select.Item>
            <Select.Item value="2">Grade 2</Select.Item>
            <Select.Item value="3">Grade 3</Select.Item>
            <Select.Item value="4">Grade 4</Select.Item>
            <Select.Item value="5">Grade 5</Select.Item>
          </Select.Content>
        </Select.Root>
      </Box>
      <Text size="1" color="gray" as="div" mt="1">
        Grades are a late-game system for specific schematics and their crafted items, separate
        from Mk item tiers. No Grade uses the normal grant path; Grades 1-5 require one specific
        offline player.
      </Text>
    </Box>
  );
}

type CatalogReviewTab = "added" | "changed" | "removed";

function ItemCatalogPanel({ tunnelId }: { tunnelId: string }) {
  const [status, setStatus] = useState<ItemCatalogStatusDto | null>(null);
  const [check, setCheck] = useState<ItemCatalogCheckDto | null>(null);
  const [tab, setTab] = useState<CatalogReviewTab>("added");
  const [confirmRemovals, setConfirmRemovals] = useState(false);
  const [busy, setBusy] = useState<"status" | "check" | "apply" | "export" | "revert" | null>(
    null,
  );
  const [revertOpen, setRevertOpen] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    setBusy((current) => current ?? "status");
    setError(null);
    try {
      const next = await managementApi.itemCatalogStatus(tunnelId);
      setStatus(next);
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setBusy((current) => (current === "status" ? null : current));
    }
  }, [tunnelId]);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const checkForUpdates = useCallback(async () => {
    setBusy("check");
    setError(null);
    setMessage(null);
    setConfirmRemovals(false);
    try {
      const next = await managementApi.itemCatalogCheck(tunnelId);
      setCheck(next);
      setTab(firstNonEmptyTab(next.diff));
      setMessage("Catalog update check completed.");
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [tunnelId]);

  const applyCatalog = useCallback(async () => {
    if (!check) return;
    setBusy("apply");
    setError(null);
    setMessage(null);
    try {
      const next = await managementApi.itemCatalogApply(
        tunnelId,
        check.catalog,
        check.sourceUrl,
        check.sourceVersion,
        confirmRemovals,
      );
      setStatus(next);
      setCheck(null);
      setConfirmRemovals(false);
      setMessage("Approved catalog applied to this server service.");
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [check, confirmRemovals, tunnelId]);

  const exportCatalog = useCallback(async () => {
    setBusy("export");
    setError(null);
    setMessage(null);
    try {
      const exported = await managementApi.itemCatalogExport(tunnelId);
      const path = await save({
        defaultPath: exported.suggestedFileName,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      await managementApi.writeItemCatalogExport(path, exported.catalogJson);
      setMessage(`Exported ${exported.summary.itemCount.toLocaleString()} items.`);
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [tunnelId]);

  const revertCatalog = useCallback(async () => {
    setBusy("revert");
    setError(null);
    setMessage(null);
    try {
      const next = await managementApi.itemCatalogRevert(tunnelId);
      setStatus(next);
      setCheck(null);
      setConfirmRemovals(false);
      setRevertOpen(false);
      setMessage("Reverted to the bundled item catalog.");
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [tunnelId]);

  const diff = check?.diff ?? null;
  const hasRemovals = (diff?.removed.length ?? 0) > 0;
  const blocked = (diff?.blockingErrors.length ?? 0) > 0;
  const applyDisabled = !diff || blocked || (hasRemovals && !confirmRemovals) || busy !== null;

  return (
    <Box>
      <Flex justify="between" align="baseline" wrap="wrap" gap="2">
        <Text size="3" weight="medium">
          Item Catalog
        </Text>
        {status ? (
          <Badge color={status.active.source === "override" ? "amber" : "gray"}>
            {status.active.source}
          </Badge>
        ) : null}
      </Flex>
      <Text size="1" color="gray">
        Review catalog updates before they affect Grant Item searches or graded item grants.
      </Text>

      {status ? <CatalogStatusSummary status={status} /> : null}

      <Flex mt="3" gap="2" align="center" wrap="wrap">
        <Button size="1" onClick={() => void checkForUpdates()} disabled={busy !== null}>
          {busy === "check" ? "Checking..." : "Check for catalog updates"}
        </Button>
        <Button
          size="1"
          variant="soft"
          onClick={() => void exportCatalog()}
          disabled={busy !== null || !status}
        >
          {busy === "export" ? "Exporting..." : "Export repo-ready catalog"}
        </Button>
        <Button
          size="1"
          variant="soft"
          color="red"
          onClick={() => setRevertOpen(true)}
          disabled={busy !== null || status?.active.source !== "override"}
        >
          Revert to bundled catalog
        </Button>
      </Flex>

      {diff ? (
        <Box mt="3">
          <CatalogDiffSummary diff={diff} />
          {diff.warnings.length > 0 ? (
            <Flex direction="column" gap="1" mt="2">
              {diff.warnings.map((warning) => (
                <Text key={warning} size="1" color="amber">
                  {warning}
                </Text>
              ))}
            </Flex>
          ) : null}
          {diff.blockingErrors.length > 0 ? (
            <Flex direction="column" gap="1" mt="2">
              {diff.blockingErrors.map((item) => (
                <Text key={item} size="1" color="red">
                  {item}
                </Text>
              ))}
            </Flex>
          ) : null}

          <Flex mt="3" gap="2" wrap="wrap">
            <Button
              size="1"
              variant={tab === "added" ? "solid" : "surface"}
              onClick={() => setTab("added")}
            >
              Added ({diff.added.length})
            </Button>
            <Button
              size="1"
              variant={tab === "changed" ? "solid" : "surface"}
              onClick={() => setTab("changed")}
            >
              Changed ({diff.changed.length})
            </Button>
            <Button
              size="1"
              variant={tab === "removed" ? "solid" : "surface"}
              color={diff.removed.length > 0 ? "red" : undefined}
              onClick={() => setTab("removed")}
            >
              Removed ({diff.removed.length})
            </Button>
          </Flex>

          <CatalogDiffTable diff={diff} tab={tab} />

          {hasRemovals ? (
            <Flex mt="3" gap="2" align="center">
              <Checkbox
                checked={confirmRemovals}
                onCheckedChange={(value) => setConfirmRemovals(Boolean(value))}
              />
              <Text size="1" color="red">
                I reviewed the removals and want to apply this catalog anyway.
              </Text>
            </Flex>
          ) : null}

          <Flex mt="3" gap="2" align="center" wrap="wrap">
            <Button onClick={() => void applyCatalog()} disabled={applyDisabled}>
              {busy === "apply" ? "Applying..." : "Apply approved catalog"}
            </Button>
            <Flex gap="1" align="center" wrap="wrap" style={{ minWidth: 0 }}>
              <Text size="1" color="gray">
                Source:
              </Text>
              <Badge color="gray" title={diff.sourceUrl || undefined}>
                {catalogSourceLabel(diff.sourceUrl)}
              </Badge>
              <Badge color="gray" className="mono">
                {shortHash(diff.candidate.catalogHash)}
              </Badge>
            </Flex>
          </Flex>
        </Box>
      ) : null}

      {status?.overrideError ? (
        <Text size="1" color="red" as="div" mt="2">
          Override ignored: {status.overrideError}
        </Text>
      ) : null}
      {message ? (
        <Text size="1" color="green" as="div" mt="2">
          {message}
        </Text>
      ) : null}
      {error ? (
        <Text size="1" color="red" as="div" mt="2">
          {error}
        </Text>
      ) : null}

      <AlertDialog.Root open={revertOpen} onOpenChange={setRevertOpen}>
        <AlertDialog.Content maxWidth="440px">
          <AlertDialog.Title>Revert item catalog?</AlertDialog.Title>
          <AlertDialog.Description size="2">
            This removes the local catalog override from this server service. Grant Item will use
            the bundled catalog again.
          </AlertDialog.Description>
          <Flex gap="2" mt="4" justify="end">
            <AlertDialog.Cancel>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </AlertDialog.Cancel>
            <Button color="red" disabled={busy === "revert"} onClick={() => void revertCatalog()}>
              {busy === "revert" ? "Reverting..." : "Revert"}
            </Button>
          </Flex>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </Box>
  );
}

function CatalogStatusSummary({ status }: { status: ItemCatalogStatusDto }) {
  return (
    <Flex mt="3" gap="2" wrap="wrap">
      <CatalogMetric label="Active items" value={status.active.itemCount} />
      <CatalogMetric label="Supports Grades" value={status.active.gradeableCount} />
      <CatalogMetric label="Stackable" value={status.active.stackableCount} />
      <CatalogMetric label="Hash" value={shortHash(status.active.catalogHash)} />
      {status.overrideMeta ? (
        <CatalogMetric label="Applied" value={formatTime(status.overrideMeta.appliedAt)} />
      ) : null}
    </Flex>
  );
}

function CatalogDiffSummary({ diff }: { diff: ItemCatalogDiffDto }) {
  return (
    <Flex gap="2" wrap="wrap">
      <CatalogMetric label="Added" value={diff.added.length} />
      <CatalogMetric label="Changed" value={diff.changed.length} />
      <CatalogMetric label="Removed" value={diff.removed.length} />
      <CatalogMetric label="Candidate items" value={diff.candidate.itemCount} />
      <CatalogMetric label="Supports Grades" value={diff.candidate.gradeableCount} />
      <CatalogMetric label="Stackable" value={diff.candidate.stackableCount} />
    </Flex>
  );
}

function CatalogMetric({ label, value }: { label: string; value: number | string }) {
  return (
    <Box
      style={{
        border: "1px solid var(--gray-a6)",
        borderRadius: 8,
        padding: "8px 10px",
        minWidth: 110,
        background: "var(--color-panel-translucent)",
      }}
    >
      <Text size="1" color="gray" as="div">
        {label}
      </Text>
      <Text size="2" weight="medium" className={typeof value === "string" ? "mono" : undefined}>
        {typeof value === "number" ? value.toLocaleString() : value}
      </Text>
    </Box>
  );
}

function CatalogDiffTable({ diff, tab }: { diff: ItemCatalogDiffDto; tab: CatalogReviewTab }) {
  const rows = tab === "added" ? diff.added : tab === "removed" ? diff.removed : diff.changed;
  const empty = rows.length === 0;
  return (
    <Table.Root variant="surface" size="1" mt="2">
      <Table.Header>
        <Table.Row>
          <Table.ColumnHeaderCell>Item</Table.ColumnHeaderCell>
          <Table.ColumnHeaderCell>Category</Table.ColumnHeaderCell>
          <Table.ColumnHeaderCell>Source</Table.ColumnHeaderCell>
          <Table.ColumnHeaderCell>Metadata</Table.ColumnHeaderCell>
        </Table.Row>
      </Table.Header>
      <Table.Body>
        {empty ? (
          <Table.Row>
            <Table.Cell colSpan={4}>
              <Text size="1" color="gray">
                No {tab} items.
              </Text>
            </Table.Cell>
          </Table.Row>
        ) : tab === "changed" ? (
          (rows as ItemCatalogChangeDto[]).map((change) => (
            <CatalogChangeTableRow key={change.id} change={change} />
          ))
        ) : (
          (rows as ItemDto[]).map((item, index) => (
            <CatalogItemTableRow
              key={`${item.id}-${item.category}-${item.source}-${index}`}
              item={item}
            />
          ))
        )}
      </Table.Body>
    </Table.Root>
  );
}

function CatalogItemTableRow({ item }: { item: ItemDto }) {
  return (
    <Table.Row>
      <Table.Cell>
        <Text size="2" as="div">
          {item.name}
        </Text>
        <Text size="1" color="gray" className="mono">
          {item.id}
        </Text>
      </Table.Cell>
      <Table.Cell>{item.category}</Table.Cell>
      <Table.Cell>{item.source}</Table.Cell>
      <Table.Cell>
        <CatalogItemBadges item={item} />
      </Table.Cell>
    </Table.Row>
  );
}

function CatalogChangeTableRow({ change }: { change: ItemCatalogChangeDto }) {
  return (
    <Table.Row>
      <Table.Cell>
        <Text size="2" as="div">
          {change.after.name}
        </Text>
        <Text size="1" color="gray" className="mono">
          {change.id}
        </Text>
      </Table.Cell>
      <Table.Cell>{change.after.category}</Table.Cell>
      <Table.Cell>{change.after.source}</Table.Cell>
      <Table.Cell>
        <Flex gap="1" wrap="wrap">
          {change.fields.map((field) => (
            <Badge key={field} color="blue">
              {field}
            </Badge>
          ))}
          <CatalogItemBadges item={change.after} />
        </Flex>
      </Table.Cell>
    </Table.Row>
  );
}

function CatalogItemBadges({ item }: { item: ItemDto }) {
  return (
    <Flex gap="1" wrap="wrap">
      {item.gradeable ? <Badge color="amber">supports Grades</Badge> : null}
      {item.tier ? <Badge color="gray">tier {item.tier}</Badge> : null}
      {item.stackMax ? <Badge color="gray">stack {item.stackMax}</Badge> : null}
    </Flex>
  );
}

function firstNonEmptyTab(diff: ItemCatalogDiffDto): CatalogReviewTab {
  if (diff.added.length > 0) return "added";
  if (diff.changed.length > 0) return "changed";
  return "removed";
}

function shortHash(hash: string | null | undefined): string {
  if (!hash) return "unknown";
  return hash.length > 10 ? hash.slice(0, 10) : hash;
}

function catalogSourceLabel(sourceUrl: string | null | undefined): string {
  if (!sourceUrl) return "unknown";
  try {
    const url = new URL(sourceUrl);
    const file = url.pathname.split("/").filter(Boolean).pop() || "catalog";
    if (url.hostname.includes("githubusercontent.com")) return `GitHub asset: ${file}`;
    if (url.hostname === "github.com") return file;
    return `${url.hostname}: ${file}`;
  } catch {
    return sourceUrl.length > 32 ? `${sourceUrl.slice(0, 29)}...` : sourceUrl;
  }
}

function SpecializationLevelPanel({
  tunnelId,
  onHistoryRefresh,
}: {
  tunnelId: string;
  onHistoryRefresh: () => Promise<void>;
}) {
  const [playerId, setPlayerId] = useState("");
  const [data, setData] = useState<PlayerSpecializationDto | null>(null);
  const [drafts, setDrafts] = useState<Record<string, number>>({});
  const [loading, setLoading] = useState(false);
  const [busyTrack, setBusyTrack] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<PublishResultDto | null>(null);

  const refresh = useCallback(async () => {
    const flsId = playerId.trim();
    if (!flsId) {
      setData(null);
      setDrafts({});
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const next = await managementApi.specialization(tunnelId, flsId);
      setData(next);
      setDrafts(
        Object.fromEntries(
          next.tracks.map((track) => [
            track.trackType,
            clampInt(track.level, 0, Math.round(track.levelMax || 100)),
          ]),
        ),
      );
    } catch (err) {
      setError(apiErrorMessage(err));
      setData(null);
      setDrafts({});
    } finally {
      setLoading(false);
    }
  }, [playerId, tunnelId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const setLevel = useCallback(
    async (track: SpecializationTrackDto) => {
      const flsId = playerId.trim();
      if (!flsId) return;
      const level = clampInt(
        drafts[track.trackType] ?? track.level,
        0,
        Math.round(track.levelMax || 100),
      );
      setBusyTrack(track.trackType);
      setError(null);
      setResult(null);
      try {
        const out = await managementApi.setSpecializationLevel(
          tunnelId,
          flsId,
          track.trackType,
          level,
        );
        setResult(out);
        await refresh();
        await onHistoryRefresh();
      } catch (err) {
        setError(apiErrorMessage(err));
      } finally {
        setBusyTrack(null);
      }
    },
    [drafts, onHistoryRefresh, playerId, refresh, tunnelId],
  );

  const playerOnline = (data?.player.online || "").toLowerCase() === "online";

  return (
    <Box mt="3">
      <Flex direction="column" gap="3">
        <Box>
          <Flex justify="between" align="baseline" gap="2">
            <Text size="2" weight="medium">
              Player
            </Text>
            {data ? (
              <Badge color={playerOnline ? "green" : "gray"}>{data.player.online || "offline"}</Badge>
            ) : null}
          </Flex>
          <Box mt="1">
            <CommandCombobox
              kind="players"
              value={playerId}
              onPick={(value) => {
                setPlayerId(stringValue(value));
                setResult(null);
                setError(null);
              }}
              tunnelId={tunnelId}
            />
          </Box>
        </Box>

        <Flex gap="2" align="center" wrap="wrap">
          <Button size="1" variant="soft" disabled={!playerId || loading} onClick={() => void refresh()}>
            {loading ? "Refreshing..." : "Refresh"}
          </Button>
          {data ? (
            <Text size="1" color="gray">
              Keystones: {data.keystonesTotal} / {data.keystonesMax}
            </Text>
          ) : null}
        </Flex>

        {data ? (
          <Flex direction="column" gap="2">
            {data.tracks.map((track) => (
              <SpecializationTrackRow
                key={track.trackType}
                track={track}
                draft={drafts[track.trackType] ?? clampInt(track.level, 0, Math.round(track.levelMax || 100))}
                disabled={playerOnline || loading || busyTrack !== null}
                busy={busyTrack === track.trackType}
                onDraft={(level) =>
                  setDrafts((prev) => ({
                    ...prev,
                    [track.trackType]: clampInt(level, 0, Math.round(track.levelMax || 100)),
                  }))
                }
                onSet={() => void setLevel(track)}
              />
            ))}
          </Flex>
        ) : (
          <Text size="2" color="gray">
            Pick a player to load specialization levels.
          </Text>
        )}

        {playerOnline ? (
          <Text size="1" color="amber">
            This player is online. Level edits are disabled until they fully log out.
          </Text>
        ) : null}
        {result?.output ? (
          <Box
            className="mono"
            style={{ fontSize: 11, padding: 6, background: "var(--color-panel-translucent)", whiteSpace: "pre-wrap" }}
          >
            {result.output}
          </Box>
        ) : null}
        {result && !result.ok && result.error ? (
          <Text size="1" color="red">
            {result.error}
          </Text>
        ) : null}
        {error ? (
          <Text size="1" color="red">
            {error}
          </Text>
        ) : null}
      </Flex>
    </Box>
  );
}

function SpecializationTrackRow({
  track,
  draft,
  disabled,
  busy,
  onDraft,
  onSet,
}: {
  track: SpecializationTrackDto;
  draft: number;
  disabled: boolean;
  busy: boolean;
  onDraft: (level: number) => void;
  onSet: () => void;
}) {
  const maxLevel = Math.round(track.levelMax || 100);
  const safeDraft = clampInt(draft, 0, maxLevel);
  return (
    <Box
      style={{
        border: "1px solid var(--gray-a6)",
        borderRadius: 8,
        padding: 12,
        background: "var(--color-panel-translucent)",
      }}
    >
      <Flex align="center" gap="3" wrap="wrap">
        <Box style={{ flex: "1 1 150px", minWidth: 0 }}>
          <Text size="2" weight="bold" as="div">
            {track.trackType}
          </Text>
          <Text size="1" color="gray" className="mono">
            Lv {Math.round(track.level)} / {maxLevel} - {track.xp.toLocaleString()} /{" "}
            {track.xpMax.toLocaleString()} xp
          </Text>
        </Box>
        <Box style={{ flex: "2 1 260px", minWidth: 180 }}>
          <input
            type="range"
            min={0}
            max={maxLevel}
            value={safeDraft}
            disabled={disabled}
            onChange={(event) => onDraft(Number(event.target.value))}
            style={{ width: "100%" }}
          />
        </Box>
        <Box style={{ flex: "0 0 86px" }}>
          <TextField.Root
            type="number"
            min={0}
            max={maxLevel}
            value={String(safeDraft)}
            disabled={disabled}
            onChange={(event) => onDraft(Number(event.target.value))}
          />
        </Box>
        <Button size="1" disabled={disabled || busy} onClick={onSet}>
          {busy ? "Setting..." : "Set"}
        </Button>
      </Flex>
    </Box>
  );
}

type ComboboxKind = "items" | "vehicles" | "players" | "skill-modules";

function comboboxKindFor(fieldKey: string): ComboboxKind | null {
  switch (fieldKey) {
    case "ItemName":
      return "items";
    case "ClassName":
      return "vehicles";
    case "PlayerId":
      return "players";
    case "Module":
      return "skill-modules";
    default:
      return null;
  }
}

function FieldInput({
  field,
  value,
  onChange,
  tunnelId,
  vehicleTemplates,
}: {
  field: FieldSpec;
  value: unknown;
  onChange: (v: unknown) => void;
  tunnelId: string;
  vehicleTemplates: string[];
}) {
  const comboKind = comboboxKindFor(field.key);
  const templateMode = field.key === "TemplateName" && vehicleTemplates.length > 0;
  return (
    <Box>
      <Flex justify="between" align="baseline" gap="2">
        <Text size="2" weight="medium">
          {field.label}
          {field.required ? " *" : ""}
        </Text>
        {field.helper ? (
          <Text size="1" color="gray">
            {field.helper}
          </Text>
        ) : null}
      </Flex>
      <Box mt="1">
        {templateMode ? (
          <TemplateCombobox
            value={typeof value === "string" ? value : value == null ? "" : String(value)}
            onPick={onChange}
            templates={vehicleTemplates}
          />
        ) : comboKind ? (
          <CommandCombobox kind={comboKind} value={value} onPick={onChange} tunnelId={tunnelId} />
        ) : (
          renderInput(field, value, onChange)
        )}
      </Box>
    </Box>
  );
}

function TemplateCombobox({
  value,
  onPick,
  templates,
}: {
  value: string;
  onPick: (v: unknown) => void;
  templates: string[];
}) {
  const loadOptions = useCallback(
    async (query: string) => {
      const q = query.trim().toLowerCase();
      const filtered = q
        ? templates.filter((t) => t.toLowerCase().includes(q))
        : templates;
      return [...filtered].sort(compareText).map((name) => ({ name }));
    },
    [templates],
  );
  return (
    <Combobox
      value={value}
      onChange={onPick}
      loadOptions={loadOptions}
      getOptionValue={(o: { name: string }) => o.name}
      resolveLabel={async (id) => id}
      renderOption={(o: { name: string }) => (
        <Text size="2" className="mono">{o.name}</Text>
      )}
      placeholder="Pick a template…"
      searchPlaceholder="Filter templates…"
    />
  );
}

/// Filters the spec's field list down to what's relevant for the current
/// values. Today only ServiceBroadcast has conditional fields — Generic
/// hides the shutdown-specific knobs, ServerShutdown hides Generic-only
/// fields, and a `ShouldCancel=true` hides everything except the cancel
/// toggle itself.
function visibleFields(
  spec: CommandSpec,
  values: Record<string, unknown>,
): FieldSpec[] {
  if (spec.id !== "ServiceBroadcast") return [...spec.fields];
  const broadcastType = (values.BroadcastType as string) || "Generic";
  const shouldCancel = values.ShouldCancel === true;
  const GENERIC_ONLY = new Set(["Title", "Body"]);
  const SHUTDOWN_ONLY = new Set([
    "ShutdownType",
    "ShutdownDuration",
    "BroadcastFrequency",
    "ShouldCancel",
  ]);
  return spec.fields.filter((field) => {
    if (field.key === "BroadcastType") return true;
    if (broadcastType === "Generic") {
      if (SHUTDOWN_ONLY.has(field.key)) return false;
      return true;
    }
    // ServerShutdown branch
    if (GENERIC_ONLY.has(field.key)) return false;
    if (shouldCancel && field.key !== "ShouldCancel") return false;
    return true;
  });
}

function renderInput(field: FieldSpec, value: unknown, onChange: (v: unknown) => void) {
  const strValue = value === undefined || value === null ? "" : String(value);
  if (field.kind === "select" && field.options) {
    return (
      <Select.Root value={strValue || field.options[0].value} onValueChange={onChange}>
        <Select.Trigger />
        <Select.Content>
          {field.options.map((opt) => (
            <Select.Item key={opt.value} value={opt.value}>
              {opt.label}
            </Select.Item>
          ))}
        </Select.Content>
      </Select.Root>
    );
  }
  if (field.kind === "text") {
    return <TextArea value={strValue} onChange={(e) => onChange(e.target.value)} rows={3} />;
  }
  if (field.kind === "bool") {
    const checked = value === true || strValue === "true" || strValue === "1";
    return (
      <Checkbox checked={checked} onCheckedChange={(c) => onChange(Boolean(c))} />
    );
  }
  return (
    <TextField.Root
      value={strValue}
      onChange={(e) => {
        const raw = e.target.value;
        if (field.kind === "int" || field.kind === "float") {
          onChange(raw === "" ? "" : Number(raw));
        } else {
          onChange(raw);
        }
      }}
    />
  );
}

function CommandCombobox({
  kind,
  value,
  onPick,
  tunnelId,
}: {
  kind: ComboboxKind;
  value: unknown;
  onPick: (v: unknown) => void;
  tunnelId: string;
}) {
  const strVal = typeof value === "string" ? value : value == null ? "" : String(value);

  const loadOptions = useCallback(
    async (query: string) => {
      try {
        if (kind === "items") {
          return sortCommandOptions(kind, await managementApi.searchItems(tunnelId, query, 30));
        }
        if (kind === "vehicles") {
          return sortCommandOptions(kind, await managementApi.searchVehicles(tunnelId, query, 30));
        }
        if (kind === "skill-modules") {
          return sortCommandOptions(kind, await managementApi.searchSkillModules(tunnelId, query, 50));
        }
        return sortCommandOptions(kind, await managementApi.searchPlayers(tunnelId, query, 30));
      } catch {
        return [] as never[];
      }
    },
    [kind, tunnelId],
  );

  const resolveLabel = useCallback(
    async (id: string): Promise<string | null> => {
      if (!id) return null;
      try {
        if (kind === "items") {
          const r = await managementApi.searchItems(tunnelId, id, 5);
          const hit = r.find((it) => it.id === id);
          return hit ? `${hit.name}  ·  ${hit.id}` : id;
        }
        if (kind === "players") {
          const r = await managementApi.searchPlayers(tunnelId, id, 5);
          const hit = r.find((p) => p.flsId === id);
          return hit ? `${hit.name} (${hit.online})  ·  ${hit.flsId}` : id;
        }
        if (kind === "skill-modules") {
          const r = await managementApi.searchSkillModules(tunnelId, id, 5);
          const hit = r.find((m) => m.id === id);
          return hit ? `${hit.name}  ·  ${hit.id}` : id;
        }
        const r = await managementApi.searchVehicles(tunnelId, id, 5);
        const hit = r.find((v) => v.id === id || v.actor_class === id);
        if (!hit) return id;
        const templates = Array.isArray(hit.templates) && hit.templates.length > 0
          ? `  ·  templates: ${hit.templates.join(", ")}`
          : "";
        return `${hit.id}${templates}`;
      } catch {
        return id;
      }
    },
    [kind, tunnelId],
  );

  if (kind === "items") {
    return (
      <Combobox
        value={strVal}
        onChange={onPick}
        loadOptions={loadOptions}
        getOptionValue={(it: any) => it.id}
        resolveLabel={resolveLabel}
        renderOption={(it: any) => (
          <Flex justify="between" gap="2">
            <Text size="2">{it.name}</Text>
            <Text size="1" color="gray" className="mono">{it.id}</Text>
          </Flex>
        )}
        placeholder="Pick an item…"
        searchPlaceholder="Search items…"
      />
    );
  }
  if (kind === "vehicles") {
    return (
      <Combobox
        value={strVal}
        onChange={onPick}
        loadOptions={loadOptions}
        // Server expects the DT_VehicleTemplates row key (e.g. "Sandbike"),
        // not the full BP actor class path.
        getOptionValue={(v: any) => v.id}
        resolveLabel={resolveLabel}
        renderOption={(v: any) => (
          <Flex direction="column">
            <Text size="2">{v.id}</Text>
            <Text size="1" color="gray">
              templates: {Array.isArray(v.templates) && v.templates.length > 0 ? v.templates.join(", ") : "—"}
            </Text>
          </Flex>
        )}
        placeholder="Pick a vehicle…"
        searchPlaceholder="Search vehicles…"
      />
    );
  }
  if (kind === "skill-modules") {
    return (
      <Combobox
        value={strVal}
        onChange={onPick}
        loadOptions={loadOptions}
        getOptionValue={(m: any) => m.id}
        resolveLabel={resolveLabel}
        renderOption={(m: any) => (
          <Flex justify="between" gap="2">
            <Box>
              <Text size="2">{m.name}</Text>
              <Text size="1" color="gray" as="div">
                {m.category} · max {m.maxLevel}
              </Text>
            </Box>
            <Text size="1" color="gray" className="mono">{m.id}</Text>
          </Flex>
        )}
        placeholder="Pick a skill module…"
        searchPlaceholder="Search skill modules…"
      />
    );
  }
  return (
    <Combobox
      value={strVal}
      onChange={onPick}
      loadOptions={loadOptions}
      getOptionValue={(p: any) => p.flsId}
      resolveLabel={resolveLabel}
      renderOption={(p: any) => (
        <Flex justify="between" gap="2" align="center">
          <Box>
            <Text size="2">{p.name || "(unnamed)"}</Text>
            <Text size="1" color="gray" as="div" className="mono">{p.flsId}</Text>
          </Box>
          <Badge color={String(p.online || "").toLowerCase() === "online" ? "green" : "gray"}>{p.online || "offline"}</Badge>
        </Flex>
      )}
      placeholder="Pick a player…"
      searchPlaceholder="Search players…"
    />
  );
}
