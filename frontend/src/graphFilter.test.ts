import { describe, expect, it } from "vitest";
import { graphFilterSuggestions } from "./graphFilter";

const baseInput = {
  nodeIds: ["1/alpha", "1/beta", "2/worker-loop"],
  locations: ["src/main.rs:12", "crates/peeps/src/enabled.rs:505"],
  crates: [
    { id: "peeps-core", label: "peeps-core" },
    { id: "peeps-web", label: "peeps-web" },
  ],
  processes: [
    { id: "1", label: "web(1234)" },
    { id: "2", label: "worker(5678)" },
  ],
  kinds: [
    { id: "request", label: "Request" },
    { id: "response", label: "Response" },
  ],
};

describe("graphFilterSuggestions", () => {
  it("filters key suggestions when no colon is present", () => {
    const out = graphFilterSuggestions({ ...baseInput, fragment: "col" });
    expect(out.map((s) => s.token)).toContain("colorBy:process");
    expect(out.map((s) => s.token)).toContain("colorBy:crate");
    expect(out.map((s) => s.token)).not.toContain("groupBy:process");
  });

  it("filters node suggestions by value after key", () => {
    const out = graphFilterSuggestions({ ...baseInput, fragment: "node:alp" });
    expect(out[0]?.token).toBe("node:1/alpha");
    expect(out.map((s) => s.token)).not.toContain("node:1/beta");
  });

  it("supports fuzzy subsequence matching", () => {
    const out = graphFilterSuggestions({ ...baseInput, fragment: "location:smr1" });
    expect(out.map((s) => s.token)).toContain("location:src/main.rs:12");
  });

  it("matches process suggestions by label as well as id", () => {
    const out = graphFilterSuggestions({ ...baseInput, fragment: "process:work" });
    expect(out[0]?.token).toBe("process:2");
  });
});
