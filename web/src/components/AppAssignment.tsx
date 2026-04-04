import { Show, For, createSignal } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { X, Plus, Speaker } from "lucide-solid";
import type { Channel, App, AppAssignment as AppAssignmentType } from "../types/session";

interface AppAssignmentProps {
  channelId: string;
  channel: Channel;
  apps: App[];
}

export default function AppAssignment(props: AppAssignmentProps) {
  const { send, state } = useSession();
  const [pickerOpen, setPickerOpen] = createSignal(false);

  const showAssignment = () => props.channel.allowAppAssignment && !props.channel.autoApp;

  const allAssignedApps = () => {
    const assigned = new Set<string>();
    for (const channel of Object.values(state.session.channels) as Channel[]) {
      if (channel.autoApp) continue;
      for (const a of channel.assignedApps ?? []) {
        assigned.add(`${a.applicationName}:${a.binaryName}`);
      }
    }
    return assigned;
  };

  const availableApps = () =>
    props.apps.filter((app) => {
      const key = `${app.name}:${app.binary}`;
      return !allAssignedApps().has(key);
    });

  return (
    <Show when={showAssignment()}>
      <div class="px-2 pb-1.5">
        <div class="flex flex-wrap items-center gap-1">
          <For each={props.channel.assignedApps ?? []}>
            {(assignment: AppAssignmentType) => (
              <span class="inline-flex items-center gap-0.5 rounded bg-accent/15 px-1.5 py-0.5 text-[10px] text-accent">
                <span class="max-w-[6rem] truncate">{assignment.applicationName}</span>
                <button
                  onClick={() =>
                    send({
                      type: "unassignApp",
                      channel: props.channelId,
                      applicationName: assignment.applicationName,
                      binaryName: assignment.binaryName,
                    })
                  }
                  class="ml-0.5 text-accent/60 hover:text-vu-hot"
                  title={`Unassign ${assignment.applicationName}`}
                >
                  <X size={10} />
                </button>
              </span>
            )}
          </For>
          <div class="relative">
            <button
              onClick={() => setPickerOpen((v) => !v)}
              class="inline-flex items-center gap-0.5 rounded border border-dashed border-border px-1 py-0.5 text-[10px] text-text-muted transition-colors hover:border-accent hover:text-accent"
              title="Assign app"
            >
              <Plus size={10} />
              <Show when={(props.channel.assignedApps ?? []).length === 0}>
                <span>App</span>
              </Show>
            </button>
            <Show when={pickerOpen()}>
              <div class="fixed inset-0 z-20" onClick={() => setPickerOpen(false)} />
              <div class="absolute bottom-full left-0 z-30 mb-1 w-48 rounded-lg border border-border bg-bg-elevated shadow-xl">
                <div class="max-h-48 overflow-y-auto p-1">
                  <Show
                    when={availableApps().length > 0}
                    fallback={
                      <p class="px-2 py-3 text-center text-[11px] text-text-muted">
                        No unassigned apps
                      </p>
                    }
                  >
                    <For each={availableApps()}>
                      {(app) => (
                        <button
                          onClick={() => {
                            send({
                              type: "assignApp",
                              channel: props.channelId,
                              applicationName: app.name,
                              binaryName: app.binary,
                            });
                            setPickerOpen(false);
                          }}
                          class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-bg-hover"
                        >
                          <Speaker size={12} class="shrink-0 text-text-muted" />
                          <span class="truncate text-[11px] text-text-secondary">
                            {app.name || app.binary}
                          </span>
                        </button>
                      )}
                    </For>
                  </Show>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
}
