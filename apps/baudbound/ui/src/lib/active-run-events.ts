import type { ActiveRun, ActiveRunEvent } from "@/lib/runner-api";

const maxLiveLogEntries = 500;

export type ActiveRunState = {
  revision: number;
  runs: ActiveRun[];
};

export function applyActiveRunEvent(
  state: ActiveRunState,
  event: ActiveRunEvent,
): ActiveRunState {
  if (event.kind === "run_recorded" || event.revision <= state.revision) {
    return state;
  }

  if (event.kind === "started") {
    return {
      revision: event.revision,
      runs: sortRuns([
        ...state.runs.filter((run) => run.run_id !== event.run.run_id),
        event.run,
      ]),
    };
  }

  if (event.kind === "log_emitted") {
    let found = false;
    const runs = state.runs.map((run) => {
      if (run.run_id !== event.run_id) return run;
      found = true;
      return {
        ...run,
        discarded_log_count: event.discarded_log_count,
        logs: [...run.logs, event.log].slice(-maxLiveLogEntries),
      };
    });
    return found ? { revision: event.revision, runs } : state;
  }

  if (event.kind === "cancellation_requested") {
    return {
      revision: event.revision,
      runs: state.runs.map((run) =>
        run.run_id === event.run_id
          ? { ...run, cancellation_requested: true }
          : run,
      ),
    };
  }

  return {
    revision: event.revision,
    runs: state.runs.filter((run) => run.run_id !== event.run_id),
  };
}

export function mergeActiveRunState(
  current: ActiveRunState,
  incoming: ActiveRunState,
): ActiveRunState {
  return current.revision > incoming.revision ? current : incoming;
}

function sortRuns(runs: ActiveRun[]) {
  return runs.sort(
    (left, right) =>
      left.started_at_unix_ms - right.started_at_unix_ms ||
      left.run_id.localeCompare(right.run_id),
  );
}
