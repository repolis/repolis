import { describe, expect, it } from "vitest";

import { groupByFile } from "../utils/file-groups";

const ref = (id: string, name: string, file: string) => ({ id, name, file });

describe("group by file", () => {
  it("keeps two buildings with one name apart by file and id", () => {
    const groups = groupByFile([
      ref("a::Config", "Config", "src/config.c"),
      ref("b::Config", "Config", "plugins/ini/config.c"),
    ]);
    expect(groups.map((g) => g.refs[0].id)).toEqual(["b::Config", "a::Config"]);
  });

  it("puts the file with most links first and sorts names inside it", () => {
    const groups = groupByFile([
      ref("1", "Token", "lexer.c"),
      ref("2", "Buffer", "buffer.c"),
      ref("3", "Lexer", "lexer.c"),
    ]);
    expect(groups).toEqual([
      {
        file: "lexer.c",
        refs: [ref("3", "Lexer", "lexer.c"), ref("1", "Token", "lexer.c")],
      },
      { file: "buffer.c", refs: [ref("2", "Buffer", "buffer.c")] },
    ]);
  });
});
