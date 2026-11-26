import { describe, expect, it } from "vitest";

import { applyAge, ramp } from "../utils/colors";

describe("colour", () => {
  it("ramps darker toward hot and stays in range", () => {
    const [a, b] = [ramp(0), ramp(1)];
    expect(b[0] < a[0] || b[2] < a[2]).toBe(true);
    for (const t of [-1, 0, 0.5, 1, 2])
      for (const c of ramp(t)) expect(c >= 0 && c <= 1).toBe(true);
  });

  it("leaves recent code alone and greys old code", () => {
    expect(applyAge([0.8, 0.2, 0.2], 10)).toEqual([0.8, 0.2, 0.2]);
    const old = applyAge([0.8, 0.2, 0.2], 2000);
    expect(old[0] - old[1]).toBeLessThan(0.6);
  });
});
