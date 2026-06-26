CREATE TABLE records (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID        NOT NULL REFERENCES entities(id),
    tenant_id   UUID        NOT NULL,
    data        JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ,
    row_version INTEGER     NOT NULL DEFAULT 1
);

-- GIN index for JSONB path queries — critical for filter performance.
CREATE INDEX idx_records_data ON records USING GIN (data jsonb_path_ops);

-- B-tree composite index for tenant-scoped list + cursor pagination.
CREATE INDEX idx_records_entity_tenant ON records (entity_id, tenant_id, updated_at DESC, id);

-- Partial index for active records (most queries exclude deleted).
CREATE INDEX idx_records_active ON records (entity_id, tenant_id, updated_at DESC)
    WHERE deleted_at IS NULL;
