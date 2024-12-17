CREATE TABLE IF NOT EXISTS "repo_sync_status" (
  "id" SERIAL PRIMARY KEY,
  "crate_name" TEXT NOT NULL,
  "github_url" TEXT,
  "mega_url" TEXT NOT NULL,
  "crate_type" VARCHAR(20) NOT NULL,
  "status" VARCHAR(20) NOT NULL,
  "err_message" TEXT,
  "created_at" TIMESTAMP NOT NULL,
  "updated_at" TIMESTAMP NOT NULL,
  CONSTRAINT uniq_repo_name UNIQUE (crate_name)
);