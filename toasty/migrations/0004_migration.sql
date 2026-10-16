ALTER TABLE `apps` ADD COLUMN `workflow_pr_state` VARCHAR(191);
-- #[toasty::breakpoint]
ALTER TABLE `apps` ADD COLUMN `workflow_pr_updated_at` VARCHAR(191);
