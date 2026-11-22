import type { Job } from "./types";

export type QueueEntry = { job: Job } | { groupId: string; jobs: Job[] };

/**
 * Playlist items are shown together under one header, placed where the group's first item
 * sits in the queue. Items keep their queue order inside the group.
 */
export function groupJobs(jobs: Job[]): QueueEntry[] {
  const groups = new Map<string, Job[]>();
  for (const j of jobs) if (j.group) groups.set(j.group.id, [...(groups.get(j.group.id) ?? []), j]);
  const out: QueueEntry[] = [];
  const seen = new Set<string>();
  for (const j of jobs) {
    if (!j.group) out.push({ job: j });
    else if (!seen.has(j.group.id)) {
      seen.add(j.group.id);
      out.push({ groupId: j.group.id, jobs: groups.get(j.group.id)! });
    }
  }
  return out;
}
