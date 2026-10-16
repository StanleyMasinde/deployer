CREATE TABLE `password_resets` (
    `id` BIGINT UNSIGNED NOT NULL,
    `token_hash` VARCHAR(191) NOT NULL,
    `created_at` BIGINT NOT NULL,
    `expires_at` BIGINT NOT NULL,
    `used` BOOLEAN NOT NULL,
    PRIMARY KEY (`id`)
);
