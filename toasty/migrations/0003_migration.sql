ALTER TABLE `apps` ADD COLUMN `https_expires_at` BIGINT;
-- #[toasty::breakpoint]
ALTER TABLE `apps` ADD COLUMN `https_enabled` BOOLEAN;
-- #[toasty::breakpoint]
ALTER TABLE `apps` ADD COLUMN `https_domain` VARCHAR(253);
-- #[toasty::breakpoint]
ALTER TABLE `apps` ADD COLUMN `https_error` TEXT;
