CREATE TABLE entities (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id   UUID        NOT NULL,
    name        TEXT        NOT NULL,
    plural_name TEXT        NOT NULL,
    metadata    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_entities_tenant_name UNIQUE (tenant_id, name)
);

CREATE INDEX idx_entities_tenant_id ON entities (tenant_id);
