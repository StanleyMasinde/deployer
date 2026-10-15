CREATE TABLE `jobs` (
    `id` BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    `deployment_id` BIGINT UNSIGNED,
    `kind` VARCHAR(191) NOT NULL,
    `payload` TEXT,
    `error` TEXT,
    `status` VARCHAR(191) NOT NULL,
    `attempts` INT UNSIGNED NOT NULL,
    `available_at` BIGINT NOT NULL,
    `created_at` BIGINT NOT NULL,
    `finished_at` BIGINT,
    PRIMARY KEY (`id`)
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX `index_jobs_by_deployment_id` ON `jobs` (`deployment_id`);
-- #[toasty::breakpoint]
CREATE INDEX `index_jobs_by_status` ON `jobs` (`status`);
-- #[toasty::breakpoint]
CREATE TABLE `users` (
    `id` BIGINT UNSIGNED NOT NULL,
    `name` VARCHAR(128) NOT NULL,
    `email` VARCHAR(254) NOT NULL,
    `password_hash` VARCHAR(191) NOT NULL,
    PRIMARY KEY (`id`)
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX `index_users_by_email` ON `users` (`email`);
-- #[toasty::breakpoint]
CREATE TABLE `apps` (
    `id` BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    `provider` VARCHAR(191) NOT NULL,
    `repository` VARCHAR(191) NOT NULL,
    `branch` VARCHAR(191) NOT NULL,
    `domain` VARCHAR(253) NOT NULL,
    `install_command` TEXT NOT NULL,
    `build_command` TEXT NOT NULL,
    `output_directory` VARCHAR(191) NOT NULL,
    `spa` BOOLEAN NOT NULL,
    `workflow_pr` TEXT,
    PRIMARY KEY (`id`)
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX `index_apps_by_repository` ON `apps` (`repository`);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX `index_apps_by_domain` ON `apps` (`domain`);
-- #[toasty::breakpoint]
CREATE TABLE `deployments` (
    `id` BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    `event_key` VARCHAR(191) NOT NULL,
    `app_id` BIGINT UNSIGNED NOT NULL,
    `run_id` BIGINT UNSIGNED NOT NULL,
    `attempt` INT UNSIGNED NOT NULL,
    `commit_sha` VARCHAR(191) NOT NULL,
    `status` VARCHAR(191) NOT NULL,
    `error` TEXT,
    `created_at` BIGINT NOT NULL,
    `finished_at` BIGINT,
    PRIMARY KEY (`id`)
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX `index_deployments_by_event_key` ON `deployments` (`event_key`);
-- #[toasty::breakpoint]
CREATE INDEX `index_deployments_by_app_id` ON `deployments` (`app_id`);
-- #[toasty::breakpoint]
CREATE INDEX `index_deployments_by_status` ON `deployments` (`status`);
