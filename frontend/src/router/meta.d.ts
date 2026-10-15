import "vue-router";

export {};

declare module "vue-router" {
  interface RouteMeta {
    ssg?: boolean;
    title?: string;
  }
}
