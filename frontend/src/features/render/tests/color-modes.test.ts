import { describe, expect, it } from "vitest";

import { colorMode } from "../utils/color-modes";
import { ramp } from "../utils/colors";
import { SCALES, widget } from "./fixtures";

describe("birth mode", () => {
  const born = colorMode("born");
  const scales = { ...SCALES, born: 300 };

  it("puts the newest file at the hot end", () => {
    const b = { ...widget, commit_churn: 2, born_day: 300 };
    expect(born.color(b, scales)).toEqual(ramp(1));
  });

  it("does not paint files without history as the oldest", () => {
    const b = { ...widget, commit_churn: 0, born_day: 0 };
    expect(born.color(b, scales)).not.toEqual(ramp(0));
  });
});

describe("smells mode", () => {
  const smells = colorMode("smells");

  it("colours a building by the smell the backend found", () => {
    const god = smells.color({ ...widget, smell: "god" }, SCALES);
    const none = smells.color(widget, SCALES);
    expect(god).not.toEqual(none);
  });
});
