BEGIN;
    -- Disable triggers on each table.
    ALTER TABLE "crates" DISABLE TRIGGER ALL;
    ALTER TABLE "crate_users" DISABLE TRIGGER ALL;
    ALTER TABLE "crate_owners" DISABLE TRIGGER ALL;
    ALTER TABLE "crate_downloads" DISABLE TRIGGER ALL;

    -- Set defaults for non-nullable columns not included in the dump.

    ALTER TABLE "crate_users" ALTER COLUMN "gh_access_token" SET DEFAULT '';

    -- Truncate all tables.
    TRUNCATE "crates" RESTART IDENTITY CASCADE;
    TRUNCATE "crate_users" RESTART IDENTITY CASCADE;
    TRUNCATE "crate_owners" RESTART IDENTITY CASCADE;
    TRUNCATE "crate_downloads" RESTART IDENTITY CASCADE;

    -- Import the CSV data.

    \copy "crates" ("created_at", "description", "documentation", "homepage", "id", "max_features", "max_upload_size", "name", "readme", "repository", "updated_at") FROM 'data/crates.csv' WITH CSV HEADER
    \copy "crate_users" ("gh_avatar", "gh_id", "gh_login", "id", "name") FROM 'data/crate_users.csv' WITH CSV HEADER
    \copy "crate_owners" ("crate_id", "created_at", "created_by", "owner_id", "owner_kind") FROM 'data/crate_owners.csv' WITH CSV HEADER
    \copy "crate_downloads" ("crate_id", "downloads") FROM 'data/crate_downloads.csv' WITH CSV HEADER

    -- Drop the defaults again.

    ALTER TABLE "crate_users" ALTER COLUMN "gh_access_token" DROP DEFAULT;

    -- Reenable triggers on each table.

    ALTER TABLE "crates" ENABLE TRIGGER ALL;
    ALTER TABLE "crate_users" ENABLE TRIGGER ALL;
    ALTER TABLE "crate_owners" ENABLE TRIGGER ALL;
    ALTER TABLE "crate_downloads" ENABLE TRIGGER ALL;

COMMIT;

