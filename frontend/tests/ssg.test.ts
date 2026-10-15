import { describe, expect, it } from "vitest";
import { prerenderedRoutes } from "../src/router/ssg";

it("prerenders only explicitly public routes, including nested route groups", () => {
  const result = prerenderedRoutes(
    ["/", "console", "guide"],
    [
      { path: "/", component: {}, meta: { ssg: true } },
      { path: "/console", component: {}, meta: { ssg: false } },
      { path: "/", children: [{ path: "guide", component: {}, meta: { ssg: true } }] },
    ],
  );

  expect(result).toEqual(["/", "/guide"]);
});
