import { expect, it } from "vitest";
import { signInSchema, setupSchema, applicationSchema, deletionSchema } from "../src/validation";
import { buildPreset } from "../src/build";

it("rejects empty credentials and invalid emails without imposing setup password length at sign-in", () => {
  expect(signInSchema.safeParse({ email: "invalid", password: "" }).success).toBe(false);
  expect(signInSchema.safeParse({ email: "owner@example.com", password: "legacy" }).success).toBe(
    true,
  );
});
it("validates setup password length and confirmation", () => {
  const valid = {
    code: "setup-code",
    name: "Owner",
    email: "owner@example.com",
    password: "long-password",
    confirmation: "long-password",
  };
  expect(setupSchema.safeParse(valid).success).toBe(true);
  expect(
    setupSchema.safeParse({ ...valid, password: "short", confirmation: "short" }).success,
  ).toBe(false);
  const mismatch = setupSchema.safeParse({ ...valid, confirmation: "different" });
  expect(mismatch.success).toBe(false);
  if (!mismatch.success) expect(mismatch.error.issues[0]?.path).toEqual(["confirmation"]);
});
it("checks domains, traversal, and ecosystem-specific commands", () => {
  const valid = {
    repository: "owner/site",
    domain: "site.example.com",
    branch: "main",
    spa: false,
    ...buildPreset("none"),
  };
  expect(applicationSchema.safeParse(valid).success).toBe(true);
  expect(
    applicationSchema.safeParse({ ...valid, domain: "https://site.example.com" }).success,
  ).toBe(false);
  expect(applicationSchema.safeParse({ ...valid, outputDirectory: "../dist" }).success).toBe(false);
  expect(applicationSchema.safeParse({ ...valid, buildCommand: "npm run build" }).success).toBe(
    false,
  );
  expect(
    applicationSchema.safeParse({ ...valid, ...buildPreset("javascript"), installCommand: "" })
      .success,
  ).toBe(false);
});
it("requires an exact domain for deletion", () => {
  expect(
    deletionSchema("site.example.com").safeParse({ confirmation: "other.example.com" }).success,
  ).toBe(false);
  expect(
    deletionSchema("site.example.com").safeParse({ confirmation: "site.example.com" }).success,
  ).toBe(true);
});
