function walkBack(
  prev: Map<string, string>,
  from: string,
  to: string,
): string[] {
  const path = [to];
  for (let at = to; at !== from;) {
    at = prev.get(at)!;
    path.push(at);
  }
  return path.reverse();
}

/** Hop count, ignoring direction: "three steps away" is actionable, a weighted cost is not. */
export function shortestPath(
  adj: Map<string, Set<string>>,
  from: string,
  to: string,
): string[] {
  const prev = new Map<string, string>([[from, from]]);
  const queue = [from];
  for (let head = 0; head < queue.length; head++) {
    const current = queue[head];
    if (current === to) return walkBack(prev, from, to);
    for (const next of adj.get(current) ?? []) {
      if (!prev.has(next)) {
        prev.set(next, current);
        queue.push(next);
      }
    }
  }
  return [];
}

export const closedLoop = (loop: string[]) => [...loop, loop[0]];
