import { describe, expect, it } from "vitest";
import { EMPTY_FILTER } from "./view-controls";
import { decodeView, encodeView, isDefaultView, EMPTY_VIEW } from "./url-state";

describe("url state", () => {
  it("round-trips everything that matters", () => {
    const v = {
      mode: "complexity",
      filter: {
        ...EMPTY_FILTER,
        language: "Go",
        min_complexity: 30,
        only_hubs: true,
      },
      selected: "src/odb.c::git_odb",
      timeline: 120,
      camera: "1.5,-2.5,200.0,0.785,0.620",
    };
    expect(decodeView(encodeView(v))).toEqual(v);
  });

  it("writes nothing for a default view, so a plain link stays clean", () => {
    expect(encodeView(EMPTY_VIEW)).toBe("");
    expect(isDefaultView(decodeView(""))).toBe(true);
  });

  it("survives ids containing separators", () => {
    // Building ids are "path::name" and paths contain slashes and dots.
    const v = { ...EMPTY_VIEW, selected: "crates/core/flags/defs.rs::Flag" };
    expect(decodeView(encodeView(v)).selected).toBe(v.selected);
  });

  it("degrades a damaged link instead of producing NaN", () => {
    const v = decodeView("#m=&f=garbage;cx:notanumber&t=abc&c=1,2");
    expect(v.mode).toBe("typology");
    expect(v.filter.min_complexity).toBe(0);
    expect(v.timeline).toBeNull();
    // A truncated camera must be dropped, not applied as NaN coordinates.
    expect(v.camera).toBeNull();
  });

  it("keeps a well-formed camera", () => {
    expect(decodeView("#c=1.5,-2.5,200,0.78,0.62").camera).toBe("1.5,-2.5,200,0.78,0.62");
  });
});
