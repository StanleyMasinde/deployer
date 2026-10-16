import { z } from "zod";
import { nextTick, ref } from "vue";

const email = z
  .string()
  .trim()
  .max(254, "Use at most 254 characters.")
  .email("Enter a valid email address.");
export const signInSchema = z.object({
  email,
  password: z.string().min(1, "Enter your password.").max(1024, "Use at most 1024 characters."),
});
export const setupSchema = z
  .object({
    code: z.string().trim().min(1, "Enter the setup code."),
    name: z.string().trim().min(1, "Enter your name.").max(128, "Use at most 128 characters."),
    email,
    password: z
      .string()
      .min(12, "Use at least 12 characters.")
      .max(1024, "Use at most 1024 characters."),
    confirmation: z.string(),
  })
  .refine((v) => v.password === v.confirmation, {
    path: ["confirmation"],
    message: "Passwords do not match.",
  });
const buildConfig = z.discriminatedUnion("ecosystem", [
  z.object({
    deploymentType: z.literal("static"),
    ecosystem: z.literal("none"),
    packageManager: z.null(),
    framework: z.literal("plainHtml"),
  }),
  z.object({
    deploymentType: z.literal("static"),
    ecosystem: z.literal("javascript"),
    packageManager: z.enum(["npm", "pnpm", "yarn", "bun"]),
    framework: z.literal("vite"),
  }),
]);
export const applicationSchema = z
  .object({
    repository: z.string().regex(/^[\w.-]{1,90}\/[\w.-]{1,90}$/, "Select a repository."),
    branch: z
      .string()
      .min(1, "Enter a branch.")
      .max(191)
      .regex(/^[a-zA-Z0-9/_.-]+$/, "Enter a valid branch.")
      .refine((v) => !v.includes(".."), "Enter a valid branch."),
    domain: z
      .string()
      .max(253)
      .regex(
        /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/,
        "Enter a domain without a protocol or path.",
      ),
    buildConfig,
    installCommand: z
      .string()
      .max(2000)
      .refine((v) => !v.includes("\0"), "Remove the null character."),
    buildCommand: z
      .string()
      .max(2000)
      .refine((v) => !v.includes("\0"), "Remove the null character."),
    outputDirectory: z
      .string()
      .min(1, "Enter an output directory.")
      .max(191)
      .refine(
        (v) =>
          v === "." || v.split("/").every((p) => /^[\w.-]+$/.test(p) && p !== "." && p !== ".."),
        "Use a relative directory without traversal.",
      ),
    spa: z.boolean(),
  })
  .superRefine((v, ctx) => {
    for (const field of ["installCommand", "buildCommand"] as const) {
      if (v.buildConfig.ecosystem === "javascript" && !v[field].trim())
        ctx.addIssue({ code: "custom", path: [field], message: "Enter a command." });
      if (v.buildConfig.ecosystem === "none" && v[field])
        ctx.addIssue({
          code: "custom",
          path: [field],
          message: "Plain static sites do not use build commands.",
        });
    }
  });
export const deletionSchema = (domain: string) =>
  z.object({
    confirmation: z.literal(domain, { error: "Enter the application's domain exactly." }),
  });

export function useValidation() {
  const errors = ref<Record<string, string>>({});
  function validate(schema: z.ZodType, value: unknown) {
    const result = schema.safeParse(value);
    errors.value = {};
    if (result.success) return true;
    for (const issue of result.error.issues) errors.value[issue.path.join(".")] ??= issue.message;
    void nextTick(() =>
      document.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus({ preventScroll: false }),
    );
    return false;
  }
  return { errors, validate };
}
