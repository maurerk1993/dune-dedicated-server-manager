import { CheckIcon, ChevronDownIcon } from "@radix-ui/react-icons";
import { DropdownMenu } from "@radix-ui/themes";

import {
  SERVER_AUTO_REFRESH_INTERVALS,
  type ServerAutoRefreshInterval,
} from "../../types/ui";

export type ServerAutoRefreshControlProps = {
  enabled: boolean;
  intervalSeconds: ServerAutoRefreshInterval;
  onToggle: () => void;
  onIntervalChange: (intervalSeconds: ServerAutoRefreshInterval) => void;
};

export default function ServerAutoRefreshControl({
  enabled,
  intervalSeconds,
  onToggle,
  onIntervalChange,
}: ServerAutoRefreshControlProps) {
  return (
    <div className="auto-refresh-split" data-enabled={enabled}>
      <button
        type="button"
        className="action-btn auto-refresh-main"
        data-tone={enabled ? "ok" : "default"}
        onClick={onToggle}
        title={enabled ? "Turn auto-refresh off" : "Turn auto-refresh on"}
      >
        {enabled ? `Auto ${intervalSeconds}s` : "Auto off"}
      </button>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          <button
            type="button"
            className="action-btn auto-refresh-menu"
            data-tone={enabled ? "ok" : "default"}
            aria-label="Choose auto-refresh interval"
            title="Choose auto-refresh interval"
          >
            <ChevronDownIcon />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end">
          {SERVER_AUTO_REFRESH_INTERVALS.map((seconds) => (
            <DropdownMenu.Item key={seconds} onSelect={() => onIntervalChange(seconds)}>
              <span className="auto-refresh-menu-item">
                <span>{seconds}s</span>
                {seconds === intervalSeconds ? (
                  <CheckIcon />
                ) : (
                  <span className="auto-refresh-check-placeholder" />
                )}
              </span>
            </DropdownMenu.Item>
          ))}
          <DropdownMenu.Separator />
          <DropdownMenu.Item onSelect={onToggle}>
            {enabled ? "Turn auto-refresh off" : "Turn auto-refresh on"}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </div>
  );
}
