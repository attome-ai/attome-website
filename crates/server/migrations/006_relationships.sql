CREATE TABLE entity_relationships (
    id             UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    from_entity_id UUID        NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    to_entity_id   UUID        NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    rel_type       TEXT        NOT NULL,
    cascade        TEXT        NOT NULL DEFAULT 'restrict',
    name           TEXT        NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_entity_relationships UNIQUE (from_entity_id, to_entity_id, name)
);

CREATE INDEX idx_entity_rel_from ON entity_relationships (from_entity_id);
CREATE INDEX idx_entity_rel_to   ON entity_relationships (to_entity_id);
