import { expect, it } from "vitest";
import { effortLabel, effortLevels } from "../components/EffortSelect";
it("uses remote per-model levels without inventing unsupported choices", () => {
  const models = [
    {
      value: "opus",
      label: "Opus",
      resolved: "actual-opus",
      effort_levels: ["low", "high"],
    },
  ];
  expect(effortLevels(models, "opus")).toEqual(["low", "high"]);
  expect(effortLevels(models, "actual-opus")).toEqual(["low", "high"]);
  expect(effortLevels(models, "unknown")).toEqual([]);
  expect(effortLabel("high")).toBe("高");
});
