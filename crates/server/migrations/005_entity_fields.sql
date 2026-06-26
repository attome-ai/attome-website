CREATE TABLE entity_fields (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID        NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    field_type  TEXT        NOT NULL,
    options     JSONB       NOT NULL DEFAULT '{}',
    sort_order  INTEGER     NOT NULL DEFAULT 0,
    is_required BOOLEAN     NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_entity_fields_entity_name UNIQUE (entity_id, name)
);

CREATE INDEX idx_entity_fields_entity_id ON entity_fields (entity_id);
